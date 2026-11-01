//! Narrow protocol-777 PvP codecs. Provenance: docs/M3_PROTOCOL_EVIDENCE.md.
use crate::{ProtocolError as E, Reader, login::Encoder};
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Attack(i32),
    Dig {
        pos: [i32; 3],
        face: u8,
        action: u8,
        sequence: i32,
    },
    Place {
        pos: [i32; 3],
        face: u8,
        hand: u8,
        sequence: i32,
    },
    Punch,
    Select(u8),
    Swap,
    Drop {
        all: bool,
    },
    Close,
    Release,
    Click {
        state: i32,
        slot: i16,
        button: u8,
        kind: u8,
    },
    Respawn,
    Interact {
        entity: i32,
        hand: u8,
        location: [f64; 3],
    },
    Use {
        hand: u8,
        sequence: i32,
    },
}
fn bounded(r: &mut Reader<'_>, max: i32) -> Result<i32, E> {
    let n = r.varint()?;
    if (0..=max).contains(&n) {
        Ok(n)
    } else {
        Err(E::InvalidLength)
    }
}
fn finite(r: &mut Reader<'_>) -> Result<f32, E> {
    let v = f32::from_be_bytes(r.take(4)?.try_into().unwrap());
    if v.is_finite() {
        Ok(v)
    } else {
        Err(E::UnexpectedPacket)
    }
}
fn hashed(r: &mut Reader<'_>) -> Result<(), E> {
    if r.boolean()? {
        bounded(r, 65535)?;
        bounded(r, 99)?;
        let n = bounded(r, 256)?;
        for _ in 0..n {
            bounded(r, 65535)?;
            r.take(4)?;
        }
        let n = bounded(r, 256)?;
        for _ in 0..n {
            bounded(r, 65535)?;
        }
    }
    Ok(())
}
pub fn decode(bytes: &[u8]) -> Result<Option<Action>, E> {
    let mut r = Reader::new(bytes);
    let id = r.varint()?;
    let a = match id {
        1 => Action::Attack(bounded(&mut r, i32::MAX)?),
        0x2e => Action::Punch,
        0x36 => {
            let v = i16::from_be_bytes(r.take(2)?.try_into().unwrap());
            if !(0..9).contains(&v) {
                return Err(E::UnexpectedPacket);
            }
            Action::Select(v as u8)
        }
        0x0c => {
            if bounded(&mut r, 1)? != 0 {
                return Err(E::UnexpectedPacket);
            }
            Action::Respawn
        }
        0x13 => {
            if bounded(&mut r, 127)? != 0 {
                return Err(E::UnexpectedPacket);
            }
            Action::Close
        }
        0x12 => {
            let container = bounded(&mut r, 127)?;
            let state = bounded(&mut r, i32::MAX)?;
            let slot = i16::from_be_bytes(r.take(2)?.try_into().unwrap());
            let button = r.byte()?;
            let kind = bounded(&mut r, 6)? as u8;
            let n = bounded(&mut r, 128)?;
            for _ in 0..n {
                r.take(2)?;
                hashed(&mut r)?;
            }
            hashed(&mut r)?;
            if container != 0 {
                return Err(E::UnexpectedPacket);
            }
            Action::Click {
                state,
                slot,
                button,
                kind,
            }
        }
        0x1a => {
            let entity = bounded(&mut r, i32::MAX)?;
            let hand = bounded(&mut r, 1)? as u8;
            let location = read_lp(&mut r)?;
            r.boolean()?;
            Action::Interact {
                entity,
                hand,
                location,
            }
        }
        0x42 => {
            let hand = bounded(&mut r, 1)? as u8;
            let pos =
                crate::play::unpack_block_pos(u64::from_be_bytes(r.take(8)?.try_into().unwrap()));
            let face = bounded(&mut r, 5)? as u8;
            for _ in 0..3 {
                let v = finite(&mut r)?;
                if !(-0.001..=1.001).contains(&v) {
                    return Err(E::UnexpectedPacket);
                }
            }
            r.boolean()?;
            r.boolean()?;
            let sequence = bounded(&mut r, i32::MAX)?;
            Action::Place {
                pos,
                face,
                hand,
                sequence,
            }
        }
        0x43 => {
            let hand = bounded(&mut r, 1)? as u8;
            let sequence = bounded(&mut r, i32::MAX)?;
            finite(&mut r)?;
            finite(&mut r)?;
            Action::Use { hand, sequence }
        }
        0x29 => {
            let action = bounded(&mut r, 8)?;
            let pos =
                crate::play::unpack_block_pos(u64::from_be_bytes(r.take(8)?.try_into().unwrap()));
            let face = r.byte()?;
            if face > 5 {
                return Err(E::UnexpectedPacket);
            }
            let sequence = bounded(&mut r, i32::MAX)?;
            match action {
                0..=3 => Action::Dig {
                    pos,
                    face,
                    action: action as u8,
                    sequence,
                },
                7 => Action::Swap,
                4 | 5 => Action::Drop { all: action == 4 },
                6 => Action::Release,
                _ => return Ok(None),
            }
        }
        _ => return Ok(None),
    };
    r.finish()?;
    Ok(Some(a))
}
// LP vector has two little-endian low bytes and a big-endian high u32.
pub fn read_lp(r: &mut Reader<'_>) -> Result<[f64; 3], E> {
    let low = r.byte()?;
    if low == 0 {
        return Ok([0.; 3]);
    }
    let mid = r.byte()?;
    let hi = u32::from_be_bytes(r.take(4)?.try_into().unwrap());
    let bits = (hi as u64) << 16 | (mid as u64) << 8 | low as u64;
    let mut scale = (low & 3) as u64;
    if low & 4 != 0 {
        scale |= (r.varint()? as u32 as u64) << 2;
    }
    let mut v = [0.; 3];
    for (i, shift) in [3, 18, 33].into_iter().enumerate() {
        v[i] = (((bits >> shift) & 32767).min(32766) as f64 * 2. / 32766. - 1.) * scale as f64;
    }
    Ok(v)
}
pub fn write_lp(out: &mut Encoder, v: [f64; 3]) {
    let max = v.iter().map(|x| x.abs()).fold(0., f64::max);
    if max < 1. / 32766. {
        out.byte(0);
        return;
    }
    let scale = max.ceil() as u64;
    let mut bits = scale & 3;
    if scale > 3 {
        bits |= 4;
    }
    for (i, shift) in [3, 18, 33].into_iter().enumerate() {
        let q = ((v[i] / scale as f64 * 0.5 + 0.5) * 32766.).round() as u64;
        bits |= q << shift;
    }
    out.byte(bits as u8);
    out.byte((bits >> 8) as u8);
    out.raw(&((bits >> 16) as u32).to_be_bytes());
    if scale > 3 {
        out.varint((scale >> 2) as i32);
    }
}
pub use crate::items::{Item, stack};
pub fn inventory(state: i32, slots: &[Item; 46], cursor: Item) -> Vec<u8> {
    let mut o = Encoder::new(0x12);
    o.varint(0);
    o.varint(state);
    o.varint(46);
    for item in slots {
        stack(&mut o, *item);
    }
    stack(&mut o, cursor);
    o.finish()
}
pub fn equipment(id: i32, main: Item, off: Item) -> Vec<u8> {
    let mut o = Encoder::new(0x68);
    o.varint(id);
    o.byte(128);
    stack(&mut o, main);
    o.byte(1);
    stack(&mut o, off);
    o.finish()
}
pub fn swing(id: i32, hand: u8) -> Vec<u8> {
    let mut o = Encoder::new(0x7b);
    o.varint(id);
    o.varint(hand as i32);
    o.varint(1);
    o.varint(6);
    o.finish()
}
pub fn health(value: f32) -> Vec<u8> {
    let mut o = Encoder::new(0x6a);
    o.raw(&value.to_be_bytes());
    o.varint(20);
    o.raw(&5f32.to_be_bytes());
    o.finish()
}
pub fn metadata(id: i32, value: f32) -> Vec<u8> {
    let mut o = Encoder::new(0x65);
    o.varint(id);
    o.byte(9);
    o.varint(3);
    o.raw(&value.to_be_bytes());
    o.byte(255);
    o.finish()
}
pub fn damage(id: i32, source: i32, damage_type: i32) -> Vec<u8> {
    let mut o = Encoder::new(0x19);
    o.varint(id);
    o.varint(damage_type);
    o.varint(source + 1);
    o.varint(source + 1);
    o.boolean(false);
    o.finish()
}
pub fn hurt(id: i32, yaw: f32) -> Vec<u8> {
    let mut o = Encoder::new(0x2b);
    o.varint(id);
    o.raw(&yaw.to_be_bytes());
    o.finish()
}
pub fn velocity(id: i32, v: [f64; 3]) -> Vec<u8> {
    let mut o = Encoder::new(0x67);
    o.varint(id);
    write_lp(&mut o, v);
    o.finish()
}
pub fn death(id: i32) -> Vec<u8> {
    let mut o = Encoder::new(0x22);
    o.raw(&id.to_be_bytes());
    o.byte(3);
    o.finish()
}
pub fn killed(id: i32) -> Vec<u8> {
    let mut o = Encoder::new(0x45);
    o.varint(id);
    o.byte(8);
    let text = b"You were defeated in the arena.";
    o.raw(&(text.len() as u16).to_be_bytes());
    o.raw(text);
    o.finish()
}
pub fn respawn(dimension: i32, mode: i32) -> Vec<u8> {
    let mut o = Encoder::new(0x54);
    o.varint(dimension);
    o.string("smolmc:bootstrap");
    o.raw(&0i64.to_be_bytes());
    o.varint(mode);
    o.varint(0);
    o.boolean(false);
    o.boolean(false);
    o.boolean(false);
    o.varint(0);
    o.varint(63);
    o.byte(0);
    o.finish()
}
pub fn abilities() -> Vec<u8> {
    let mut o = Encoder::new(0x41);
    o.byte(0);
    o.raw(&0.05f32.to_be_bytes());
    o.raw(&0.1f32.to_be_bytes());
    o.finish()
}

