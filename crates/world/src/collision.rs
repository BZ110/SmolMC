//! Local axis-aligned collision resolution. Static verified boxes only; no world tick.
use crate::{InstanceOverlay, Material, terrain::ImportedTerrain};
use std::sync::Arc;
const EPS: f64 = 1e-7;
#[derive(Clone, Debug)]
pub struct TerrainSnapshot {
    pub base: Arc<ImportedTerrain>,
    pub overlay: InstanceOverlay,
}
impl TerrainSnapshot {
    pub fn block(&self, p: [i32; 3]) -> Material {
        self.overlay
            .override_at(p)
            .unwrap_or_else(|| self.base.material(p))
    }
    pub fn body_clear(&self, pos: [f64; 3]) -> bool {
        let body = Aabb::player(pos, false);
        if pos[0] - 0.3 < self.base.bounds.min[0] as f64
            || pos[0] + 0.3 > self.base.bounds.max[0] as f64
            || pos[2] - 0.3 < self.base.bounds.min[2] as f64
            || pos[2] + 0.3 > self.base.bounds.max[2] as f64
        {
            return false;
        }
        for y in body.min[1].floor() as i32..=body.max[1].floor() as i32 {
            for z in body.min[2].floor() as i32..=body.max[2].floor() as i32 {
                for x in body.min[0].floor() as i32..=body.max[0].floor() as i32 {
                    for b in self.block([x, y, z]).info().boxes {
                        if body.intersects(Aabb {
                            min: [x as f64 + b[0], y as f64 + b[1], z as f64 + b[2]],
                            max: [x as f64 + b[3], y as f64 + b[4], z as f64 + b[5]],
                        }) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }
    /// Bounded recovery on current edited terrain; never materializes or scans a whole map.
    pub fn safe_spawn(&self, preferred: [f64; 3]) -> Option<[f64; 3]> {
        if self.body_clear(preferred) && self.grounded(preferred, false) {
            return Some(preferred);
        }
        let x = preferred[0].floor() as i32;
        let z = preferred[2].floor() as i32;
        for radius in 0i32..=4 {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs().max(dz.abs()) != radius {
                        continue;
                    }
                    for y in (self.base.bounds.min[1]..self.base.bounds.max[1] - 2).rev() {
                        if self.block([x + dx, y, z + dz]).solid() {
                            let pos = [(x + dx) as f64 + 0.5, y as f64 + 1., (z + dz) as f64 + 0.5];
                            if self.body_clear(pos) && self.grounded(pos, false) {
                                return Some(pos);
                            }
                        }
                    }
                }
            }
        }
        None
    }
    pub fn grounded(&self, pos: [f64; 3], crouch: bool) -> bool {
        resolve(pos, [0., -0.01, 0.], crouch, false, |p| self.block(p))
            .is_some_and(|r| r.delta[1] > -0.009999)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: [f64; 3],
    pub max: [f64; 3],
}
impl Aabb {
    pub fn player(p: [f64; 3], crouch: bool) -> Self {
        Self {
            min: [p[0] - 0.3, p[1], p[2] - 0.3],
            max: [
                p[0] + 0.3,
                p[1] + if crouch { 1.5 } else { 1.8 },
                p[2] + 0.3,
            ],
        }
    }
    pub fn moved(self, d: [f64; 3]) -> Self {
        Self {
            min: std::array::from_fn(|i| self.min[i] + d[i]),
            max: std::array::from_fn(|i| self.max[i] + d[i]),
        }
    }
    pub fn intersects(self, b: Self) -> bool {
        (0..3).all(|i| self.max[i] > b.min[i] + EPS && self.min[i] < b.max[i] - EPS)
    }
    fn clip(self, other: Self, axis: usize, mut d: f64) -> f64 {
        if (0..3).any(|i| {
            i != axis && (self.max[i] <= other.min[i] + EPS || self.min[i] >= other.max[i] - EPS)
        }) {
            return d;
        }
        if d > 0. && self.max[axis] <= other.min[axis] + EPS {
            d = d.min((other.min[axis] - self.max[axis]).max(0.))
        } else if d < 0. && self.min[axis] >= other.max[axis] - EPS {
            d = d.max((other.max[axis] - self.min[axis]).min(0.))
        }
        d
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Resolved {
    pub delta: [f64; 3],
    pub grounded: bool,
    pub collided: bool,
}
fn axes(mut b: Aabb, mut d: [f64; 3], shapes: &[Aabb]) -> [f64; 3] {
    let horizontal = if d[0].abs() < d[2].abs() {
        [2, 0]
    } else {
        [0, 2]
    };
    for a in [1, horizontal[0], horizontal[1]] {
        for shape in shapes {
            d[a] = b.clip(*shape, a, d[a])
        }
        let mut v = [0.; 3];
        v[a] = d[a];
        b = b.moved(v)
    }
    d
}
pub fn resolve(
    pos: [f64; 3],
    wanted: [f64; 3],
    crouch: bool,
    step: bool,
    mut block: impl FnMut([i32; 3]) -> Material,
) -> Option<Resolved> {
    if !pos
        .iter()
        .chain(&wanted)
        .all(|n| n.is_finite() && n.abs() <= 100512.)
    {
        return None;
    }
    let b = Aabb::player(pos, crouch);
    let lo: [i32; 3] = std::array::from_fn(|i| (b.min[i] + wanted[i].min(0.) - EPS).floor() as i32);
    let hi: [i32; 3] = std::array::from_fn(|i| {
        (b.max[i] + wanted[i].max(0.) + if i == 1 && step { 0.6 } else { EPS }).floor() as i32
    });
    let count = (0..3).try_fold(1usize, |n, i| n.checked_mul((hi[i] - lo[i] + 1) as usize))?;
    if count > 8192 {
        return None;
    }
    let mut local = [Aabb {
        min: [0.; 3],
        max: [0.; 3],
    }; 64];
    let mut count_shapes = 0usize;
    let mut overflow = Vec::new();
    for y in lo[1]..=hi[1] {
        for z in lo[2]..=hi[2] {
            for x in lo[0]..=hi[0] {
                for s in block([x, y, z]).info().boxes {
                    let shape = Aabb {
                        min: [x as f64 + s[0], y as f64 + s[1], z as f64 + s[2]],
                        max: [x as f64 + s[3], y as f64 + s[4], z as f64 + s[5]],
                    };
                    if count_shapes < local.len() {
                        local[count_shapes] = shape;
                    } else {
                        if overflow.is_empty() {
                            overflow.extend_from_slice(&local);
                        }
                        overflow.push(shape);
                    }
                    count_shapes += 1;
                    if count_shapes > 32768 {
                        return None;
                    }
                }
            }
        }
    }
    let shapes = if overflow.is_empty() {
        &local[..count_shapes]
    } else {
        overflow.as_slice()
    };
    let mut d = axes(b, wanted, shapes);
    if step && (d[0] - wanted[0]).abs() + (d[2] - wanted[2]).abs() > EPS {
        let raised = axes(b, [wanted[0], 0.6, wanted[2]], shapes);
        if raised[0] * raised[0] + raised[2] * raised[2] > d[0] * d[0] + d[2] * d[2] + EPS {
            let down = axes(b.moved(raised), [0., wanted[1] - raised[1], 0.], shapes);
            d = [raised[0], raised[1] + down[1], raised[2]]
        }
    }
    Some(Resolved {
        delta: d,
        grounded: wanted[1] < 0. && d[1] > wanted[1] + EPS,
        collided: (0..3).any(|i| (d[i] - wanted[i]).abs() > EPS),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn terrain(p: [i32; 3]) -> Material {
        if p[1] < 0 || (p[0] == 16 && p[1] < 3) {
            Material::Stone
        } else {
            Material::Air
        }
    }
    #[test]
    fn walls_floors_ceiling_negative_chunk_and_air() {
        let r = resolve([15.5, 0., -0.5], [2., -0.2, 0.], false, true, terrain).unwrap();
        assert!((r.delta[0] - 0.2).abs() < 1e-6);
        assert_eq!(r.delta[1], 0.);
        assert!(r.grounded);
        let r = resolve([-0.5, 1., -0.5], [0., -2., 0.], false, false, terrain).unwrap();
        assert_eq!(r.delta[1], -1.);
        let r = resolve([0.5, 0., 0.5], [0., 2., 0.], false, false, |p| {
            if p[1] == 2 {
                Material::Stone
            } else {
                Material::Air
            }
        })
        .unwrap();
        assert!((r.delta[1] - 0.2).abs() < 1e-6);
        assert!(resolve([0.; 3], [f64::NAN, 0., 0.], false, false, terrain).is_none());
        assert!(resolve([0.; 3], [100., 100., 100.], false, false, terrain).is_none());
    }
    #[test]
    fn verified_half_slab_step_and_ceiling_prevent_step() {
        let slab = Material::parse(
            "minecraft:stone_slab",
            Some("type=bottom,waterlogged=false"),
        )
        .unwrap();
        let block = |p: [i32; 3]| {
            if p[1] < 0 {
                Material::Stone
            } else if p == [1, 0, 0] {
                slab
            } else {
                Material::Air
            }
        };
        let r = resolve([0.5, 0., 0.5], [1., -0.01, 0.], false, true, block).unwrap();
        assert_eq!(r.delta, [1., 0.5, 0.]);
        let r = resolve([0.5, 0., 0.5], [1., -0.01, 0.], false, true, |p| {
            if p[1] == 2 { Material::Stone } else { block(p) }
        })
        .unwrap();
        assert!((r.delta[0] - 0.2).abs() < 1e-6);
    }
}
