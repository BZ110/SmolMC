//! Protocol-777 M1C packet layouts; see the version-specific evidence ledger.
use crate::{
    ProtocolError, Reader,
    login::{self, Encoder},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Movement {
    pub position: Option<[f64; 3]>,
    pub rotation: Option<[f32; 2]>,
    pub flags: u8,
}
#[derive(Debug, PartialEq)]
pub enum Event {
    Teleport {
        id: i32,
        position: [f64; 3],
        rotation: [f32; 2],
    },
    KeepAlive(i64),
    Batch(f32),
    Move(Movement),
    Flying(bool),
    PlayerCommand {
        entity: i32,
        action: i32,
    },
    ReadOnlyAction {
        pos: [i32; 3],
        sequence: i32,
    },
    Input(u8),
    Chat(String),
    Ignored,
}
fn double(r: &mut Reader<'_>) -> Result<f64, ProtocolError> {
    let value = f64::from_be_bytes(
        r.take(8)?
            .try_into()
            .map_err(|_| ProtocolError::Truncated)?,
    );
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ProtocolError::UnexpectedPacket)
    }
}
fn float(r: &mut Reader<'_>) -> Result<f32, ProtocolError> {
    let value = f32::from_be_bytes(
        r.take(4)?
            .try_into()
            .map_err(|_| ProtocolError::Truncated)?,
    );
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ProtocolError::UnexpectedPacket)
    }
}
fn long(r: &mut Reader<'_>) -> Result<i64, ProtocolError> {
    Ok(i64::from_be_bytes(
        r.take(8)?
            .try_into()
            .map_err(|_| ProtocolError::Truncated)?,
    ))
}

pub fn decode(bytes: &[u8]) -> Result<Event, ProtocolError> {
    let mut r = Reader::new(bytes);
    let id = r.varint()?;
    let event = match id {
        0x01 => {
            if r.varint()? < 0 {
                return Err(ProtocolError::UnexpectedPacket);
            }
            Event::Ignored // attack has no effect in the M1D read-only profile
        }
        0x09 => return crate::presence::chat(bytes).map(Event::Chat),
        0x06 => {
            if r.varint()? != 0 {
                return Err(ProtocolError::UnexpectedPacket);
            }
            Event::Ignored
        }
        0x00 => Event::Teleport {
            id: r.varint()?,
            position: [double(&mut r)?, double(&mut r)?, double(&mut r)?],
            rotation: [float(&mut r)?, float(&mut r)?],
        },
        0x1c => Event::KeepAlive(long(&mut r)?),
        0x0b => {
            let value = float(&mut r)?;
            if value <= 0.0 {
                return Err(ProtocolError::UnexpectedPacket);
            }
            Event::Batch(value)
        }
        0x1e..=0x21 => {
            let position = if id == 0x1e || id == 0x1f {
                Some([double(&mut r)?, double(&mut r)?, double(&mut r)?])
            } else {
                None
            };
            let rotation = if id == 0x1f || id == 0x20 {
                Some([float(&mut r)?, float(&mut r)?])
            } else {
                None
            };
            let flags = r.byte()?;
            if flags & !3 != 0 {
                return Err(ProtocolError::UnexpectedPacket);
            }
            Event::Move(Movement {
                position,
                rotation,
                flags,
            })
        }
        0x28 => {
            let flags = r.byte()?;
            if flags & !2 != 0 {
                return Err(ProtocolError::UnexpectedPacket);
            }
            Event::Flying(flags & 2 != 0)
        }
        0x2a => {
            let entity = r.varint()?;
            let action = r.varint()?;
            let data = r.varint()?;
            if !(0..=6).contains(&action) || data < 0 {
                return Err(ProtocolError::UnexpectedPacket);
            }
            Event::PlayerCommand { entity, action }
        }
        0x29 => {
            let action = r.varint()?;
            let pos = unpack_block_pos(long(&mut r)? as u64);
            let direction = r.byte()?;
            let sequence = r.varint()?;
            if !(0..=8).contains(&action) || direction > 5 || sequence < 0 {
                return Err(ProtocolError::UnexpectedPacket);
            }
            Event::ReadOnlyAction { pos, sequence }
        }
        0x2b => {
            let flags = r.byte()?;
            if flags & 0x80 != 0 {
                return Err(ProtocolError::UnexpectedPacket);
            }
            Event::Input(flags)
        }
        0x36 => {
            let slot = i16::from_be_bytes(
                r.take(2)?
                    .try_into()
                    .map_err(|_| ProtocolError::Truncated)?,
            );
            if !(0..9).contains(&slot) {
                return Err(ProtocolError::UnexpectedPacket);
            }
            Event::Ignored
        }
        0x13 => {
            r.varint()?;
            Event::Ignored
        } // container ID is VarInt in protocol 777
        0x0d | 0x2c | 0x2e => Event::Ignored,
        0x0a => {
            login::discard_chat_session(bytes)?;
            return Ok(Event::Ignored);
        }
        0x0e => {
            login::client_information(bytes, id)?;
            return Ok(Event::Ignored);
        }
        0x16 => {
            login::custom_payload(bytes, id)?;
            return Ok(Event::Ignored);
        }
        _ => return Err(ProtocolError::UnexpectedPacket),
    };
    r.finish()?;
    Ok(event)
}

