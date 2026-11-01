//! Protocol 777 M1A only. No sockets, world state, authentication or Java runtime.
//! Wire provenance and policy differences: docs/STAGE1_PROTOCOL_EVIDENCE.md.

use std::{error::Error, fmt};

pub const PROTOCOL_VERSION: i32 = 777;
pub const MINECRAFT_VERSION: &str = "26.3";
/// Stage-specific inbound budget, below the wire's 2^21 - 1 byte ceiling.
pub const MAX_INBOUND_FRAME: usize = 1024;
pub const MAX_OUTBOUND_FRAME: usize = 4096;
pub mod login;
pub mod play;
pub mod presence;
pub mod pvp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    Truncated,
    VarIntOverflow,
    InvalidLength,
    InvalidUtf8,
    StringTooLong,
    UnexpectedPacket,
    InvalidIntent,
    TrailingBytes,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for ProtocolError {}

/// Returns (signed value, consumed bytes). Nonminimal encodings are accepted.
pub fn decode_varint(input: &[u8]) -> Result<(i32, usize), ProtocolError> {
    let mut value = 0u32;
    for index in 0..5 {
        let byte = *input.get(index).ok_or(ProtocolError::Truncated)?;
        if index == 4 && byte & 0xf0 != 0 {
            return Err(ProtocolError::VarIntOverflow);
        }
        value |= u32::from(byte & 0x7f) << (7 * index);
        if byte & 0x80 == 0 {
            return Ok((value as i32, index + 1));
        }
    }
    Err(ProtocolError::VarIntOverflow)
}

pub fn encode_varint(value: i32, output: &mut Vec<u8>) {
    let mut value = value as u32;
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        output.push(byte | if value == 0 { 0 } else { 0x80 });
        if value == 0 {
            break;
        }
    }
}

/// Decode the outer, at-most-three-byte VarInt, before allocating a payload.
pub fn decode_frame_length(prefix: &[u8]) -> Result<usize, ProtocolError> {
    if prefix.len() > 3 {
        return Err(ProtocolError::InvalidLength);
    }
    let (length, used) = decode_varint(prefix)?;
    if used != prefix.len() || !(1..=MAX_INBOUND_FRAME as i32).contains(&length) {
        return Err(ProtocolError::InvalidLength);
    }
    Ok(length as usize)
}

pub struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }
    pub fn remaining(&self) -> &'a [u8] {
        self.0
    }
    pub fn varint(&mut self) -> Result<i32, ProtocolError> {
        let (value, used) = decode_varint(self.0)?;
        self.0 = &self.0[used..];
        Ok(value)
    }

    pub fn take(&mut self, length: usize) -> Result<&'a [u8], ProtocolError> {
        if length > self.0.len() {
            return Err(ProtocolError::Truncated);
        }
        let (head, tail) = self.0.split_at(length);
        self.0 = tail;
        Ok(head)
    }

    fn hostname(&mut self) -> Result<&'a str, ProtocolError> {
        self.string(255)
    }
    pub fn string(&mut self, max: usize) -> Result<&'a str, ProtocolError> {
        let length = self.varint()?;
        // Minecraft strings use a byte budget of max UTF-16 units * 3.
        if length < 0 || length as usize > max.saturating_mul(3) {
            return Err(ProtocolError::StringTooLong);
        }
        let value = std::str::from_utf8(self.take(length as usize)?)
            .map_err(|_| ProtocolError::InvalidUtf8)?;
        if value.encode_utf16().count() > max {
            return Err(ProtocolError::StringTooLong);
        }
        Ok(value)
    }

    pub fn byte(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }
    pub fn boolean(&mut self) -> Result<bool, ProtocolError> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(ProtocolError::UnexpectedPacket),
        }
    }
    pub fn array(&mut self, max: usize) -> Result<&'a [u8], ProtocolError> {
        let length = self.varint()?;
        if length < 0 || length as usize > max {
            return Err(ProtocolError::InvalidLength);
        }
        self.take(length as usize)
    }
    pub fn finish(self) -> Result<(), ProtocolError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(ProtocolError::TrailingBytes)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Status,
    Login,
    Transfer,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Handshake {
    pub protocol: i32,
    pub intent: Intent,
}

pub fn decode_handshake(payload: &[u8]) -> Result<Handshake, ProtocolError> {
    let mut reader = Reader(payload);
    if reader.varint()? != 0 {
        return Err(ProtocolError::UnexpectedPacket);
    }
    let protocol = reader.varint()?;
    // Validate, then discard the untrusted host and port. Neither is an identity.
    reader.hostname()?;
    reader.take(2)?;
    let intent = match reader.varint()? {
        1 => Intent::Status,
        2 => Intent::Login,
        3 => Intent::Transfer,
        _ => return Err(ProtocolError::InvalidIntent),
    };
    reader.finish()?;
    Ok(Handshake { protocol, intent })
}

pub fn decode_status_request(payload: &[u8]) -> Result<(), ProtocolError> {
    let mut reader = Reader(payload);
    if reader.varint()? != 0 {
        return Err(ProtocolError::UnexpectedPacket);
    }
    reader.finish()
}

pub fn decode_ping(payload: &[u8]) -> Result<[u8; 8], ProtocolError> {
    let mut reader = Reader(payload);
    if reader.varint()? != 1 {
        return Err(ProtocolError::UnexpectedPacket);
    }
    let mut nonce = [0; 8];
    nonce.copy_from_slice(reader.take(8)?);
    reader.finish()?;
    Ok(nonce)
}

fn string_packet(id: i32, value: &str) -> Vec<u8> {
    let mut payload = Vec::with_capacity(value.len() + 6);
    encode_varint(id, &mut payload);
    encode_varint(value.len() as i32, &mut payload);
    payload.extend_from_slice(value.as_bytes());
    payload
}

