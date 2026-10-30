//! Legitimate account verification only. Production endpoint is fixed; no offline
//! fallback, forwarded identities, caller-supplied URLs, or access-token handling.
use openssl::{
    pkey::{PKey, Private},
    rand::rand_bytes,
    rsa::{Padding, Rsa},
};
use smolmc_protocol_777::login::{Profile, Property};
use std::{io, sync::Arc, time::Duration};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use zeroize::Zeroizing;

const AUTH_CONCURRENCY: usize = 8;
const RSA_CONCURRENCY: usize = 2;
const MAX_RESPONSE: usize = 64 * 1024;
const SESSION_URL: &str = "https://sessionserver.mojang.com/session/minecraft/hasJoined";

fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "account verification failed",
    )
}
fn unavailable() -> io::Error {
    io::Error::new(
        io::ErrorKind::ConnectionRefused,
        "account verification unavailable",
    )
}

pub struct Auth {
    key: PKey<Private>,
    der: Vec<u8>,
    http: reqwest::Client,
    admissions: Arc<Semaphore>,
    jobs: Arc<Semaphore>,
}

pub struct Challenge {
    pub token: [u8; 4],
    _permit: OwnedSemaphorePermit,
}

impl Auth {
    /// Startup only, before the listening socket is advertised. No per-login keygen.
    pub fn new() -> io::Result<Self> {
        let rsa = Rsa::generate(1024).map_err(|_| unavailable())?;
        let key = PKey::from_rsa(rsa).map_err(|_| unavailable())?;
        let der = key.public_key_to_der().map_err(|_| unavailable())?;
        let http = reqwest::Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(8))
            .pool_max_idle_per_host(2)
            .pool_idle_timeout(Duration::from_secs(30))
            .http1_only()
            .user_agent("SmolMC-M1B/0.1")
            .build()
            .map_err(|_| unavailable())?;
        Ok(Self {
            key,
            der,
            http,
            admissions: Arc::new(Semaphore::new(AUTH_CONCURRENCY)),
            jobs: Arc::new(Semaphore::new(RSA_CONCURRENCY)),
        })
    }
    pub fn public_key(&self) -> &[u8] {
        &self.der
    }
    pub fn in_flight(&self) -> usize {
        AUTH_CONCURRENCY - self.admissions.available_permits()
    }
    pub fn begin(&self) -> io::Result<Challenge> {
        let permit = Arc::clone(&self.admissions)
            .try_acquire_owned()
            .map_err(|_| {
                io::Error::new(io::ErrorKind::WouldBlock, "authentication capacity reached")
            })?;
        let mut token = [0; 4];
        rand_bytes(&mut token).map_err(|_| unavailable())?;
        Ok(Challenge {
            token,
            _permit: permit,
        })
    }
    pub async fn decrypt(
        &self,
        secret: Vec<u8>,
        token: Vec<u8>,
        challenge: &Challenge,
    ) -> io::Result<Zeroizing<[u8; 16]>> {
        if secret.len() != 128 || token.len() != 128 {
            return Err(denied());
        }
        let permit = Arc::clone(&self.jobs)
            .acquire_owned()
            .await
            .map_err(|_| unavailable())?;
        let key = self.key.clone();
        let expected = challenge.token;
        tokio::task::spawn_blocking(move || {
            // The permit remains inside the started operation even if its awaiting
            // session is cancelled. wait_idle accounts for all such operations.
            let _permit = permit;
            let rsa = key.rsa().map_err(|_| denied())?;
            let mut clear = Zeroizing::new(vec![0; rsa.size() as usize]);
            let used = rsa
                .private_decrypt(&token, &mut clear, Padding::PKCS1)
                .map_err(|_| denied())?;
            if used != 4 || !openssl::memcmp::eq(&clear[..4], &expected) {
                return Err(denied());
            }
            let used = rsa
                .private_decrypt(&secret, &mut clear, Padding::PKCS1)
                .map_err(|_| denied())?;
            if used != 16 {
                return Err(denied());
            }
            let mut result = Zeroizing::new([0; 16]);
            result.copy_from_slice(&clear[..16]);
            Ok(result)
        })
        .await
        .map_err(|_| unavailable())?
    }
    pub async fn verify(&self, username: &str, shared_key: &[u8; 16]) -> io::Result<Profile> {
        if !valid_name(username) {
            return Err(denied());
        }
        let mut sha = openssl::sha::Sha1::new();
        sha.update(shared_key);
        sha.update(&self.der);
        let digest = signed_hex(sha.finish()); // empty server ID adds no bytes
        let mut url = reqwest::Url::parse(SESSION_URL).map_err(|_| unavailable())?;
        url.query_pairs_mut()
            .append_pair("username", username)
            .append_pair("serverId", &digest)
            .append_pair("unsigned", "false");
        let response = self
            .http
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|_| unavailable())?;
        read_profile(response, username).await
    }
    pub async fn wait_idle(&self) -> io::Result<()> {
        let _all = tokio::time::timeout(
            Duration::from_secs(3),
            self.jobs.acquire_many(RSA_CONCURRENCY as u32),
        )
        .await
        .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?
        .map_err(|_| unavailable())?;
        Ok(())
    }
}

