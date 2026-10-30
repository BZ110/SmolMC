//! Bounded, read-only 26.3 Anvil import. No world generation, entity execution or IO on reads.
use crate::{
    ChunkPos, HEIGHT, MIN_Y, Material,
    nbt::{self, Tag, invalid},
};
use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::Arc,
};
pub const MAX_CHUNKS: usize = 256;
pub const MAX_TEMPLATE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_REGION_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_IMPORT_INPUT: usize = 128 * 1024 * 1024;
pub const MAX_CHUNK_BYTES: usize = 2 * 1024 * 1024;
#[derive(Clone, Copy, Debug)]
pub struct ImportBounds {
    pub min: [i32; 3],
    pub max: [i32; 3],
}
impl ImportBounds {
    pub fn validate(self) -> io::Result<Self> {
        if (0..3).any(|i| self.min[i] >= self.max[i])
            || self.min[1] < MIN_Y
            || self.max[1] > MIN_Y + HEIGHT
            || self.min[0].unsigned_abs() > 100000
            || self.max[0].unsigned_abs() > 100000
            || self.min[2].unsigned_abs() > 100000
            || self.max[2].unsigned_abs() > 100000
        {
            return Err(invalid("invalid import bounds"));
        }
        let nx = (self.max[0] - 1).div_euclid(16) - self.min[0].div_euclid(16) + 1;
        let nz = (self.max[2] - 1).div_euclid(16) - self.min[2].div_euclid(16) + 1;
        if nx as usize * nz as usize > MAX_CHUNKS {
            return Err(invalid("import chunk budget"));
        }
        Ok(self)
    }
    pub fn contains(self, p: [i32; 3]) -> bool {
        (0..3).all(|i| p[i] >= self.min[i] && p[i] < self.max[i])
    }
    pub fn chunks(self) -> impl Iterator<Item = ChunkPos> {
        let minx = self.min[0].div_euclid(16);
        let maxx = (self.max[0] - 1).div_euclid(16);
        let minz = self.min[2].div_euclid(16);
        let maxz = (self.max[2] - 1).div_euclid(16);
        (minz..=maxz).flat_map(move |z| (minx..=maxx).map(move |x| ChunkPos { x, z }))
    }
}
#[derive(Debug)]
pub struct Packed {
    pub palette: Box<[i32]>,
    pub words: Box<[u64]>,
    pub bits: u8,
    pub count: usize,
}
impl Packed {
    pub fn value(&self, i: usize) -> i32 {
        if self.palette.len() == 1 {
            return self.palette[0];
        }
        let per = 64 / self.bits as usize;
        let at = ((self.words[i / per] >> ((i % per) * self.bits as usize))
            & ((1 << self.bits) - 1)) as usize;
        self.palette[at]
    }
    fn read(
        tag: &Tag,
        count: usize,
        min_bits: u8,
        mut map: impl FnMut(&Tag) -> io::Result<i32>,
    ) -> io::Result<Self> {
        let entries = tag
            .get("palette")
            .and_then(Tag::list)
            .ok_or_else(|| invalid("missing palette"))?;
        if entries.is_empty() || entries.len() > count {
            return Err(invalid("palette size"));
        }
        let palette = entries
            .iter()
            .map(&mut map)
            .collect::<io::Result<Vec<_>>>()?;
        let bits = if palette.len() == 1 {
            0
        } else {
            min_bits.max((usize::BITS - (palette.len() - 1).leading_zeros()) as u8)
        };
        let words = tag.get("data").and_then(Tag::longs).unwrap_or(&[]);
        let expected = if bits == 0 {
            0
        } else {
            count.div_ceil(64 / bits as usize)
        };
        if words.len() != expected {
            return Err(invalid("packed palette word length"));
        }
        let p = Self {
            palette: palette.into_boxed_slice(),
            words: words.into(),
            bits,
            count,
        };
        if bits > 0 {
            let per = 64 / bits as usize;
            let mask = (1u64 << bits) - 1;
            for i in 0..count {
                if ((p.words[i / per] >> ((i % per) * bits as usize)) & mask) as usize
                    >= p.palette.len()
                {
                    return Err(invalid("palette index outside range"));
                }
            }
        }
        Ok(p)
    }
    pub fn singleton(value: i32, count: usize) -> Self {
        Self {
            palette: vec![value].into_boxed_slice(),
            words: Box::new([]),
            bits: 0,
            count,
        }
    }
    pub fn bytes(&self) -> usize {
        self.palette.len() * 4 + self.words.len() * 8 + std::mem::size_of::<Self>()
    }
}
#[derive(Debug)]
pub struct Section {
    pub blocks: Packed,
    pub biomes: Packed,
    pub sky: Option<Arc<[u8]>>,
    pub block_light: Option<Arc<[u8]>>,
}
pub type LightLayers = (Option<Arc<[u8]>>, Option<Arc<[u8]>>);
#[derive(Debug)]
pub struct TerrainChunk {
    pub pos: ChunkPos,
    pub sections: Box<[Option<Section>]>,
    pub light_sections: BTreeMap<i32, LightLayers>,
    pub bytes: usize,
}
impl TerrainChunk {
    pub fn material(&self, p: [i32; 3]) -> Material {
        if !(MIN_Y..MIN_Y + HEIGHT).contains(&p[1]) {
            return Material::Air;
        }
        let s = (p[1] - MIN_Y) as usize / 16;
        let i = ((p[1].rem_euclid(16) * 256) + (p[2].rem_euclid(16) * 16) + p[0].rem_euclid(16))
            as usize;
        self.sections[s]
            .as_ref()
            .map_or(Material::Air, |s| Material::known(s.blocks.value(i)))
    }
}
#[derive(Debug)]
pub struct ImportedTerrain {
    pub bounds: ImportBounds,
    pub chunks: BTreeMap<ChunkPos, Arc<TerrainChunk>>,
    pub bytes: usize,
    pub input_bytes: usize,
    pub clipped_blocks: usize,
    pub source_files: Vec<PathBuf>,
    pub removed_palette_entries: BTreeMap<String, usize>,
}
impl ImportedTerrain {
    pub fn material(&self, p: [i32; 3]) -> Material {
        if !self.bounds.contains(p) {
            return Material::Air;
        }
        self.chunks
            .get(&ChunkPos {
                x: p[0].div_euclid(16),
                z: p[2].div_euclid(16),
            })
            .map_or(Material::Air, |c| c.material(p))
    }
    pub fn safe_spawn(&self, near: [i32; 2]) -> Option<[f64; 3]> {
        let mut best = None;
        for c in self.chunks.values() {
            for z in 0..16 {
                for x in 0..16 {
                    let x = c.pos.x * 16 + x;
                    let z = c.pos.z * 16 + z;
                    for y in (self.bounds.min[1]..self.bounds.max[1] - 2).rev() {
                        if self.material([x, y, z]).solid()
                            && self.material([x, y + 1, z]).air()
                            && self.material([x, y + 2, z]).air()
                        {
                            let score = (x - near[0]).pow(2) + (z - near[1]).pow(2);
                            if best.is_none_or(|(s, _)| score < s) {
                                best =
                                    Some((score, [x as f64 + 0.5, y as f64 + 1., z as f64 + 0.5]))
                            }
                            break;
                        }
                    }
                }
            }
        }
        best.map(|(_, p)| p)
    }
}
fn state(tag: &Tag) -> io::Result<i32> {
    let (name, props) = if let Some(name) = tag.string() {
        (name, None)
    } else {
        let name = tag
            .get("id")
            .or_else(|| tag.get("Name"))
            .and_then(Tag::string)
            .ok_or_else(|| invalid("invalid block-state record"))?;
        let props = tag
            .get("properties")
            .or_else(|| tag.get("Properties"))
            .map(|p| {
                p.compound()
                    .ok_or_else(|| invalid("properties compound"))?
                    .iter()
                    .map(|(k, v)| {
                        v.string()
                            .map(|v| format!("{k}={v}"))
                            .ok_or_else(|| invalid("property string"))
                    })
                    .collect::<io::Result<Vec<_>>>()
                    .map(|v| v.join(","))
            })
            .transpose()?;
        (name, props)
    };
    Material::parse(name, props.as_deref())
        .map(Material::state)
        .ok_or_else(|| {
            invalid(&format!(
                "unsupported state {name} {}",
                props.as_deref().unwrap_or("default")
            ))
        })
}
fn light(tag: Option<&Tag>) -> io::Result<Option<Arc<[u8]>>> {
    tag.map(|v| {
        let b = v.bytes().ok_or_else(|| invalid("light byte array"))?;
        if b.len() != 2048 {
            return Err(invalid("light length"));
        }
        Ok(b.into())
    })
    .transpose()
}
pub fn decode_chunk(
    bytes: &[u8],
    pos: ChunkPos,
    bounds: ImportBounds,
    biomes: &HashMap<String, i32>,
) -> io::Result<(TerrainChunk, usize)> {
    decode_chunk_policy(bytes, pos, bounds, biomes, false, &mut BTreeMap::new())
}
fn decode_chunk_policy(
    bytes: &[u8],
    pos: ChunkPos,
    bounds: ImportBounds,
    biomes: &HashMap<String, i32>,
    convert: bool,
    removed: &mut BTreeMap<String, usize>,
) -> io::Result<(TerrainChunk, usize)> {
    let root = nbt::read(bytes)?;
    if root.get("DataVersion").and_then(Tag::number) != Some(5023) {
        return Err(invalid(
            "requires exact 26.3 data version 5023; no data fixer",
        ));
    }
    if root.get("xPos").and_then(Tag::number) != Some(pos.x as i64)
        || root.get("zPos").and_then(Tag::number) != Some(pos.z as i64)
    {
        return Err(invalid("chunk coordinate mismatch"));
    }
    if root.get("Status").and_then(Tag::string) != Some("minecraft:full") {
        return Err(invalid("only fully generated chunks supported"));
    }
    if let Some(entities) = root.get("block_entities").and_then(Tag::list) {
        for t in entities {
            let pos = ["x", "y", "z"].map(|k| {
                t.get(k)
                    .and_then(Tag::number)
                    .and_then(|n| i32::try_from(n).ok())
            });
            if pos.iter().any(Option::is_none) {
                return Err(invalid("block entity position"));
            }
            if !convert && bounds.contains(pos.map(Option::unwrap)) {
                return Err(invalid("block entities unsupported inside crop"));
            }
        }
    }
    let entries = root
        .get("sections")
        .and_then(Tag::list)
        .ok_or_else(|| invalid("missing sections"))?;
    if entries.len() > 26 {
        return Err(invalid("section count"));
    }
    let mut sections: Vec<Option<Section>> = (0..24).map(|_| None).collect();
    let mut light_sections = BTreeMap::new();
    let mut clipped = 0;
    let mut size = std::mem::size_of::<TerrainChunk>();
    for t in entries {
        let sy = t
            .get("Y")
            .and_then(Tag::number)
            .ok_or_else(|| invalid("section Y"))?;
        if !(-5..=20).contains(&sy) {
            return Err(invalid("section Y outside dimension"));
        }
        if light_sections.contains_key(&(sy as i32)) {
            return Err(invalid("duplicate section"));
        }
        let sky = light(t.get("SkyLight"))?;
        let block_light = light(t.get("BlockLight"))?;
        size += sky.as_ref().map_or(0, |v| v.len()) + block_light.as_ref().map_or(0, |v| v.len());
        if (-4..20).contains(&sy) {
            let by = sy as i32 * 16;
            let blocks_tag = t.get("block_states");
            let blocks = if by + 16 <= bounds.min[1] || by >= bounds.max[1] {
                clipped += 4096;
                Packed::singleton(0, 4096)
            } else {
                let p = Packed::read(
                    blocks_tag.ok_or_else(|| invalid("missing block states"))?,
                    4096,
                    4,
                    |tag| match state(tag) {
                        Ok(id) => Ok(id),
                        Err(error) => {
                            if !convert {
                                return Err(error);
                            }
                            let name = tag
                                .string()
                                .or_else(|| {
                                    tag.get("id")
                                        .or_else(|| tag.get("Name"))
                                        .and_then(Tag::string)
                                })
                                .ok_or_else(|| invalid("invalid block record"))?;
                            if !name.starts_with("minecraft:") || name.len() > 128 {
                                return Err(invalid("unsupported conversion namespace"));
                            }
                            let key = error.to_string();
                            if removed.len() >= 256 && !removed.contains_key(&key) {
                                return Err(invalid("conversion report budget"));
                            }
                            *removed.entry(key).or_default() += 1;
                            Ok(0)
                        }
                    },
                )?;
                if by < bounds.min[1]
                    || by + 16 > bounds.max[1]
                    || pos.x * 16 < bounds.min[0]
                    || pos.x * 16 + 16 > bounds.max[0]
                    || pos.z * 16 < bounds.min[2]
                    || pos.z * 16 + 16 > bounds.max[2]
                {
                    return Err(invalid(
                        "import cropping must align to chunk/section boundaries",
                    ));
                }
                p
            };
            let biomes_tag = t.get("biomes").ok_or_else(|| invalid("missing biomes"))?;
            let b = Packed::read(biomes_tag, 64, 1, |t| {
                biomes
                    .get(t.string().ok_or_else(|| invalid("biome identifier"))?)
                    .copied()
                    .ok_or_else(|| invalid("unknown biome"))
            })?;
            size += blocks.bytes() + b.bytes();
            sections[(sy + 4) as usize] = Some(Section {
                blocks,
                biomes: b,
                sky: None,
                block_light: None,
            });
        }
        light_sections.insert(sy as i32, (sky, block_light));
    }
    Ok((
        TerrainChunk {
            pos,
            sections: sections.into_boxed_slice(),
            light_sections,
            bytes: size,
        },
        clipped,
    ))
}
fn open_region(root: &Path, rx: i32, rz: i32) -> io::Result<File> {
    let p = root.join(format!("r.{rx}.{rz}.mca"));
    let meta = std::fs::symlink_metadata(&p)?;
    if !meta.is_file()
        || meta.file_type().is_symlink()
        || meta.len() > MAX_REGION_BYTES
        || meta.len() < 8192
        || meta.len() % 4096 != 0
    {
        return Err(invalid("region file type/size/alignment"));
    }
    let canonical = p.canonicalize()?;
    if canonical.parent() != Some(root) {
        return Err(invalid("region path escaped selected root"));
    }
    File::open(canonical)
}
pub fn import_world(
    root: &Path,
    bounds: ImportBounds,
    biomes: &HashMap<String, i32>,
) -> io::Result<Arc<ImportedTerrain>> {
    import_world_policy(root, bounds, biomes, false)
}
pub fn import_world_policy(
    root: &Path,
    bounds: ImportBounds,
    biomes: &HashMap<String, i32>,
    convert: bool,
) -> io::Result<Arc<ImportedTerrain>> {
    let bounds = bounds.validate()?;
    if bounds.min[0] % 16 != 0
        || bounds.max[0] % 16 != 0
        || bounds.min[2] % 16 != 0
        || bounds.max[2] % 16 != 0
        || bounds.min[1] % 16 != 0
        || bounds.max[1] % 16 != 0
    {
        return Err(invalid("crop must align to 16-block boundaries"));
    }
    let new = root.join("dimensions/minecraft/overworld/region");
    let root = if new.is_dir() {
        new
    } else {
        root.join("region")
    }
    .canonicalize()?;
    let mut terrain = ImportedTerrain {
        bounds,
        chunks: BTreeMap::new(),
        bytes: 0,
        input_bytes: 0,
        clipped_blocks: 0,
        source_files: vec![],
        removed_palette_entries: BTreeMap::new(),
    };
    let mut regions: HashMap<(i32, i32), (File, Vec<u8>)> = HashMap::new();
    for pos in bounds.chunks() {
        let region = (pos.x.div_euclid(32), pos.z.div_euclid(32));
        if let std::collections::hash_map::Entry::Vacant(v) = regions.entry(region) {
            let mut f = open_region(&root, region.0, region.1)?;
            let mut header = vec![0; 8192];
            f.read_exact(&mut header)?;
            let total = f.metadata()?.len() / 4096;
            let mut allocated = vec![false; total as usize];
            allocated[0] = true;
            allocated[1] = true;
            for loc in header[..4096].as_chunks::<4>().0 {
                let sector = ((loc[0] as usize) << 16) | ((loc[1] as usize) << 8) | loc[2] as usize;
                let count = loc[3] as usize;
                if sector == 0 && count == 0 {
                    continue;
                }
                if sector < 2 || count == 0 || sector + count > allocated.len() {
                    return Err(invalid("region location table range"));
                }
                for used in &mut allocated[sector..sector + count] {
                    if *used {
                        return Err(invalid("overlapping region sectors"));
                    }
                    *used = true;
                }
            }
            terrain
                .source_files
                .push(root.join(format!("r.{}.{}.mca", region.0, region.1)));
            v.insert((f, header));
        }
        let (f, header) = regions.get_mut(&region).unwrap();
        let slot = (pos.z.rem_euclid(32) * 32 + pos.x.rem_euclid(32)) as usize * 4;
        let sector = ((header[slot] as u32) << 16)
            | ((header[slot + 1] as u32) << 8)
            | header[slot + 2] as u32;
        let sectors = header[slot + 3] as usize;
        if sector < 2 || sectors == 0 {
            return Err(invalid("missing selected chunk"));
        }
        let offset = sector as u64 * 4096;
        if offset + sectors as u64 * 4096 > f.metadata()?.len() {
            return Err(invalid("region location outside file"));
        }
        f.seek(SeekFrom::Start(offset))?;
        let mut size = [0; 4];
        f.read_exact(&mut size)?;
        let size = u32::from_be_bytes(size) as usize;
        if !(2..=MAX_CHUNK_BYTES).contains(&size) || size + 4 > sectors * 4096 {
            return Err(invalid("chunk compressed size"));
        }
        terrain.input_bytes = terrain
            .input_bytes
            .checked_add(size)
            .filter(|n| *n <= MAX_IMPORT_INPUT)
            .ok_or_else(|| invalid("import input budget"))?;
        let mut data = vec![0; size];
        f.read_exact(&mut data)?;
        let decoded = decompress(&data)?;
        let (chunk, clipped) = decode_chunk_policy(
            &decoded,
            pos,
            bounds,
            biomes,
            convert,
            &mut terrain.removed_palette_entries,
        )
        .map_err(|e| invalid(&format!("chunk {},{}: {e}", pos.x, pos.z)))?;
        terrain.bytes += chunk.bytes;
        if terrain.bytes > MAX_TEMPLATE_BYTES {
            return Err(invalid("template retained bytes budget"));
        }
        terrain.clipped_blocks += clipped;
        terrain.chunks.insert(pos, Arc::new(chunk));
    }
    crate::lighting::relight_base(&mut terrain)?;
    Ok(Arc::new(terrain))
}

