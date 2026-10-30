//! Shared protocol-777 chunk suffixes for a bounded immutable imported map.
use smolmc_protocol_777::{encode_varint, login::Encoder};
use smolmc_world::{
    ChunkPos, HEIGHT, MIN_Y, Material,
    terrain::{ImportedTerrain, Packed, TerrainChunk},
};
use std::{collections::BTreeMap, io, sync::Arc};
pub struct ImportedChunks {
    chunks: BTreeMap<ChunkPos, Arc<[u8]>>,
    empty: Arc<[u8]>,
    bytes: usize,
}
impl ImportedChunks {
    pub fn new(
        terrain: &ImportedTerrain,
        default_biome: i32,
        biome_count: usize,
    ) -> io::Result<Self> {
        let mut chunks = BTreeMap::new();
        let mut bytes = 0;
        for (&pos, c) in &terrain.chunks {
            let suffix = encode_suffix(Some(c), default_biome, biome_count);
            bytes += suffix.len() + 9;
            if bytes > 32 * 1024 * 1024 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "encoded template budget",
                ));
            }
            let mut packet = Vec::with_capacity(suffix.len() + 9);
            packet.push(super::CHUNK_PACKET_ID as u8);
            packet.extend(pos.x.to_be_bytes());
            packet.extend(pos.z.to_be_bytes());
            packet.extend(suffix);
            chunks.insert(pos, Arc::from(packet));
        }
        let empty = Arc::from(encode_suffix(None, default_biome, biome_count));
        Ok(Self {
            chunks,
            empty,
            bytes,
        })
    }
    pub fn packet(&self, pos: ChunkPos) -> Option<&[u8]> {
        self.chunks.get(&pos).map(AsRef::as_ref)
    }
    pub fn encode(&self, pos: ChunkPos, out: &mut Vec<u8>) {
        out.clear();
        if let Some(packet) = self.packet(pos) {
            out.extend_from_slice(packet);
            return;
        }
        out.push(super::CHUNK_PACKET_ID as u8);
        out.extend(pos.x.to_be_bytes());
        out.extend(pos.z.to_be_bytes());
        out.extend_from_slice(&self.empty);
    }
    pub fn bytes(&self) -> usize {
        self.bytes + self.empty.len()
    }
}
fn palette(out: &mut Vec<u8>, p: &Packed, min_bits: u8, max_local: u8, global_bits: u8) {
    let bits = if p.palette.len() == 1 {
        0
    } else {
        min_bits.max((usize::BITS - (p.palette.len() - 1).leading_zeros()) as u8)
    };
    let indirect = bits <= max_local;
    let wire_bits = if indirect { bits } else { global_bits };
    out.push(wire_bits);
    if bits == 0 {
        encode_varint(p.palette[0], out);
        return;
    }
    if indirect {
        encode_varint(p.palette.len() as i32, out);
        for &id in &p.palette {
            encode_varint(id, out)
        }
    }
    if indirect && wire_bits == p.bits {
        for word in &p.words {
            out.extend(word.to_be_bytes())
        }
        return;
    }
    let per = 64 / wire_bits as usize;
    let mut values = Vec::with_capacity(p.count);
    for i in 0..p.count {
        values.push(if indirect {
            p.palette.iter().position(|v| *v == p.value(i)).unwrap() as u64
        } else {
            p.value(i) as u64
        })
    }
    for chunk in values.chunks(per) {
        let word = chunk
            .iter()
            .enumerate()
            .fold(0u64, |v, (i, n)| v | (n << (i * wire_bits as usize)));
        out.extend(word.to_be_bytes());
    }
}
fn bitset(out: &mut Encoder, mask: u32) {
    let b = mask.to_le_bytes();
    let n = b.iter().rposition(|&v| v != 0).map_or(0, |n| n + 1);
    out.array(&b[..n]);
}
pub fn encode_suffix(chunk: Option<&TerrainChunk>, biome: i32, biome_count: usize) -> Vec<u8> {
    let mut out = Encoder::new(3);
    let mut surface = [0u16; 256];
    let mut motion = [0u16; 256];
    if let Some(c) = chunk {
        for z in 0..16 {
            for x in 0..16 {
                for y in (MIN_Y..MIN_Y + HEIGHT).rev() {
                    let m = c.material([c.pos.x * 16 + x, y, c.pos.z * 16 + z]);
                    let at = (z * 16 + x) as usize;
                    if surface[at] == 0 && !m.air() {
                        surface[at] = (y - MIN_Y + 1) as u16
                    }
                    if motion[at] == 0 && !m.info().boxes.is_empty() {
                        motion[at] = (y - MIN_Y + 1) as u16
                    }
                    if surface[at] != 0 && motion[at] != 0 {
                        break;
                    }
                }
            }
        }
    }
    for (kind, heights) in [(1, &surface), (4, &motion), (5, &motion)] {
        out.varint(kind);
        out.varint(37);
        for values in heights.chunks(7) {
            let word = values
                .iter()
                .enumerate()
                .fold(0u64, |v, (i, n)| v | ((*n as u64) << (i * 9)));
            out.raw(&word.to_be_bytes());
        }
    }
    let mut sections = Vec::new();
    let air = Packed::singleton(0, 4096);
    let plain = Packed::singleton(biome, 64);
    let biome_bits = (usize::BITS - (biome_count.max(2) - 1).leading_zeros()) as u8;
    for at in 0..24 {
        let section = chunk.and_then(|c| c.sections[at].as_ref());
        let b = section.map_or(&air, |s| &s.blocks);
        let count = (0..4096)
            .filter(|&i| {
                !Material::from_state(b.value(i))
                    .expect("supported block")
                    .air()
            })
            .count();
        sections.extend((count as u16).to_be_bytes());
        sections.extend(0u16.to_be_bytes());
        palette(&mut sections, b, 4, 8, 16);
        palette(
            &mut sections,
            section.map_or(&plain, |s| &s.biomes),
            1,
            3,
            biome_bits,
        );
    }
    out.array(&sections);
    out.varint(0);
    let mut sky_data = Vec::new();
    let mut block_data = Vec::new();
    let (mut sky_mask, mut block_mask, mut empty_sky, mut empty_block) = (0u32, 0u32, 0u32, 0u32);
    for bit in 0..26 {
        let sy = bit - 5;
        let source = chunk.and_then(|c| c.light_sections.get(&sy));
        let sky = source.and_then(|s| s.0.as_deref());
        let block = source.and_then(|s| s.1.as_deref());
        if let Some(light) = sky {
            sky_mask |= 1 << bit;
            sky_data.push(light.to_vec())
        } else if chunk.is_none() || sy >= 12 {
            sky_mask |= 1 << bit;
            sky_data.push(vec![255; 2048])
        } else {
            empty_sky |= 1 << bit
        }
        if let Some(light) = block {
            block_mask |= 1 << bit;
            block_data.push(light.to_vec())
        } else {
            empty_block |= 1 << bit
        }
    }
    for mask in [sky_mask, block_mask, empty_sky, empty_block] {
        bitset(&mut out, mask)
    }
    out.varint(sky_data.len() as i32);
    for b in sky_data {
        out.array(&b)
    }
    out.varint(block_data.len() as i32);
    for b in block_data {
        out.array(&b)
    }
    out.finish()
}
/// Protocol 777 LIGHT_UPDATE (ID independently checked against GameProtocols).
pub fn light_update<'a>(
    pos: ChunkPos,
    sections: impl IntoIterator<Item = (i32, &'a [u8])>,
) -> Vec<u8> {
    let mut out = Encoder::new(0x31);
    out.varint(pos.x);
    out.varint(pos.z);
    let (mut mask, mut empty) = (0u32, 0u32);
    let mut arrays = Vec::new();
    for (sy, data) in sections {
        assert!((-5..=20).contains(&sy) && data.len() == 2048);
        if data.iter().all(|v| *v == 0) {
            empty |= 1 << (sy + 5)
        } else {
            mask |= 1 << (sy + 5);
            arrays.push(data)
        }
    }
    for m in [mask, 0, empty, 0] {
        bitset(&mut out, m)
    }
    out.varint(arrays.len() as i32);
    for data in arrays {
        out.array(data)
    }
    out.varint(0);
    out.finish()
}
