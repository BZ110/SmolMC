//! Bounded static skylight for supported zero-emission terrain. Event-driven, never ticking.
use crate::{ChunkPos, HEIGHT, InstanceOverlay, MIN_Y, Material, terrain::ImportedTerrain};
use std::{
    collections::{BTreeMap, VecDeque},
    io,
    sync::{Arc, OnceLock},
};
pub type LightKey = (ChunkPos, i32);
pub type LightPatch = BTreeMap<LightKey, Arc<[u8]>>;
fn invalid(s: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, s)
}
fn opacity(m: Material) -> u8 {
    static TABLE: OnceLock<Box<[u8]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = vec![15; 65536];
        for s in crate::STATES {
            t[s.id as usize] = s.opacity;
        }
        t.into_boxed_slice()
    })[m.state() as usize]
}
pub fn uniform(value: u8) -> Arc<[u8]> {
    static ZERO: OnceLock<Arc<[u8]>> = OnceLock::new();
    static FULL: OnceLock<Arc<[u8]>> = OnceLock::new();
    if value == 0 {
        ZERO.get_or_init(|| Arc::from([0; 2048])).clone()
    } else {
        FULL.get_or_init(|| Arc::from([255; 2048])).clone()
    }
}
pub fn base_sky(base: &ImportedTerrain, p: [i32; 3]) -> u8 {
    if p[1] >= MIN_Y + HEIGHT {
        return 15;
    }
    if p[1] < MIN_Y {
        return 0;
    }
    let c = ChunkPos {
        x: p[0].div_euclid(16),
        z: p[2].div_euclid(16),
    };
    let Some(chunk) = base.chunks.get(&c) else {
        return 15;
    };
    let Some((Some(data), _)) = chunk.light_sections.get(&p[1].div_euclid(16)) else {
        return 0;
    };
    let i = (p[1].rem_euclid(16) * 256 + p[2].rem_euclid(16) * 16 + p[0].rem_euclid(16)) as usize;
    (data[i / 2] >> ((i % 2) * 4)) & 15
}
pub fn sky(base: &ImportedTerrain, overlay: &InstanceOverlay, p: [i32; 3]) -> u8 {
    if let Some(data) = overlay.sky_section((
        ChunkPos {
            x: p[0].div_euclid(16),
            z: p[2].div_euclid(16),
        },
        p[1].div_euclid(16),
    )) {
        let i =
            (p[1].rem_euclid(16) * 256 + p[2].rem_euclid(16) * 16 + p[0].rem_euclid(16)) as usize;
        (data[i / 2] >> ((i % 2) * 4)) & 15
    } else {
        base_sky(base, p)
    }
}
pub fn recompute(
    base: &ImportedTerrain,
    overlay: &InstanceOverlay,
    edits: &[[i32; 3]],
) -> io::Result<LightPatch> {
    if edits.is_empty() {
        return Ok(BTreeMap::new());
    }
    let minx = edits.iter().map(|p| p[0].div_euclid(16) - 1).min().unwrap();
    let maxx = edits.iter().map(|p| p[0].div_euclid(16) + 1).max().unwrap();
    let minz = edits.iter().map(|p| p[2].div_euclid(16) - 1).min().unwrap();
    let maxz = edits.iter().map(|p| p[2].div_euclid(16) + 1).max().unwrap();
    calculate(base, overlay, [minx, minz, maxx, maxz], false)
}
pub fn relight_base(base: &mut ImportedTerrain) -> io::Result<()> {
    let b = base.bounds;
    let patch = calculate(
        base,
        &InstanceOverlay::new(),
        [
            b.min[0].div_euclid(16),
            b.min[2].div_euclid(16),
            (b.max[0] - 1).div_euclid(16),
            (b.max[2] - 1).div_euclid(16),
        ],
        true,
    )?;
    let mut added = 0usize;
    for ((pos, sy), data) in patch {
        if let Some(chunk) = base.chunks.get_mut(&pos) {
            let chunk = Arc::get_mut(chunk)
                .ok_or_else(|| invalid("base terrain must be unique during import"))?;
            let old = chunk
                .light_sections
                .get(&sy)
                .and_then(|p| p.0.as_ref())
                .map_or(0, |p| p.len());
            chunk.light_sections.insert(sy, (Some(data), None));
            added = added.saturating_add(2048usize.saturating_sub(old));
        }
    }
    base.bytes = base
        .bytes
        .checked_add(added)
        .ok_or_else(|| invalid("lighting byte overflow"))?;
    if base.bytes > crate::terrain::MAX_TEMPLATE_BYTES {
        return Err(invalid("retained lighting budget"));
    }
    Ok(())
}
fn calculate(
    base: &ImportedTerrain,
    overlay: &InstanceOverlay,
    r: [i32; 4],
    initial: bool,
) -> io::Result<LightPatch> {
    let nx = ((r[2] - r[0] + 1) * 16) as usize;
    let nz = ((r[3] - r[1] + 1) * 16) as usize;
    let ny = HEIGHT as usize;
    let n = nx
        .checked_mul(nz)
        .and_then(|n| n.checked_mul(ny))
        .ok_or_else(|| invalid("light extent overflow"))?;
    if n > if initial {
        256 * 256 * 384
    } else {
        2 * 1024 * 1024
    } {
        return Err(invalid("lighting work extent budget"));
    }
    let x0 = r[0] * 16;
    let z0 = r[1] * 16;
    let index = |x: usize, y: usize, z: usize| (y * nz + z) * nx + x;
    let mut blocks = vec![0u8; n];
    let mut values = vec![0u8; n];
    for z in 0..nz {
        for x in 0..nx {
            let mut light = 15u8;
            for y in (0..ny).rev() {
                let p = [x0 + x as i32, MIN_Y + y as i32, z0 + z as i32];
                let block = overlay.override_at(p).unwrap_or_else(|| base.material(p));
                let at = index(x, y, z);
                let op = opacity(block);
                blocks[at] = op;
                light = light.saturating_sub(if light == 15 && op == 0 { 0 } else { op.max(1) });
                values[at] = light;
            }
        }
    }
    let mut queue: VecDeque<u32> = VecDeque::new();
    let neighbors = |at: usize| {
        let x = at % nx;
        let z = (at / nx) % nz;
        let y = at / (nx * nz);
        [
            (x > 0).then(|| at - 1),
            (x + 1 < nx).then(|| at + 1),
            (z > 0).then(|| at - nx),
            (z + 1 < nz).then(|| at + nx),
            (y > 0).then(|| at - nx * nz),
            (y + 1 < ny).then(|| at + nx * nz),
        ]
    };
    // Fixed boundary data is outside the edited column's 15-block influence. Full map boundaries are open sky.
    for y in 0..ny {
        for z in 0..nz {
            for x in 0..nx {
                if x != 0 && x + 1 != nx && z != 0 && z + 1 != nz {
                    continue;
                }
                let at = index(x, y, z);
                let p = [x0 + x as i32, MIN_Y + y as i32, z0 + z as i32];
                for (edge, offset) in [
                    (x == 0, [-1, 0, 0]),
                    (x + 1 == nx, [1, 0, 0]),
                    (z == 0, [0, 0, -1]),
                    (z + 1 == nz, [0, 0, 1]),
                ] {
                    if edge {
                        let outside = if initial {
                            15
                        } else {
                            sky(base, overlay, std::array::from_fn(|i| p[i] + offset[i]))
                        };
                        values[at] = values[at].max(outside.saturating_sub(blocks[at].max(1)));
                    }
                }
            }
        }
    }
    for at in 0..n {
        let value = values[at];
        if value < 2 {
            continue;
        }
        for other in neighbors(at).into_iter().flatten() {
            let loss = if other + nx * nz == at && value == 15 && blocks[other] == 0 {
                0
            } else {
                blocks[other].max(1)
            };
            let next = value.saturating_sub(loss);
            if next > values[other] {
                values[other] = next;
                queue.push_back(other as u32);
                if queue.len() > 1_048_576 {
                    return Err(invalid("light frontier budget"));
                }
            }
        }
    }
    let mut work = 0usize;
    while let Some(at) = queue.pop_front() {
        let at = at as usize;
        let value = values[at];
        for other in neighbors(at).into_iter().flatten() {
            work += 1;
            if work > 200_000_000 {
                return Err(invalid("light relaxation budget"));
            }
            let loss = if other + nx * nz == at && value == 15 && blocks[other] == 0 {
                0
            } else {
                blocks[other].max(1)
            };
            let next = value.saturating_sub(loss);
            if next > values[other] {
                values[other] = next;
                queue.push_back(other as u32);
                if queue.len() > 1_048_576 {
                    return Err(invalid("light frontier budget"));
                }
            }
        }
    }
    let mut patch = BTreeMap::new();
    for cz in r[1]..=r[3] {
        for cx in r[0]..=r[2] {
            for sy in -4..20 {
                let mut data = [0u8; 2048];
                for y in 0..16 {
                    for z in 0..16 {
                        for x in 0..16 {
                            let local = y * 256 + z * 16 + x;
                            let at = index(
                                ((cx - r[0]) * 16) as usize + x,
                                ((sy + 4) * 16) as usize + y,
                                ((cz - r[1]) * 16) as usize + z,
                            );
                            data[local / 2] |= values[at] << ((local % 2) * 4);
                        }
                    }
                }
                let key = (ChunkPos { x: cx, z: cz }, sy);
                let previous = overlay.sky_section(key).or_else(|| {
                    base.chunks
                        .get(&key.0)
                        .and_then(|c| c.light_sections.get(&sy))
                        .and_then(|v| v.0.as_ref())
                        .map(AsRef::as_ref)
                });
                if initial || previous != Some(&data[..]) {
                    let bytes = if data.iter().all(|v| *v == 0) {
                        uniform(0)
                    } else if data.iter().all(|v| *v == 255) {
                        uniform(255)
                    } else {
                        Arc::from(data)
                    };
                    patch.insert(key, bytes);
                }
            }
        }
    }
    Ok(patch)
}
