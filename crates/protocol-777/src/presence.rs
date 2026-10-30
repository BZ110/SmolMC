//! M1D-only protocol-777 fields. Evidence: docs/M1D_PROTOCOL_EVIDENCE.md.
use crate::{
    ProtocolError, Reader,
    login::{Encoder, Profile},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntitySnapshot {
    pub position: [f64; 3],
    pub rotation: [f32; 2], // yaw, pitch
    pub on_ground: bool,
    pub crouching: bool,
    pub sprinting: bool,
}

pub fn chat(bytes: &[u8]) -> Result<String, ProtocolError> {
    let mut r = Reader::new(bytes);
    if r.varint()? != 0x09 {
        return Err(ProtocolError::UnexpectedPacket);
    }
    let text = r.string(256)?;
    if text.trim().is_empty() || text.chars().any(|c| c.is_control() || c == '§') {
        return Err(ProtocolError::UnexpectedPacket);
    }
    r.take(8)?; // epoch milliseconds; not a secure-chat validation claim
    r.take(8)?; // salt
    if r.boolean()? {
        r.take(256)?;
    } // fixed signature, never relayed as trusted
    // We send no signed player chat, so there are no signatures to acknowledge.
    if r.varint()? != 0 || r.take(3)? != [0, 0, 0] {
        return Err(ProtocolError::UnexpectedPacket);
    }
    r.byte()?; // last-seen checksum; no signed chain advertised
    r.finish()?;
    Ok(text.to_owned())
}

pub fn player_info(profile: &Profile) -> Vec<u8> {
    let mut e = Encoder::new(0x47);
    e.byte(0x8f); // ADD, INITIALIZE_CHAT, GAME_MODE, LISTED, HAT; fixed 8-bit enum set
    e.varint(1);
    e.raw(&profile.uuid);
    e.string(&profile.name);
    e.varint(profile.properties.len() as i32);
    for p in &profile.properties {
        e.string(&p.name);
        e.string(&p.value);
        e.boolean(p.signature.is_some());
        if let Some(s) = &p.signature {
            e.string(s);
        }
    }
    e.boolean(false); // no remote signed-chat session/key
    e.varint(1); // creative
    e.boolean(true); // listed
    e.boolean(true); // hat
    e.finish()
}
pub fn player_remove(uuid: &[u8; 16]) -> Vec<u8> {
    let mut e = Encoder::new(0x46);
    e.varint(1);
    e.raw(uuid);
    e.finish()
}
pub fn entity_remove(id: i32) -> Vec<u8> {
    let mut e = Encoder::new(0x4e);
    e.varint(1);
    e.varint(id);
    e.finish()
}
fn angle(value: f32) -> u8 {
    (f64::from(value).mul_add(256.0 / 360.0, 0.0).floor() as i32) as u8
}
pub fn spawn(id: i32, uuid: &[u8; 16], state: EntitySnapshot) -> Vec<u8> {
    let mut e = Encoder::new(0x01);
    e.varint(id);
    e.raw(uuid);
    e.varint(159);
    for v in state.position {
        e.raw(&v.to_be_bytes());
    }
    e.byte(0); // LPVec3 ZERO, not three legacy velocity shorts
    e.byte(angle(state.rotation[1]));
    e.byte(angle(state.rotation[0]));
    e.byte(angle(state.rotation[0]));
    e.varint(0);
    e.finish()
}
pub fn position(id: i32, state: EntitySnapshot) -> Vec<u8> {
    let mut e = Encoder::new(0x23);
    e.varint(id);
    e.varint(0); // PositionPath.LINEAR
    for v in state.position {
        e.raw(&v.to_be_bytes());
    }
    for v in state.rotation {
        e.raw(&v.to_be_bytes());
    }
    e.boolean(state.on_ground);
    e.finish()
}
pub fn head(id: i32, yaw: f32) -> Vec<u8> {
    let mut e = Encoder::new(0x55);
    e.varint(id);
    e.byte(angle(yaw));
    e.finish()
}
pub fn pose(id: i32, state: EntitySnapshot) -> Vec<u8> {
    let mut e = Encoder::new(0x65);
    e.varint(id);
    e.byte(0);
    e.varint(0);
    e.byte(if state.crouching { 2 } else { 0 } | if state.sprinting { 8 } else { 0 });
    e.byte(6);
    e.varint(20);
    e.varint(if state.crouching { 5 } else { 0 });
    e.byte(255);
    e.finish()
}

/// Anonymous NBT TAG_String. Java NBT uses modified UTF-8 over UTF-16 code units,
/// including surrogate pairs, rather than the packet String's UTF-8 encoding.
pub fn system(text: &str) -> Result<Vec<u8>, ProtocolError> {
    let mut bytes = Vec::with_capacity(text.len());
    for u in text.encode_utf16() {
        match u {
            1..=0x7f => bytes.push(u as u8),
            0..=0x7ff => {
                bytes.push(0xc0 | (u >> 6) as u8);
                bytes.push(0x80 | (u & 63) as u8);
            }
            _ => {
                bytes.push(0xe0 | (u >> 12) as u8);
                bytes.push(0x80 | ((u >> 6) & 63) as u8);
                bytes.push(0x80 | (u & 63) as u8);
            }
        }
    }
    let len = u16::try_from(bytes.len()).map_err(|_| ProtocolError::StringTooLong)?;
    let mut e = Encoder::new(0x7c);
    e.byte(8);
    e.raw(&len.to_be_bytes());
    e.raw(&bytes);
    e.boolean(false);
    Ok(e.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_adversarial_play_inputs_never_panic() {
        let mut state = 0x98a5_1132u32;
        for n in 0..20000 {
            let len = n % 1024;
            let mut bytes = Vec::with_capacity(len);
            for _ in 0..len {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                bytes.push((state >> 24) as u8);
            }
            let _ = crate::play::decode(&bytes);
        }
        assert_eq!(
            crate::play::decode(&[1, 7]),
            Ok(crate::play::Event::Ignored)
        );
        assert!(crate::play::decode(&[1]).is_err());
        assert!(crate::play::decode(&[1, 0xff, 0xff, 0xff, 0xff, 15]).is_err());
        assert!(crate::play::decode(&[1, 7, 0]).is_err());
    }
    fn packet(text: &str, signed: bool) -> Vec<u8> {
        let mut e = Encoder::new(9);
        e.string(text);
        e.raw(&[0; 16]);
        e.boolean(signed);
        if signed {
            e.raw(&[7; 256]);
        }
        e.varint(0);
        e.raw(&[0; 3]);
        e.byte(1);
        e.finish()
    }
    #[test]
    fn chat_bounds_complete_fields_and_malformed_cases() {
        for signed in [false, true] {
            let b = packet("hello 😀", signed);
            assert_eq!(chat(&b).unwrap(), "hello 😀");
            for n in 0..b.len() {
                assert!(chat(&b[..n]).is_err());
            }
            let mut trailing = b;
            trailing.push(0);
            assert!(chat(&trailing).is_err());
        }
        for text in ["", " ", "fake\nname", "§aGreen"] {
            assert!(chat(&packet(text, false)).is_err());
        }
        assert!(chat(&packet(&"a".repeat(257), false)).is_err());
        let mut b = packet("ok", false);
        let n = b.len();
        b[n - 5] = 1;
        assert!(chat(&b).is_err());
    }
    #[test]
    fn java_modified_utf8_literal_and_wire_controls() {
        assert_eq!(
            system("A\0😀").unwrap(),
            [
                0x7c, 8, 0, 9, 65, 0xc0, 0x80, 0xed, 0xa0, 0xbd, 0xed, 0xb8, 0x80, 0
            ]
        );
        let state = EntitySnapshot {
            position: [1., 65., -2.],
            rotation: [90., -45.],
            on_ground: true,
            crouching: true,
            sprinting: false,
        };
        let b = spawn(7, &[1; 16], state);
        assert_eq!(&b[18..20], &[0x9f, 1]);
        assert_eq!(&b[b.len() - 5..], &[0, 224, 64, 64, 0]);
        let b = position(7, state);
        assert_eq!(&b[..3], &[0x23, 7, 0]);
        assert_eq!(b.len(), 36);
        assert_eq!(head(7, -90.), [0x55, 7, 192]);
        assert_eq!(pose(7, state), [0x65, 7, 0, 0, 2, 6, 20, 5, 255]);
        assert_eq!(entity_remove(7), [0x4e, 1, 7]);
    }
}
