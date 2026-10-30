//! Sparse immutable chunk snapshots. Clone shares all data; a write copies only
//! the chunk index and touched sparse chunks. Empty overlays allocate nothing.
use crate::{ChunkPos, Material, WorldError, WorldLimits};
use std::{
    mem::size_of,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering::Relaxed},
    },
};
const ARC_HEADER: usize = 2 * size_of::<usize>();
pub const MAX_EDIT_BATCH: usize = 4096;
pub const DEFAULT_OVERLAY_BUDGET: usize = 64 * 1024 * 1024;
#[derive(Debug)]
pub struct OverlayBudget {
    limit: usize,
    used: AtomicUsize,
    peak: AtomicUsize,
}
impl OverlayBudget {
    pub fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self {
            limit,
            used: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        })
    }
    pub fn snapshot(&self) -> OverlayMemory {
        OverlayMemory {
            limit: self.limit,
            charged_bytes: self.used.load(Relaxed),
            peak_charged_bytes: self.peak.load(Relaxed),
        }
    }
    fn reserve(self: &Arc<Self>, bytes: usize) -> Result<Charge, WorldError> {
        let old = self
            .used
            .try_update(Relaxed, Relaxed, |n| {
                n.checked_add(bytes).filter(|n| *n <= self.limit)
            })
            .map_err(|_| WorldError::GlobalOverlayBudgetExceeded)?;
        self.peak.fetch_max(old + bytes, Relaxed);
        Ok(Charge {
            budget: Arc::clone(self),
            bytes,
        })
    }
}
/// Known Arc headers, structs and boxed payload lengths. Excludes allocator
/// metadata/rounding, temporary mutation buffers and OS/runtime memory; not RSS.
#[derive(Clone, Copy, Debug, Default)]
pub struct OverlayMemory {
    pub limit: usize,
    pub charged_bytes: usize,
    pub peak_charged_bytes: usize,
}
#[derive(Debug)]
struct Charge {
    budget: Arc<OverlayBudget>,
    bytes: usize,
}
impl Drop for Charge {
    fn drop(&mut self) {
        self.budget.used.fetch_sub(self.bytes, Relaxed);
    }
}
#[derive(Debug)]
struct EditedChunk {
    pos: ChunkPos,
    entries: Box<[(u16, Material)]>,
    charge: Charge,
}
#[derive(Debug)]
struct Index {
    chunks: Box<[Arc<EditedChunk>]>,
    count: usize,
    logical_bytes: usize,
    _charge: Charge,
}
#[derive(Clone, Debug, Default)]
pub struct InstanceOverlay {
    index: Option<Arc<Index>>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceError {
    CapacityExceeded,
}
fn local(pos: [i32; 3]) -> (ChunkPos, u16) {
    (
        ChunkPos {
            x: pos[0].div_euclid(16),
            z: pos[2].div_euclid(16),
        },
        (((pos[1] - crate::MIN_Y) as u16) << 8)
            | ((pos[2].rem_euclid(16) as u16) << 4)
            | pos[0].rem_euclid(16) as u16,
    )
}
fn absolute(chunk: ChunkPos, key: u16) -> [i32; 3] {
    [
        chunk.x * 16 + i32::from(key & 15),
        crate::MIN_Y + i32::from(key >> 8),
        chunk.z * 16 + i32::from((key >> 4) & 15),
    ]
}
impl InstanceOverlay {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn same_snapshot(&self, other: &Self) -> bool {
        match (&self.index, &other.index) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
    pub fn shares_chunk(&self, other: &Self, pos: ChunkPos) -> bool {
        match (self.chunk(pos), other.chunk(pos)) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
    fn chunks(&self) -> &[Arc<EditedChunk>] {
        self.index.as_ref().map_or(&[], |i| &i.chunks)
    }
    fn chunk(&self, pos: ChunkPos) -> Option<&Arc<EditedChunk>> {
        self.chunks()
            .binary_search_by_key(&pos, |c| c.pos)
            .ok()
            .map(|i| &self.chunks()[i])
    }
    pub fn override_at(&self, pos: [i32; 3]) -> Option<Material> {
        if !(crate::MIN_Y..crate::SURFACE - 1).contains(&pos[1]) {
            return None;
        }
        let (chunk, key) = local(pos);
        let entries = &self.chunk(chunk)?.entries;
        entries
            .binary_search_by_key(&key, |e| e.0)
            .ok()
            .map(|i| entries[i].1)
    }
    pub fn get_block(&self, pos: [i32; 3], base: impl Fn(i32) -> Material) -> Material {
        self.override_at(pos).unwrap_or_else(|| base(pos[1]))
    }
    pub fn edits(&self) -> impl Iterator<Item = ([i32; 3], Material)> + '_ {
        self.chunks().iter().flat_map(|c| {
            c.entries
                .iter()
                .map(move |&(key, m)| (absolute(c.pos, key), m))
        })
    }
    pub fn chunk_edits(&self, pos: ChunkPos) -> impl Iterator<Item = ([i32; 3], Material)> + '_ {
        self.chunk(pos).into_iter().flat_map(move |c| {
            c.entries
                .iter()
                .map(move |&(key, m)| (absolute(pos, key), m))
        })
    }
    pub fn edit_count(&self) -> usize {
        self.index.as_ref().map_or(0, |i| i.count)
    }
    pub fn modified_chunks(&self) -> usize {
        self.chunks().len()
    }
    pub fn logical_bytes(&self) -> usize {
        self.index.as_ref().map_or(0, |i| i.logical_bytes)
    }
    /// Transactional: all positions must be validated by the world owner first.
    /// None removes an override; duplicates apply in input order (last wins).
    pub(crate) fn apply(
        &self,
        changes: &[([i32; 3], Option<Material>)],
        limits: WorldLimits,
        budget: &Arc<OverlayBudget>,
    ) -> Result<Self, WorldError> {
        if changes.len() > MAX_EDIT_BATCH {
            return Err(WorldError::BatchTooLarge);
        }
        let mut sorted: Vec<_> = changes
            .iter()
            .enumerate()
            .map(|(ordinal, (pos, m))| {
                let (c, k) = local(*pos);
                (c, k, ordinal, *m)
            })
            .collect();
        sorted.sort_unstable_by_key(|e| (e.0, e.1, e.2));
        let mut chunks = self.chunks().to_vec();
        let mut at = 0;
        while at < sorted.len() {
            let pos = sorted[at].0;
            let found = chunks.binary_search_by_key(&pos, |c| c.pos);
            let mut entries = found
                .as_ref()
                .ok()
                .map_or_else(Vec::new, |&i| chunks[i].entries.to_vec());
            while at < sorted.len() && sorted[at].0 == pos {
                let (_, key, _, m) = sorted[at];
                match (entries.binary_search_by_key(&key, |e| e.0), m) {
                    (Ok(i), Some(m)) => entries[i].1 = m,
                    (Ok(i), None) => {
                        entries.remove(i);
                    }
                    (Err(i), Some(m)) => entries.insert(i, (key, m)),
                    (Err(_), None) => {}
                }
                at += 1;
            }
            if let Ok(i) = found
                && chunks[i].entries.as_ref() == entries.as_slice()
            {
                continue;
            }
            if entries.is_empty() {
                if let Ok(i) = found {
                    chunks.remove(i);
                }
                continue;
            }
            let bytes = ARC_HEADER
                + size_of::<EditedChunk>()
                + entries.len() * size_of::<(u16, Material)>();
            let chunk = Arc::new(EditedChunk {
                pos,
                entries: entries.into_boxed_slice(),
                charge: budget.reserve(bytes)?,
            });
            match found {
                Ok(i) => chunks[i] = chunk,
                Err(i) => chunks.insert(i, chunk),
            }
        }
        if chunks.is_empty() {
            return Ok(Self::new());
        }
        if chunks.len() == self.chunks().len()
            && chunks
                .iter()
                .zip(self.chunks())
                .all(|(a, b)| Arc::ptr_eq(a, b))
        {
            return Ok(self.clone());
        }
        let count = chunks.iter().map(|c| c.entries.len()).sum();
        let index_bytes =
            ARC_HEADER + size_of::<Index>() + chunks.len() * size_of::<Arc<EditedChunk>>();
        let logical_bytes = index_bytes + chunks.iter().map(|c| c.charge.bytes).sum::<usize>();
        if count > limits.max_edits {
            return Err(WorldError::OverlayCapacityExceeded);
        }
        if chunks.len() > limits.max_modified_chunks || logical_bytes > limits.max_overlay_bytes {
            return Err(WorldError::WorldBudgetExceeded);
        }
        Ok(Self {
            index: Some(Arc::new(Index {
                chunks: chunks.into_boxed_slice(),
                count,
                logical_bytes,
                _charge: budget.reserve(index_bytes)?,
            })),
        })
    }
    // Legacy standalone helper; managed worlds always use their shared budget.
    pub fn set_block(&mut self, pos: [i32; 3], m: Material) -> Result<(), InstanceError> {
        if !(crate::MIN_Y..crate::SURFACE - 1).contains(&pos[1]) {
            return Err(InstanceError::CapacityExceeded);
        }
        let budget = self.index.as_ref().map_or_else(
            || OverlayBudget::new(DEFAULT_OVERLAY_BUDGET),
            |i| Arc::clone(&i._charge.budget),
        );
        *self = self
            .apply(&[(pos, Some(m))], WorldLimits::default(), &budget)
            .map_err(|_| InstanceError::CapacityExceeded)?;
        Ok(())
    }
    pub fn remove_block(&mut self, pos: [i32; 3]) -> Option<Material> {
        let old = self.override_at(pos)?;
        let budget = Arc::clone(&self.index.as_ref()?._charge.budget);
        *self = self
            .apply(&[(pos, None)], WorldLimits::default(), &budget)
            .ok()?;
        Some(old)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cow_shares_untouched_chunks_and_tracks_pinned_versions() {
        let budget = OverlayBudget::new(64 * 1024);
        let limits = WorldLimits::default();
        let a = InstanceOverlay::new()
            .apply(
                &[
                    ([0, 0, 0], Some(Material::Air)),
                    ([-17, 0, -1], Some(Material::Air)),
                ],
                limits,
                &budget,
            )
            .unwrap();
        let b = a.clone();
        assert!(a.same_snapshot(&b));
        let used = budget.snapshot().charged_bytes;
        let c = b.apply(&[([0, 0, 0], None)], limits, &budget).unwrap();
        assert!(a.shares_chunk(&c, ChunkPos { x: -2, z: -1 }));
        assert_eq!(a.override_at([0, 0, 0]), Some(Material::Air));
        assert_eq!(c.override_at([0, 0, 0]), None);
        assert!(budget.snapshot().charged_bytes > used);
        drop(a);
        drop(b);
        assert_eq!(budget.snapshot().charged_bytes, c.logical_bytes());
        drop(c);
        assert_eq!(budget.snapshot().charged_bytes, 0);
    }
    #[test]
    fn failed_budget_transaction_frees_all_new_nodes() {
        let budget = OverlayBudget::new(512);
        let limits = WorldLimits::default();
        let a = InstanceOverlay::new()
            .apply(&[([0, 0, 0], Some(Material::Air))], limits, &budget)
            .unwrap();
        let before = budget.snapshot().charged_bytes;
        let huge: Vec<_> = (0..200).map(|x| ([x, 0, 0], Some(Material::Air))).collect();
        assert!(matches!(
            a.apply(&huge, limits, &budget),
            Err(WorldError::GlobalOverlayBudgetExceeded)
        ));
        assert_eq!(budget.snapshot().charged_bytes, before);
        assert_eq!(a.edit_count(), 1);
        drop(a);
        assert_eq!(budget.snapshot().charged_bytes, 0);
    }
    #[test]
    fn duplicate_order_revert_and_chunk_boundaries_are_exact() {
        let budget = OverlayBudget::new(64 * 1024);
        let limits = WorldLimits::default();
        let a = InstanceOverlay::new()
            .apply(
                &[
                    ([-1, -64, -17], Some(Material::Air)),
                    ([1, 62, 1], Some(Material::Stone)),
                    ([1, 62, 1], None),
                ],
                limits,
                &budget,
            )
            .unwrap();
        assert_eq!(
            a.edits().collect::<Vec<_>>(),
            vec![([-1, -64, -17], Material::Air)]
        );
        assert_eq!(a.chunk_edits(ChunkPos { x: -1, z: -2 }).count(), 1);
        let b = a.apply(&[([-1, -64, -17], None)], limits, &budget).unwrap();
        assert_eq!(b.logical_bytes(), 0);
    }
}
