//! Immutable M1C content/geometry, independent of sessions and protocol. No live
//! simulation jobs, timers or persistence. M2A adds an explicit world-instance and
//! template model with per-instance overlays; the immutable materials and geometry
//! below remain read-only and shared. No world simulation, Java bridge or survival.
mod block;
pub mod collision;
pub mod lighting;
mod nbt;
pub mod terrain;
pub use block::{Material, STATES, StateInfo};
mod instance;
mod overlay;
mod profile;
pub use instance::{
    Dimension, TemplateId, TemplateSignature, WorldError, WorldHandle, WorldId, WorldInstance,
    WorldManager, WorldMetrics, WorldState, WorldTemplate,
};
pub use overlay::{
    DEFAULT_OVERLAY_BUDGET, InstanceError, InstanceOverlay, MAX_EDIT_BATCH, OverlayBudget,
    OverlayMemory,
};
pub use profile::{Feature, WorldLimits, WorldProfile};

pub const MIN_Y: i32 = -64;
pub const HEIGHT: i32 = 384;
pub const SURFACE: i32 = 64; // first air layer; stone ends at 63
pub const VIEW_RADIUS: i32 = 2;
pub const VIEW_CAPACITY: usize = 25;
pub const SPAWN: [f64; 3] = [8.0, 65.0, 8.0];
pub const HORIZONTAL_BOUND: f64 = 100_000.0;

#[derive(Debug, Default)]
pub struct VirtualStone;
impl VirtualStone {
    pub fn block(&self, y: i32) -> Material {
        if (MIN_Y..SURFACE).contains(&y) {
            Material::Stone
        } else {
            Material::Air
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}
impl ChunkPos {
    pub const ORIGIN: Self = Self { x: 0, z: 0 };
    pub fn from_position(x: f64, z: f64) -> Option<Self> {
        if !x.is_finite()
            || !z.is_finite()
            || x.abs() > HORIZONTAL_BOUND
            || z.abs() > HORIZONTAL_BOUND
        {
            return None;
        }
        Some(Self {
            x: (x / 16.0).floor() as i32,
            z: (z / 16.0).floor() as i32,
        })
    }
    pub fn contains(self, other: Self) -> bool {
        (self.x - other.x).abs() <= VIEW_RADIUS && (self.z - other.z).abs() <= VIEW_RADIUS
    }
}

/// Fixed-capacity, allocation-free subscriber state. Pending chunks are computed
/// from desired center and sent entries, so rapid travel cannot grow a mailbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkView {
    center: ChunkPos,
    sent: [Option<ChunkPos>; VIEW_CAPACITY],
}
impl Default for ChunkView {
    fn default() -> Self {
        Self {
            center: ChunkPos::ORIGIN,
            sent: [None; VIEW_CAPACITY],
        }
    }
}
impl ChunkView {
    pub fn positions(&self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.sent.iter().flatten().copied()
    }
    pub fn center(&self) -> ChunkPos {
        self.center
    }
    pub fn set_center(&mut self, center: ChunkPos) {
        self.center = center;
    }
    pub fn loaded(&self) -> usize {
        self.sent.iter().flatten().count()
    }
    pub fn contains(&self, pos: ChunkPos) -> bool {
        self.sent.contains(&Some(pos))
    }
    pub fn next_unload(&self) -> Option<ChunkPos> {
        self.sent
            .iter()
            .flatten()
            .find(|&&pos| !self.center.contains(pos))
            .copied()
    }
    pub fn remove(&mut self, pos: ChunkPos) {
        if let Some(slot) = self.sent.iter_mut().find(|slot| **slot == Some(pos)) {
            *slot = None;
        }
    }
    pub fn next_missing(&self) -> Option<ChunkPos> {
        // A full, valid view covers all 25 distinct positions. Avoid the ring's
        // repeated contains scans on every ordinary Play wake; retain correct
        // behavior after a center change, before departures are removed.
        if self
            .sent
            .iter()
            .all(|slot| slot.is_some_and(|pos| self.center.contains(pos)))
        {
            return None;
        }
        // Near-first Chebyshev rings; at most 25 comparisons against 25 slots.
        for radius in 0i32..=VIEW_RADIUS {
            for z in -radius..=radius {
                for x in -radius..=radius {
                    if x.abs().max(z.abs()) != radius {
                        continue;
                    }
                    let pos = ChunkPos {
                        x: self.center.x + x,
                        z: self.center.z + z,
                    };
                    if !self.contains(pos) {
                        return Some(pos);
                    }
                }
            }
        }
        None
    }
    pub fn insert(&mut self, pos: ChunkPos) -> bool {
        if !self.center.contains(pos) || self.contains(pos) {
            return false;
        }
        if let Some(slot) = self.sent.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(pos);
            true
        } else {
            false
        }
    }
}

/// Fixed M2A live-world safety cap, not a capacity promise.
pub const fn instance_limit() -> usize {
    instance::MAX_INSTANCES
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn virtual_material_and_negative_coordinates() {
        let world = VirtualStone;
        assert_eq!(world.block(-65), Material::Air);
        assert_eq!(world.block(-64), Material::Stone);
        assert_eq!(world.block(63), Material::Stone);
        assert_eq!(world.block(64), Material::Air);
        for (x, chunk) in [
            (-16.01, -2),
            (-16.0, -1),
            (-0.01, -1),
            (0.0, 0),
            (15.99, 0),
            (16.0, 1),
        ] {
            assert_eq!(
                ChunkPos::from_position(x, x),
                Some(ChunkPos { x: chunk, z: chunk })
            );
        }
        assert!(ChunkPos::from_position(f64::NAN, 0.0).is_none());
    }
    #[test]
    fn walking_edges_unloads_only_departures_and_retains_overlap() {
        let mut view = ChunkView::default();
        while let Some(pos) = view.next_missing() {
            assert!(view.insert(pos));
        }
        assert_eq!(view.loaded(), 25);
        view.set_center(ChunkPos { x: 1, z: 0 });
        let mut removed = 0;
        while let Some(pos) = view.next_unload() {
            assert_eq!(pos.x, -2);
            view.remove(pos);
            removed += 1;
        }
        assert_eq!(removed, 5);
        let mut added = 0;
        while let Some(pos) = view.next_missing() {
            assert_eq!(pos.x, 3);
            assert!(view.insert(pos));
            added += 1;
        }
        assert_eq!(added, 5);
        assert_eq!(view.loaded(), 25);
    }
    #[test]
    fn churn_stays_fixed_size_and_deduplicated() {
        let mut view = ChunkView::default();
        for x in -100..100 {
            view.set_center(ChunkPos { x, z: -x });
            while let Some(pos) = view.next_unload() {
                view.remove(pos);
            }
            while let Some(pos) = view.next_missing() {
                assert!(view.insert(pos));
                assert!(!view.insert(pos));
            }
            assert_eq!(view.loaded(), 25);
        }
        assert!(std::mem::size_of::<ChunkView>() <= 512);
    }
}