fn decompress(data: &[u8]) -> io::Result<Vec<u8>> {
    let (&kind, body) = data
        .split_first()
        .ok_or_else(|| invalid("missing chunk compression"))?;
    let mut decoded = Vec::new();
    match kind {
        1 => {
            let mut decoder = flate2::bufread::GzDecoder::new(body);
            decoder
                .by_ref()
                .take(MAX_CHUNK_BYTES as u64 + 1)
                .read_to_end(&mut decoded)?;
            if !decoder.into_inner().is_empty() {
                return Err(invalid("trailing compressed chunk bytes"));
            }
        }
        2 => {
            let mut decoder = flate2::bufread::ZlibDecoder::new(body);
            decoder
                .by_ref()
                .take(MAX_CHUNK_BYTES as u64 + 1)
                .read_to_end(&mut decoded)?;
            if !decoder.into_inner().is_empty() {
                return Err(invalid("trailing compressed chunk bytes"));
            }
        }
        3 => decoded.extend_from_slice(body),
        _ => {
            return Err(invalid(
                "unsupported external/LZ4/custom region compression",
            ));
        }
    }
    if decoded.len() > MAX_CHUNK_BYTES {
        return Err(invalid("chunk decompression budget"));
    }
    Ok(decoded)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn compound(entries: Vec<(&str, Tag)>) -> Tag {
        Tag::Compound(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect(),
        )
    }
    #[test]
    fn packed_palette_indices_boundaries_and_invalid_values() {
        let mut words = vec![0u64; 256];
        words[0] = 1;
        words[15] = 1 << 60;
        words[16] = 1;
        let tag = compound(vec![
            (
                "palette",
                Tag::List(vec![
                    Tag::String("minecraft:air".into()),
                    Tag::String("minecraft:stone".into()),
                ]),
            ),
            ("data", Tag::Longs(words)),
        ]);
        let packed = Packed::read(&tag, 4096, 4, state).unwrap();
        assert_eq!(packed.value(0), 1);
        assert_eq!(packed.value(255), 1);
        assert_eq!(packed.value(256), 1);
        assert_eq!(packed.value(4095), 0);
        let bad = compound(vec![
            ("palette", Tag::List(vec![Tag::Number(0), Tag::Number(1)])),
            ("data", Tag::Longs(vec![15; 256])),
        ]);
        assert!(Packed::read(&bad, 4096, 4, |t| Ok(t.number().unwrap() as i32)).is_err());
        assert!(
            Packed::read(
                &compound(vec![("palette", Tag::List(vec![]))]),
                4096,
                4,
                state
            )
            .is_err()
        );
    }
    #[test]
    fn exact_state_records_and_mixed_nbt_compatibility() {
        assert_eq!(state(&Tag::String("minecraft:stone".into())).unwrap(), 1);
        assert!(state(&Tag::String("minecraft:water".into())).is_err());
        let p = compound(vec![("axis", Tag::String("x".into()))]);
        let t = compound(vec![
            ("id", Tag::String("minecraft:oak_log".into())),
            ("properties", p),
        ]);
        assert!(state(&t).is_ok());
        assert!(
            state(&compound(vec![
                ("id", Tag::String("minecraft:oak_log".into())),
                (
                    "properties",
                    compound(vec![("axis", Tag::String("bad".into()))])
                )
            ]))
            .is_err()
        );
    }
    #[test]
    fn compression_bomb_trailing_bytes_and_unsupported_are_rejected() {
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        z.write_all(b"hello").unwrap();
        let mut good = vec![2];
        good.extend(z.finish().unwrap());
        assert_eq!(decompress(&good).unwrap(), b"hello");
        good.push(99);
        assert!(decompress(&good).is_err());
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        z.write_all(&vec![0; MAX_CHUNK_BYTES + 1]).unwrap();
        let mut bomb = vec![2];
        bomb.extend(z.finish().unwrap());
        assert!(decompress(&bomb).is_err());
        assert!(decompress(&[4, 0]).is_err());
        assert!(decompress(&[130, 0]).is_err());
    }
    #[test]
    fn region_overlap_symlink_and_import_extent_limits() {
        let root = std::env::temp_dir().join(format!("smc-region-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut bytes = vec![0; 16384];
        bytes[..8].copy_from_slice(&[0, 0, 2, 1, 0, 0, 2, 1]);
        std::fs::write(root.join("r.0.0.mca"), &bytes).unwrap();
        let bounds = ImportBounds {
            min: [0, 0, 0],
            max: [16, 16, 16],
        };
        let world = root.join("world");
        std::fs::create_dir_all(world.join("region")).unwrap();
        std::fs::copy(root.join("r.0.0.mca"), world.join("region/r.0.0.mca")).unwrap();
        assert!(
            import_world(&world, bounds, &HashMap::new())
                .unwrap_err()
                .to_string()
                .contains("overlapping")
        );
        assert!(
            ImportBounds {
                min: [i32::MIN, 0, 0],
                max: [16, 16, 16]
            }
            .validate()
            .is_err()
        );
        assert!(
            ImportBounds {
                min: [0, -64, 0],
                max: [10000, 320, 10000]
            }
            .validate()
            .is_err()
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("r.0.0.mca"), root.join("r.1.0.mca")).unwrap();
            assert!(open_region(&root.canonicalize().unwrap(), 1, 0).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
