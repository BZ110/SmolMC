//! One immutable coordinate-independent chunk suffix for the M1C virtual stone
//! template. No dense blocks, per-coordinate resident chunks, cache or mutation.
use smolmc_protocol_777::login::Encoder;
use smolmc_world::{ChunkPos, HEIGHT, MIN_Y, SURFACE};

pub const AIR_STATE: i32 = 0;
pub const STONE_STATE: i32 = 1;
pub const CHUNK_PACKET_ID: i32 = 0x2e;

pub struct StoneChunk {
    suffix: Box<[u8]>,
}
impl StoneChunk {
    pub fn new(biome_id: i32) -> Self {
        assert!(biome_id >= 0);
        let mut out = Encoder::new(3); // count of typed heightmaps, not packet ID/NBT
        // WORLD_SURFACE=1, MOTION_BLOCKING=4, MOTION_BLOCKING_NO_LEAVES=5.
        // 9 bits/value; entries do not straddle longs, so 7 entries/long, 37 longs.
        for id in [1, 4, 5] {
            out.varint(id);
            out.varint(37);
            for index in 0..37 {
                let count = 7.min(256 - index * 7);
                let word = (0..count).fold(0u64, |word, slot| {
                    word | (((SURFACE - MIN_Y) as u64) << (slot * 9))
                });
                out.raw(&word.to_be_bytes());
            }
        }
        let mut sections = Vec::with_capacity(24 * 10);
        for section in 0..HEIGHT / 16 {
            let stone = MIN_Y + section * 16 < SURFACE;
            sections.extend_from_slice(&(if stone { 4096i16 } else { 0 }).to_be_bytes());
            sections.extend_from_slice(&0i16.to_be_bytes()); // fluid count (777)
            sections.push(0); // singleton block palette: no storage/count follows
            smolmc_protocol_777::encode_varint(
                if stone { STONE_STATE } else { AIR_STATE },
                &mut sections,
            );
            sections.push(0); // singleton biome palette
            smolmc_protocol_777::encode_varint(biome_id, &mut sections);
        }
        out.array(&sections);
        out.varint(0); // no block entities
        // 26 light sections: bottom sentinel (-80), 24 geometry sections, top (320).
        let all = (1u64 << 26) - 1;
        let dark = (1u64 << 9) - 1; // bottom sentinel + eight opaque stone sections
        for mask in [all ^ dark, 0, dark, all] {
            // Protocol 777 BIT_SET is a byte array from BitSet.toByteArray():
            // little-endian bits, trailing zero bytes trimmed. It is NOT longs.
            let bytes = mask.to_le_bytes();
            let length = bytes
                .iter()
                .rposition(|&byte| byte != 0)
                .map_or(0, |index| index + 1);
            out.array(&bytes[..length]);
        }
        out.varint(17);
        let full_light = [0xff; 2048];
        for _ in 0..17 {
            out.array(&full_light);
        }
        out.varint(0); // no nonzero block-light arrays
        Self {
            suffix: out.finish().into_boxed_slice(),
        }
    }
    pub fn shared_bytes(&self) -> usize {
        self.suffix.len()
    }
    /// Caller reuses one scratch allocation per active stream. Identical suffix
    /// remains shared; only packet ID and the coordinate header vary.
    pub fn encode(&self, pos: ChunkPos, out: &mut Vec<u8>) {
        out.clear();
        out.push(CHUNK_PACKET_ID as u8);
        out.extend_from_slice(&pos.x.to_be_bytes());
        out.extend_from_slice(&pos.z.to_be_bytes());
        out.extend_from_slice(&self.suffix);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smolmc_protocol_777::Reader;
    #[test]
    fn independent_chunk_decode_checks_geometry_heightmaps_and_light() {
        let template = StoneChunk::new(42);
        let mut bytes = Vec::new();
        template.encode(ChunkPos { x: -3, z: 7 }, &mut bytes);
        let mut r = Reader::new(&bytes);
        assert_eq!(r.varint().unwrap(), 0x2e);
        assert_eq!(r.take(4).unwrap(), (-3i32).to_be_bytes());
        assert_eq!(r.take(4).unwrap(), 7i32.to_be_bytes());
        assert_eq!(r.varint().unwrap(), 3);
        for kind in [1, 4, 5] {
            assert_eq!(r.varint().unwrap(), kind);
            assert_eq!(r.varint().unwrap(), 37);
            let mut values = 0;
            for _ in 0..37 {
                let word = u64::from_be_bytes(r.take(8).unwrap().try_into().unwrap());
                for slot in 0..7 {
                    if values < 256 {
                        assert_eq!((word >> (slot * 9)) & 511, 128);
                        values += 1;
                    }
                }
            }
            assert_eq!(values, 256);
        }
        let mut s = Reader::new(r.array(4096).unwrap());
        for section in 0..24 {
            let count = i16::from_be_bytes(s.take(2).unwrap().try_into().unwrap());
            assert_eq!(count, if section < 8 { 4096 } else { 0 });
            assert_eq!(s.take(2).unwrap(), [0, 0]);
            assert_eq!(s.byte().unwrap(), 0);
            assert_eq!(s.varint().unwrap(), if section < 8 { 1 } else { 0 });
            assert_eq!(s.byte().unwrap(), 0);
            assert_eq!(s.varint().unwrap(), 42);
        }
        s.finish().unwrap();
        assert_eq!(r.varint().unwrap(), 0);
        for expected in [((1u64 << 26) - 1) ^ 511, 0, 511, (1u64 << 26) - 1] {
            let bytes = r.array(8).unwrap();
            let mut word = [0; 8];
            word[..bytes.len()].copy_from_slice(bytes);
            assert_eq!(u64::from_le_bytes(word), expected);
        }
        assert_eq!(r.varint().unwrap(), 17);
        for _ in 0..17 {
            assert!(r.array(2048).unwrap().iter().all(|&v| v == 255));
        }
        assert_eq!(r.varint().unwrap(), 0);
        r.finish().unwrap();
        assert!(template.shared_bytes() < 40 * 1024);
    }
    #[test]
    fn light_bitset_uses_java_bitset_byte_order_fixture() {
        let template = StoneChunk::new(0);
        let mut payload = Vec::new();
        template.encode(ChunkPos::ORIGIN, &mut payload);
        let mut r = Reader::new(&payload[9..]);
        let maps = r.varint().unwrap();
        for _ in 0..maps {
            r.varint().unwrap();
            let count = r.varint().unwrap();
            r.take(count as usize * 8).unwrap();
        }
        r.array(4096).unwrap();
        assert_eq!(r.varint().unwrap(), 0);
        assert_eq!(
            &r.remaining()[..18],
            &[
                4, 0, 254, 255, 3, 0, 2, 255, 1, 4, 255, 255, 255, 3, 17, 128, 16, 255
            ]
        );
    }
    #[test]
    fn reuse_does_not_materialize_or_reallocate_coordinate_chunks() {
        let template = StoneChunk::new(0);
        let mut scratch = Vec::new();
        template.encode(ChunkPos::ORIGIN, &mut scratch);
        let ptr = scratch.as_ptr();
        let capacity = scratch.capacity();
        let suffix = scratch[9..].to_vec();
        for x in -1000..1000 {
            template.encode(ChunkPos { x, z: -x }, &mut scratch);
            assert_eq!(scratch.as_ptr(), ptr);
            assert_eq!(scratch.capacity(), capacity);
            assert_eq!(scratch[9..], suffix);
        }
    }
}