pub fn pack_block_pos(pos: [i32; 3]) -> u64 {
    ((pos[0] as u64 & 0x3ffffff) << 38)
        | ((pos[2] as u64 & 0x3ffffff) << 12)
        | (pos[1] as u64 & 0xfff)
}
pub fn unpack_block_pos(value: u64) -> [i32; 3] {
    [
        (value as i64 >> 38) as i32,
        ((value << 52) as i64 >> 52) as i32,
        ((value << 26) as i64 >> 38) as i32,
    ]
}
pub fn center(x: i32, z: i32) -> Vec<u8> {
    let mut out = Encoder::new(0x60);
    out.varint(x);
    out.varint(z);
    out.finish()
}
pub fn unload(x: i32, z: i32) -> Vec<u8> {
    let mut out = Encoder::new(0x26);
    out.raw(&z.to_be_bytes());
    out.raw(&x.to_be_bytes());
    out.finish()
}
pub fn batch_start() -> Vec<u8> {
    vec![0x0c]
}
pub fn batch_finished(size: i32) -> Vec<u8> {
    let mut out = Encoder::new(0x0b);
    out.varint(size);
    out.finish()
}
pub fn abilities(flying: bool) -> Vec<u8> {
    let mut out = Encoder::new(0x41);
    out.byte(0x0d | if flying { 2 } else { 0 });
    out.raw(&0.05f32.to_be_bytes());
    out.raw(&0.1f32.to_be_bytes());
    out.finish()
}
pub fn position(id: i32, pos: [f64; 3], rotation: [f32; 2]) -> Vec<u8> {
    let mut out = Encoder::new(0x49);
    out.varint(id);
    for value in pos {
        out.raw(&value.to_be_bytes());
    }
    for _ in 0..3 {
        out.raw(&0f64.to_be_bytes());
    }
    for value in rotation {
        out.raw(&value.to_be_bytes());
    }
    out.raw(&0i32.to_be_bytes());
    out.finish() // no relative flags
}
pub fn default_spawn(pos: [i32; 3]) -> Vec<u8> {
    let mut out = Encoder::new(0x63);
    out.string("smolmc:bootstrap");
    out.raw(&pack_block_pos(pos).to_be_bytes());
    out.raw(&0f32.to_be_bytes());
    out.raw(&0f32.to_be_bytes());
    out.finish()
}
pub fn start_loading() -> Vec<u8> {
    let mut out = Encoder::new(0x27);
    out.byte(13);
    out.raw(&0f32.to_be_bytes());
    out.finish()
}
pub fn keep_alive(nonce: i64) -> Vec<u8> {
    let mut out = Encoder::new(0x2d);
    out.raw(&nonce.to_be_bytes());
    out.finish()
}
pub fn static_time(clock_id: i32) -> Vec<u8> {
    let mut out = Encoder::new(0x73);
    out.raw(&0i64.to_be_bytes());
    out.varint(1);
    out.varint(clock_id);
    out.varint(6000); // positive 6000 encoded identically as VarLong (frozen noon)
    out.raw(&0f32.to_be_bytes());
    out.raw(&0f32.to_be_bytes());
    out.finish()
}
pub fn block_correction(pos: [i32; 3], state: i32) -> Vec<u8> {
    let mut out = Encoder::new(8);
    out.raw(&pack_block_pos(pos).to_be_bytes());
    out.varint(state);
    out.finish()
}
pub fn block_ack(sequence: i32) -> Vec<u8> {
    let mut out = Encoder::new(4);
    out.varint(sequence);
    out.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn movement_fields_and_invalid_floats() {
        let mut b = vec![0x1f];
        for v in [1f64, 65.0, -1.0] {
            b.extend_from_slice(&v.to_be_bytes());
        }
        for v in [90f32, -45.0] {
            b.extend_from_slice(&v.to_be_bytes());
        }
        b.push(3);
        assert_eq!(
            decode(&b).unwrap(),
            Event::Move(Movement {
                position: Some([1.0, 65.0, -1.0]),
                rotation: Some([90.0, -45.0]),
                flags: 3
            })
        );
        for end in 0..b.len() {
            assert!(decode(&b[..end]).is_err());
        }
        b[1..9].copy_from_slice(&f64::NAN.to_be_bytes());
        assert!(decode(&b).is_err());
        assert!(decode(&[0x21, 4]).is_err());
        assert!(decode(&[0x28, 4]).is_err());
    }
    #[test]
    fn coordinate_and_control_wire_fixtures() {
        for pos in [[0, 64, 0], [-1, -64, -16], [100000, 319, -100000]] {
            assert_eq!(unpack_block_pos(pack_block_pos(pos)), pos);
        }
        assert_eq!(unload(1, -2), [0x26, 0xff, 0xff, 0xff, 0xfe, 0, 0, 0, 1]);
        assert_eq!(center(-1, 0), [0x60, 0xff, 0xff, 0xff, 0xff, 0x0f, 0]);
        let mut teleport = vec![0, 1];
        for value in [8f64, 65.0, 8.0] {
            teleport.extend_from_slice(&value.to_be_bytes());
        }
        teleport.extend_from_slice(&[0; 8]);
        assert_eq!(
            decode(&teleport).unwrap(),
            Event::Teleport {
                id: 1,
                position: [8.0, 65.0, 8.0],
                rotation: [0.0; 2]
            }
        );
        for end in 0..teleport.len() {
            assert!(decode(&teleport[..end]).is_err());
        }
        assert_eq!(
            decode(&[0x1c, 0, 0, 0, 0, 0, 0, 0, 5]).unwrap(),
            Event::KeepAlive(5)
        );
        let mut batch = vec![0x0b];
        batch.extend_from_slice(&f32::INFINITY.to_be_bytes());
        assert!(decode(&batch).is_err());
        batch[1..].copy_from_slice(&0f32.to_be_bytes());
        assert!(decode(&batch).is_err());
    }
}