/// Vanilla sword modifier synchronizes the local player's cooldown indicator.
pub fn attributes(id: i32, main: Item) -> Vec<u8> {
    let mut o = Encoder::new(0x86);
    o.varint(id);
    o.varint(1);
    o.varint(5);
    o.raw(&4f64.to_be_bytes());
    if let Some(modifier) = main.speed_modifier() {
        o.varint(1);
        o.string("minecraft:base_attack_speed");
        o.raw(&(modifier as f64).to_be_bytes());
        o.varint(0);
    } else {
        o.varint(0);
    }
    o.finish()
}
pub fn critical(id: i32) -> Vec<u8> {
    let mut o = Encoder::new(0x02);
    o.varint(id);
    o.byte(1);
    o.finish()
}

/// Inline named sound holder avoids copying the static sound registry. The
/// vanilla holder codec uses tag 0 for an inline sound event.
pub fn sound(name: &str, position: [f64; 3], seed: i64) -> Vec<u8> {
    let mut o = Encoder::new(0x77);
    o.varint(0);
    o.string(name);
    o.boolean(false);
    o.varint(7);
    for v in position {
        o.raw(&((v * 8.) as i32).to_be_bytes());
    }
    o.raw(&1f32.to_be_bytes());
    o.raw(&1f32.to_be_bytes());
    o.raw(&seed.to_be_bytes());
    o.finish()
}
pub fn particles(
    kind: i32,
    position: [f64; 3],
    spread: [f32; 3],
    speed: f32,
    count: i32,
) -> Vec<u8> {
    let mut o = Encoder::new(0x30);
    o.varint(kind);
    o.boolean(false);
    o.boolean(false);
    for v in position {
        o.raw(&v.to_be_bytes());
    }
    for v in spread {
        o.raw(&v.to_be_bytes());
    }
    for _ in 0..3 {
        o.raw(&speed.to_be_bytes());
    }
    o.varint(count);
    o.varint(0);
    o.finish()
}
/// Protocol 777 block destruction animation (actual reference class checked).
pub fn block_destruction(entity: i32, pos: [i32; 3], stage: i32) -> Vec<u8> {
    let mut out = Encoder::new(5);
    out.varint(entity);
    out.raw(&crate::play::pack_block_pos(pos).to_be_bytes());
    out.byte(stage as u8);
    out.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_attacks_punch_slots_respawn_and_offhand() {
        for (packet, a) in [
            (vec![1, 5], Action::Attack(5)),
            (vec![0x2e], Action::Punch),
            (vec![0x36, 0, 8], Action::Select(8)),
            (vec![0xc, 0], Action::Respawn),
            (vec![0x29, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], Action::Swap),
        ] {
            assert_eq!(decode(&packet).unwrap(), Some(a));
            for n in 1..packet.len() {
                assert!(decode(&packet[..n]).is_err());
            }
            let mut trailing = packet;
            trailing.push(0);
            assert!(decode(&trailing).is_err());
        }
        for packet in [
            &[0x36, 0, 9][..],
            &[1, 255, 255, 255, 255, 15],
            &[0x43, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            &[0x12, 0, 0, 0, 36, 0, 0, 129, 1],
        ] {
            assert!(decode(packet).is_err());
        }
    }
    #[test]
    fn hashed_click_predictions_are_bounded_discarded_and_exact() {
        let mut p = Encoder::new(0x12);
        p.varint(0);
        p.varint(3);
        p.raw(&36i16.to_be_bytes());
        p.byte(0);
        p.varint(0);
        p.varint(1);
        p.raw(&36i16.to_be_bytes());
        p.boolean(false);
        p.boolean(true);
        p.varint(1045);
        p.varint(1);
        p.varint(0);
        p.varint(0);
        assert_eq!(
            decode(&p.finish()).unwrap(),
            Some(Action::Click {
                state: 3,
                slot: 36,
                button: 0,
                kind: 0
            })
        );
    }
    #[test]
    fn lp_vectors_match_wire_layout_and_bounded_quantization() {
        for vector in [[0.; 3], [0.4, 0.4, -0.4], [4.5, -2.1, 0.00002]] {
            let mut p = Encoder::new(0);
            write_lp(&mut p, vector);
            let bytes = p.finish();
            let decoded = read_lp(&mut Reader::new(&bytes[1..])).unwrap();
            for i in 0..3 {
                assert!((decoded[i] - vector[i]).abs() < 0.0002);
            }
        }
        assert_eq!(velocity(7, [0.; 3]), [0x67, 7, 0]);
        assert_eq!(swing(7, 1), [0x7b, 7, 1, 1, 6]);
        assert_eq!(metadata(7, 0.), [0x65, 7, 9, 3, 0, 0, 0, 0, 255]);
    }
}