async fn read_profile(mut response: reqwest::Response, username: &str) -> io::Result<Profile> {
    if response.status() != reqwest::StatusCode::OK {
        return Err(if response.status() == reqwest::StatusCode::NO_CONTENT {
            denied()
        } else {
            unavailable()
        });
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_RESPONSE as u64)
    {
        return Err(denied());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
        if body.len() + chunk.len() > MAX_RESPONSE {
            return Err(denied());
        }
        body.extend_from_slice(&chunk);
    }
    parse_profile(&body, username)
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 16
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn bounded_string(value: &serde_json::Value, field: &str, units: usize) -> io::Result<String> {
    let text = value[field].as_str().ok_or_else(denied)?;
    if text.len() > units * 3 || text.encode_utf16().count() > units {
        return Err(denied());
    }
    Ok(text.to_owned())
}

fn parse_profile(bytes: &[u8], requested: &str) -> io::Result<Profile> {
    let json: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| denied())?;
    let name = bounded_string(&json, "name", 16)?;
    if !valid_name(&name) || !name.eq_ignore_ascii_case(requested) {
        return Err(denied());
    }
    let id = json["id"].as_str().ok_or_else(denied)?;
    if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(denied());
    }
    let mut uuid = [0; 16];
    for (index, byte) in uuid.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&id[index * 2..index * 2 + 2], 16).map_err(|_| denied())?;
    }
    let props = json["properties"].as_array().ok_or_else(denied)?;
    if props.len() > 16 {
        return Err(denied());
    }
    let mut properties = Vec::with_capacity(props.len());
    for prop in props {
        let name = bounded_string(prop, "name", 64)?;
        let value = bounded_string(prop, "value", 32767)?;
        let signature = if prop.get("signature").is_some() {
            Some(bounded_string(prop, "signature", 1024)?)
        } else {
            None
        };
        properties.push(Property {
            name,
            value,
            signature,
        });
    }
    Ok(Profile {
        uuid,
        name,
        properties,
    })
}

/// Java BigInteger(byte[]).toString(16): signed two's-complement, no leading zero.
pub fn signed_hex(mut bytes: [u8; 20]) -> String {
    let negative = bytes[0] & 0x80 != 0;
    if negative {
        let mut carry = 1u16;
        for byte in bytes.iter_mut().rev() {
            let value = u16::from(!*byte) + carry;
            *byte = value as u8;
            carry = value >> 8;
        }
    }
    let full = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let trimmed = full.trim_start_matches('0');
    let magnitude = if trimmed.is_empty() { "0" } else { trimmed };
    format!("{}{magnitude}", if negative { "-" } else { "" })
}