pub fn status_response() -> Vec<u8> {
    status_response_with("SmolMC Stage 1: status only (login unavailable)", 0, 0)
}

pub fn status_response_with(description: &str, online: usize, max_players: usize) -> Vec<u8> {
    // Static, honest status: no authenticated players or world capacity in M1A.
    let json = serde_json::json!({
        "version": {"name": MINECRAFT_VERSION, "protocol": PROTOCOL_VERSION},
        "players": {"max": max_players, "online": online, "sample": []},
        "description": {"text": description},
        "enforcesSecureChat": false
    });
    string_packet(0, &json.to_string())
}

pub fn login_rejection(protocol: i32) -> Vec<u8> {
    let reason = if protocol == PROTOCOL_VERSION {
        "SmolMC Stage 1 supports server-list status only. Login is not implemented."
    } else {
        "SmolMC targets Minecraft 26.3 (protocol 777). Stage 1 supports status only."
    };
    string_packet(0, &serde_json::json!({"text": reason}).to_string())
}

pub fn pong(nonce: [u8; 8]) -> Vec<u8> {
    let mut payload = vec![1];
    payload.extend_from_slice(&nonce);
    payload
}

pub fn frame(payload: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    if payload.is_empty() || payload.len() > MAX_OUTBOUND_FRAME {
        return Err(ProtocolError::InvalidLength);
    }
    let mut output = Vec::with_capacity(payload.len() + 3);
    encode_varint(payload.len() as i32, &mut output);
    output.extend_from_slice(payload);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_boundaries_and_overflow() {
        for value in [0, 127, 128, 777, 16383, 16384, i32::MAX, -1, i32::MIN] {
            let mut bytes = Vec::new();
            encode_varint(value, &mut bytes);
            assert_eq!(decode_varint(&bytes), Ok((value, bytes.len())));
        }
        assert_eq!(
            decode_varint(&[0x80; 5]),
            Err(ProtocolError::VarIntOverflow)
        );
        assert_eq!(
            decode_varint(&[0xff; 5]),
            Err(ProtocolError::VarIntOverflow)
        );
        assert_eq!(decode_varint(&[0x80]), Err(ProtocolError::Truncated));
        assert_eq!(decode_varint(&[0x80, 0]), Ok((0, 2)));
    }

    #[test]
    fn frame_budget_checked_before_allocation() {
        assert_eq!(decode_frame_length(&[0]), Err(ProtocolError::InvalidLength));
        assert_eq!(decode_frame_length(&[0x80, 8]), Ok(1024));
        assert_eq!(
            decode_frame_length(&[0x81, 8]),
            Err(ProtocolError::InvalidLength)
        );
        assert_eq!(
            decode_frame_length(&[0x81, 0x80, 0x80, 0]),
            Err(ProtocolError::InvalidLength)
        );
        assert!(frame(&[]).is_err());
        assert!(frame(&vec![0; MAX_OUTBOUND_FRAME + 1]).is_err());
    }

    #[test]
    fn independent_handshake_fixture_and_truncations() {
        // packet id, 777, "localhost", port 25565, status intent.
        let bytes = b"\x00\x89\x06\x09localhost\x63\xdd\x01";
        assert_eq!(
            decode_handshake(bytes),
            Ok(Handshake {
                protocol: 777,
                intent: Intent::Status
            })
        );
        for end in 0..bytes.len() {
            assert!(decode_handshake(&bytes[..end]).is_err());
        }
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert_eq!(
            decode_handshake(&trailing),
            Err(ProtocolError::TrailingBytes)
        );
    }

    #[test]
    fn hostname_checks_utf8_and_utf16_units() {
        fn handshake(host: &[u8]) -> Vec<u8> {
            let mut bytes = vec![0, 0x89, 6];
            encode_varint(host.len() as i32, &mut bytes);
            bytes.extend_from_slice(host);
            bytes.extend_from_slice(&[0x63, 0xdd, 1]);
            bytes
        }
        assert!(decode_handshake(&handshake("界".repeat(255).as_bytes())).is_ok());
        assert!(decode_handshake(&handshake("😀".repeat(127).as_bytes())).is_ok());
        assert_eq!(
            decode_handshake(&handshake("😀".repeat(128).as_bytes())),
            Err(ProtocolError::StringTooLong)
        );
        assert_eq!(
            decode_handshake(&handshake(&[0xff])),
            Err(ProtocolError::InvalidUtf8)
        );
        assert!(decode_handshake(&handshake(&[b'a'; 766])).is_err());
    }

    #[test]
    fn status_and_ping_require_exact_payloads() {
        assert_eq!(decode_status_request(&[0]), Ok(()));
        assert!(decode_status_request(&[0, 0]).is_err());
        assert!(decode_status_request(&[1]).is_err());
        let ping = [1, 0x80, 0, 0, 0, 0, 0, 0, 1];
        assert_eq!(pong(decode_ping(&ping).unwrap()), ping);
        assert!(decode_ping(&ping[..8]).is_err());
        assert!(decode_ping(&[0; 9]).is_err());
    }

    #[test]
    fn arbitrary_bounded_inputs_never_panic() {
        // Reproducible 20k-case smoke corpus; not a substitute for coverage fuzzing.
        let mut seed = 0x12345678u32;
        for case in 0..20_000 {
            let mut bytes = vec![0; case % (MAX_INBOUND_FRAME + 1)];
            for byte in &mut bytes {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                *byte = seed as u8;
            }
            let _ = decode_varint(&bytes);
            let _ = decode_handshake(&bytes);
            let _ = decode_status_request(&bytes);
            let _ = decode_ping(&bytes);
        }
    }
}

pub mod items;
