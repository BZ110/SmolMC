//! Protocol 777 Login/Configuration and minimum Play entry. All IDs/layouts
//! pinned in evidence/m1b-packet-ids.json and docs/M1B_PROTOCOL_EVIDENCE.md.
use crate::{ProtocolError, Reader, encode_varint};

#[derive(Clone, Debug)]
pub struct Property {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Profile {
    pub uuid: [u8; 16],
    pub name: String,
    pub properties: Vec<Property>,
}

pub struct Encoder(Vec<u8>);
impl Encoder {
    pub fn new(id: i32) -> Self {
        let mut out = Self(Vec::new());
        out.varint(id);
        out
    }
    pub fn varint(&mut self, value: i32) {
        encode_varint(value, &mut self.0);
    }
    pub fn string(&mut self, text: &str) {
        self.varint(text.len() as i32);
        self.raw(text.as_bytes());
    }
    pub fn array(&mut self, bytes: &[u8]) {
        self.varint(bytes.len() as i32);
        self.raw(bytes);
    }
    pub fn raw(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
    pub fn byte(&mut self, value: u8) {
        self.0.push(value);
    }
    pub fn boolean(&mut self, value: bool) {
        self.byte(u8::from(value));
    }
    pub fn finish(self) -> Vec<u8> {
        self.0
    }
}

pub fn login_start(bytes: &[u8]) -> Result<(String, [u8; 16]), ProtocolError> {
    let mut reader = Reader::new(bytes);
    if reader.varint()? != 0 {
        return Err(ProtocolError::UnexpectedPacket);
    }
    let name = reader.string(16)?.to_owned();
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return Err(ProtocolError::UnexpectedPacket);
    }
    let mut uuid = [0; 16];
    uuid.copy_from_slice(reader.take(16)?);
    reader.finish()?;
    Ok((name, uuid))
}

pub fn key_response(bytes: &[u8]) -> Result<(Vec<u8>, Vec<u8>), ProtocolError> {
    let mut reader = Reader::new(bytes);
    if reader.varint()? != 1 {
        return Err(ProtocolError::UnexpectedPacket);
    }
    let secret = reader.array(128)?.to_vec();
    let token = reader.array(128)?.to_vec();
    if secret.len() != 128 || token.len() != 128 {
        return Err(ProtocolError::InvalidLength);
    }
    reader.finish()?;
    Ok((secret, token))
}

pub fn empty_packet(bytes: &[u8], id: i32) -> Result<(), ProtocolError> {
    let mut reader = Reader::new(bytes);
    if reader.varint()? != id {
        return Err(ProtocolError::UnexpectedPacket);
    }
    reader.finish()
}

pub fn encryption_request(public_key_der: &[u8], challenge: &[u8; 4]) -> Vec<u8> {
    let mut out = Encoder::new(1);
    out.string("");
    out.array(public_key_der);
    out.array(challenge);
    out.boolean(true);
    out.finish()
}

pub fn compression(threshold: i32) -> Vec<u8> {
    let mut out = Encoder::new(3);
    out.varint(threshold);
    out.finish()
}

pub fn login_finished(profile: &Profile, session_id: &[u8; 16]) -> Vec<u8> {
    let mut out = Encoder::new(2);
    out.raw(&profile.uuid);
    out.string(&profile.name);
    out.varint(profile.properties.len() as i32);
    for prop in &profile.properties {
        out.string(&prop.name);
        out.string(&prop.value);
        out.boolean(prop.signature.is_some());
        if let Some(signature) = &prop.signature {
            out.string(signature);
        }
    }
    out.raw(session_id);
    out.finish()
}

pub fn disconnect_login(reason: &str) -> Vec<u8> {
    let mut out = Encoder::new(0);
    out.string(&serde_json::json!({"text": reason}).to_string());
    out.finish()
}

/// Common/config/play disconnect uses an anonymous NBT string text component.
/// Callers provide only fixed, bounded ASCII diagnostic text.
pub fn disconnect_common(id: i32, reason: &str) -> Vec<u8> {
    let mut out = Encoder::new(id);
    out.byte(8); // TAG_String; network NBT has no root name
    out.raw(&(reason.len() as u16).to_be_bytes());
    out.raw(reason.as_bytes());
    out.finish()
}

pub fn enabled_features() -> Vec<u8> {
    let mut out = Encoder::new(13);
    out.varint(1);
    out.string("minecraft:vanilla");
    out.finish()
}

pub fn select_known_packs() -> Vec<u8> {
    let mut out = Encoder::new(15);
    out.varint(1);
    for value in ["minecraft", "core", "26.3"] {
        out.string(value);
    }
    out.finish()
}

pub fn known_pack_response(bytes: &[u8]) -> Result<bool, ProtocolError> {
    let mut reader = Reader::new(bytes);
    if reader.varint()? != 7 {
        return Err(ProtocolError::UnexpectedPacket);
    }
    let count = reader.varint()?;
    if !(0..=64).contains(&count) {
        return Err(ProtocolError::InvalidLength);
    }
    let mut matches = count == 1;
    for _ in 0..count {
        let namespace = reader.string(32767)?;
        let id = reader.string(32767)?;
        let version = reader.string(32767)?;
        matches &= namespace == "minecraft" && id == "core" && version == "26.3";
    }
    reader.finish()?;
    Ok(matches)
}

/// Zero copied registry assets: only names, with data omitted after core-pack ACK.
pub fn registry_data(registry: &str, entries: &[String]) -> Vec<u8> {
    let mut out = Encoder::new(7);
    out.string(registry);
    out.varint(entries.len() as i32);
    for entry in entries {
        out.string(entry);
        out.boolean(false);
    }
    out.finish()
}

pub fn empty_named_tags(registries: &[(String, Vec<String>)]) -> Vec<u8> {
    let mut out = Encoder::new(14);
    out.varint(registries.len() as i32);
    for (registry, tags) in registries {
        out.string(registry);
        out.varint(tags.len() as i32);
        for tag in tags {
            out.string(tag);
            out.varint(0);
        }
    }
    out.finish()
}

pub fn client_information(bytes: &[u8], id: i32) -> Result<(), ProtocolError> {
    let mut reader = Reader::new(bytes);
    if reader.varint()? != id {
        return Err(ProtocolError::UnexpectedPacket);
    }
    reader.string(16)?;
    reader.byte()?; // signed view-distance byte, a preference rather than authority
    if !(0..=2).contains(&reader.varint()?) {
        return Err(ProtocolError::UnexpectedPacket);
    }
    reader.boolean()?;
    reader.byte()?;
    if !(0..=1).contains(&reader.varint()?) {
        return Err(ProtocolError::UnexpectedPacket);
    }
    reader.boolean()?;
    reader.boolean()?;
    if !(0..=2).contains(&reader.varint()?) {
        return Err(ProtocolError::UnexpectedPacket);
    }
    reader.finish()
}

pub fn custom_payload(bytes: &[u8], id: i32) -> Result<(), ProtocolError> {
    let mut reader = Reader::new(bytes);
    if reader.varint()? != id {
        return Err(ProtocolError::UnexpectedPacket);
    }
    let channel = reader.string(32767)?;
    if channel.is_empty()
        || !channel
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_./:-".contains(&b))
    {
        return Err(ProtocolError::UnexpectedPacket);
    }
    if channel == "minecraft:brand" {
        reader.string(32767)?;
        reader.finish()
    } else if reader.remaining().len() <= 32767 {
        // Verified reference supports bounded opaque payloads; no plugin execution.
        Ok(())
    } else {
        Err(ProtocolError::InvalidLength)
    }
}

pub fn play_login(entity_id: i32, dimension_type: i32, max_players: i32) -> Vec<u8> {
    play_login_with_mode(entity_id, dimension_type, max_players, 2)
}

pub fn play_login_with_mode(
    entity_id: i32,
    dimension_type: i32,
    max_players: i32,
    mode: i32,
) -> Vec<u8> {
    let mut out = Encoder::new(0x32);
    out.raw(&entity_id.to_be_bytes());
    out.boolean(false);
    out.varint(1);
    out.string("smolmc:bootstrap");
    out.varint(max_players);
    out.varint(2);
    out.varint(2);
    out.boolean(false);
    out.boolean(true);
    out.boolean(false);
    out.varint(dimension_type);
    out.string("smolmc:bootstrap");
    out.raw(&0i64.to_be_bytes());
    out.varint(mode);
    out.varint(0); // OptionalInt.empty previous game type (not legacy byte -1)
    out.boolean(false);
    out.boolean(false);
    out.boolean(false);
    out.varint(0);
    out.varint(63); // portal cooldown, sea level
    out.boolean(true);
    out.boolean(false); // online mode; no secure player chat claim
    out.finish()
}

/// An automatic vanilla Play-entry message. Chat remains unsupported: this key
/// is bounded and discarded, never trusted as identity or advertised as verified.
pub fn discard_chat_session(bytes: &[u8]) -> Result<(), ProtocolError> {
    let mut reader = Reader::new(bytes);
    if reader.varint()? != 0x0a {
        return Err(ProtocolError::UnexpectedPacket);
    }
    reader.take(16)?; // chat session UUID
    reader.take(8)?; // expiry epoch milliseconds
    reader.array(512)?; // encoded public key
    reader.array(4096)?; // service signature
    reader.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_start_golden_fixture_and_all_truncations() {
        let fixture = [b"\x00\x04Test".as_slice(), &[0x12; 16]].concat();
        assert_eq!(
            login_start(&fixture).unwrap(),
            ("Test".to_owned(), [0x12; 16])
        );
        for end in 0..fixture.len() {
            assert!(login_start(&fixture[..end]).is_err());
        }
        let mut bad = fixture.clone();
        bad[2] = b'!';
        assert!(login_start(&bad).is_err());
        bad = fixture;
        bad.push(0);
        assert!(login_start(&bad).is_err());
    }

    #[test]
    fn key_response_enforces_rsa_block_bounds() {
        let mut fixture = vec![1, 0x80, 1];
        fixture.extend_from_slice(&[7; 128]);
        fixture.extend_from_slice(&[0x80, 1]);
        fixture.extend_from_slice(&[9; 128]);
        let (secret, token) = key_response(&fixture).unwrap();
        assert_eq!(secret, [7; 128]);
        assert_eq!(token, [9; 128]);
        for end in 0..fixture.len() {
            assert!(key_response(&fixture[..end]).is_err());
        }
        fixture[1] = 0x81;
        assert!(key_response(&fixture).is_err());
    }

    #[test]
    fn known_pack_acknowledgement_requires_exact_version_and_complete_packet() {
        let fixture = b"\x07\x01\x09minecraft\x04core\x0426.3";
        assert_eq!(known_pack_response(fixture), Ok(true));
        assert_eq!(known_pack_response(&[7, 0]), Ok(false));
        let mut changed = fixture.to_vec();
        *changed.last_mut().unwrap() = b'2';
        assert_eq!(known_pack_response(&changed), Ok(false));
        changed.push(0);
        assert!(known_pack_response(&changed).is_err());
        assert!(known_pack_response(&[7, 65]).is_err());
    }

    #[test]
    fn client_information_all_fields_and_invalid_enums() {
        let fixture = b"\x00\x05en_us\x02\x00\x01\x7f\x01\x00\x01\x00";
        assert!(client_information(fixture, 0).is_ok());
        for end in 0..fixture.len() {
            assert!(client_information(&fixture[..end], 0).is_err());
        }
        let mut invalid = fixture.to_vec();
        invalid[8] = 3;
        assert!(client_information(&invalid, 0).is_err());
        assert!(client_information(fixture, 1).is_err());
    }

    #[test]
    fn login_finished_includes_profile_properties_and_separate_session_uuid() {
        let profile = Profile {
            uuid: [1; 16],
            name: "Test".into(),
            properties: vec![Property {
                name: "textures".into(),
                value: "opaque".into(),
                signature: Some("sig".into()),
            }],
        };
        let packet = login_finished(&profile, &[2; 16]);
        let mut reader = Reader::new(&packet);
        assert_eq!(reader.varint().unwrap(), 2);
        assert_eq!(reader.take(16).unwrap(), [1; 16]);
        assert_eq!(reader.string(16).unwrap(), "Test");
        assert_eq!(reader.varint().unwrap(), 1);
        assert_eq!(reader.string(64).unwrap(), "textures");
        assert_eq!(reader.string(32767).unwrap(), "opaque");
        assert!(reader.boolean().unwrap());
        assert_eq!(reader.string(1024).unwrap(), "sig");
        assert_eq!(reader.take(16).unwrap(), [2; 16]);
        reader.finish().unwrap();
    }

    #[test]
    fn common_disconnect_is_anonymous_nbt_and_payloads_are_bounded() {
        assert_eq!(disconnect_common(2, "Test"), b"\x02\x08\x00\x04Test");
        assert!(custom_payload(b"\x02\x0fminecraft:brand\x07vanilla", 2).is_ok());
        assert!(custom_payload(b"\x02\x03BAD", 2).is_err());
        let mut bad = vec![0x0a];
        bad.extend_from_slice(&[0; 24]);
        bad.extend_from_slice(&[0x81, 4]); // oversized encoded public-key array, 513
        assert!(discard_chat_session(&bad).is_err());
    }
}