pub fn random_uuid() -> io::Result<[u8; 16]> {
    let mut uuid = [0; 16];
    rand_bytes(&mut uuid).map_err(|_| unavailable())?;
    uuid[6] = (uuid[6] & 0x0f) | 0x40;
    uuid[8] = (uuid[8] & 0x3f) | 0x80;
    Ok(uuid)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_digest_matches_published_java_examples() {
        for (name, expected) in [
            ("Notch", "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48"),
            ("jeb_", "-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1"),
            ("simon", "88e16a1019277b15d58faf0541e11910eb756f6"),
        ] {
            assert_eq!(signed_hex(openssl::sha::sha1(name.as_bytes())), expected);
        }
        assert_eq!(signed_hex([0; 20]), "0");
        assert_eq!(signed_hex([0xff; 20]), "-1");
    }
    #[tokio::test]
    async fn challenge_crypto_and_permits() {
        let auth = Auth::new().unwrap();
        let challenge = auth.begin().unwrap();
        let public = Rsa::public_key_from_der(&auth.der).unwrap();
        let key = [9; 16];
        let encrypt = |data: &[u8]| {
            let mut out = vec![0; 128];
            let used = public
                .public_encrypt(data, &mut out, Padding::PKCS1)
                .unwrap();
            out.truncate(used);
            out
        };
        let actual = auth
            .decrypt(encrypt(&key), encrypt(&challenge.token), &challenge)
            .await
            .unwrap();
        assert_eq!(&*actual, &key);
        assert!(
            auth.decrypt(encrypt(&key), encrypt(&[1, 2, 3, 4]), &challenge)
                .await
                .is_err()
        );
        assert!(
            auth.decrypt(encrypt(&[0; 15]), encrypt(&challenge.token), &challenge)
                .await
                .is_err()
        );
        let mut holds = vec![challenge];
        for _ in 1..AUTH_CONCURRENCY {
            holds.push(auth.begin().unwrap());
        }
        assert!(auth.begin().is_err());
        drop(holds);
        assert_eq!(auth.in_flight(), 0);
        auth.wait_idle().await.unwrap();
    }
    #[test]
    fn verification_response_bounds_and_identity() {
        let good = br#"{"id":"0123456789abcdef0123456789abcdef","name":"Test","properties":[]}"#;
        let profile = parse_profile(good, "test").unwrap();
        assert_eq!(profile.uuid[0], 1);
        assert!(parse_profile(good, "other").is_err());
        assert!(parse_profile(b"{}", "Test").is_err());
        assert!(parse_profile(br#"{"id":"bad","name":"Test","properties":[]}"#, "Test").is_err());
    }

    #[tokio::test]
    async fn http_status_body_limits_and_malformed_json_fail_closed() {
        for status in [204, 401, 429, 500, 503] {
            let response = reqwest::Response::from(
                http::Response::builder().status(status).body("{}").unwrap(),
            );
            assert!(read_profile(response, "Test").await.is_err());
        }
        for body in ["not json".to_owned(), " ".repeat(MAX_RESPONSE + 1)] {
            let response =
                reqwest::Response::from(http::Response::builder().status(200).body(body).unwrap());
            assert!(read_profile(response, "Test").await.is_err());
        }
        let response = reqwest::Response::from(
            http::Response::builder()
                .status(200)
                .header("Content-Length", MAX_RESPONSE + 1)
                .body("{}")
                .unwrap(),
        );
        assert!(read_profile(response, "Test").await.is_err());
    }

    #[test]
    fn profile_properties_are_bounded_before_serialization() {
        let base = serde_json::json!({"id":"0123456789abcdef0123456789abcdef", "name":"Test",
            "properties":[{"name":"textures", "value":"opaque", "signature":"signature"}]});
        assert!(parse_profile(&serde_json::to_vec(&base).unwrap(), "Test").is_ok());
        let mut oversized = base.clone();
        oversized["properties"][0]["signature"] = serde_json::Value::String("a".repeat(1025));
        assert!(parse_profile(&serde_json::to_vec(&oversized).unwrap(), "Test").is_err());
        oversized["properties"] = serde_json::Value::Array(vec![base["properties"][0].clone(); 17]);
        assert!(parse_profile(&serde_json::to_vec(&oversized).unwrap(), "Test").is_err());
    }
}
