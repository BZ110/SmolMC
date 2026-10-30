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
    pub(crate) fn reserve(self: &Arc<Self>, bytes: usize) -> Result<Charge, WorldError> {
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
pub(crate) struct Charge {
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
    entries: Box<[(u32, Material)]>,
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
    lights: Option<Arc<LightIndex>>,
}
#[derive(Debug)]
struct LightSection {
    data: Arc<[u8]>,
    _charge: Charge,
}
#[derive(Debug)]
struct LightIndex {
    entries: Box<[(crate::lighting::LightKey, Arc<LightSection>)]>,
    bytes: usize,
    _charge: Charge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceError {
    CapacityExceeded,
}
fn local(pos: [i32; 3]) -> (ChunkPos, u32) {
    (
        ChunkPos {
            x: pos[0].div_euclid(16),
            z: pos[2].div_euclid(16),
        },
        (((pos[1] - crate::MIN_Y) as u32) << 8)
            | ((pos[2].rem_euclid(16) as u32) << 4)
            | pos[0].rem_euclid(16) as u32,
    )
}
fn absolute(chunk: ChunkPos, key: u32) -> [i32; 3] {
    [
        chunk.x * 16 + (key & 15) as i32,
        crate::MIN_Y + (key >> 8) as i32,
        chunk.z * 16 + ((key >> 4) & 15) as i32,
    ]
}
impl InstanceOverlay {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn same_snapshot(&self, other: &Self) -> bool {
        self.same_blocks(other)
            && match (&self.lights, &other.lights) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                _ => false,
            }
    }
    pub fn same_blocks(&self, other: &Self) -> bool {
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
        if !(crate::MIN_Y..crate::MIN_Y + crate::HEIGHT).contains(&pos[1]) {
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
            + self.lights.as_ref().map_or(0, |l| l.bytes)
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
                + entries.len() * size_of::<(u32, Material)>();
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
            return Ok(Self {
                index: None,
                lights: self.lights.clone(),
            });
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
            lights: self.lights.clone(),
            index: Some(Arc::new(Index {
                chunks: chunks.into_boxed_slice(),
                count,
                logical_bytes,
                _charge: budget.reserve(index_bytes)?,
            })),
        })
    }
    pub fn sky_section(&self, key: crate::lighting::LightKey) -> Option<&[u8]> {
        let l = self.lights.as_ref()?;
        l.entries
            .binary_search_by_key(&key, |e| e.0)
            .ok()
            .map(|i| l.entries[i].1.data.as_ref())
    }
    pub fn light_sections(&self) -> impl Iterator<Item = (crate::lighting::LightKey, &[u8])> {
        self.lights
            .iter()
            .flat_map(|l| l.entries.iter().map(|(key, s)| (*key, s.data.as_ref())))
    }
    pub(crate) fn with_lighting(
        &self,
        base: &crate::terrain::ImportedTerrain,
        patch: crate::lighting::LightPatch,
        limits: WorldLimits,
        budget: &Arc<OverlayBudget>,
    ) -> Result<Self, WorldError> {
        let mut entries = self
            .lights
            .as_ref()
            .map_or_else(Vec::new, |l| l.entries.to_vec());
        for (key, data) in patch {
            let base = base
                .chunks
                .get(&key.0)
                .and_then(|c| c.light_sections.get(&key.1))
                .and_then(|l| l.0.as_ref());
            let found = entries.binary_search_by_key(&key, |e| e.0);
            if base.map(AsRef::as_ref) == Some(data.as_ref())
                || (base.is_none() && data.iter().all(|b| *b == 255))
            {
                if let Ok(i) = found {
                    entries.remove(i);
                }
                continue;
            }
            if found.is_ok_and(|i| entries[i].1.data.as_ref() == data.as_ref()) {
                continue;
            }
            let bytes = ARC_HEADER + size_of::<LightSection>() + data.len();
            let node = Arc::new(LightSection {
                data,
                _charge: budget.reserve(bytes)?,
            });
            match found {
                Ok(i) => entries[i].1 = node,
                Err(i) => entries.insert(i, (key, node)),
            }
        }
        let lights = if entries.is_empty() {
            None
        } else {
            let root = ARC_HEADER
                + size_of::<LightIndex>()
                + entries.len() * size_of::<(crate::lighting::LightKey, Arc<LightSection>)>();
            let bytes = root + entries.iter().map(|(_, s)| s._charge.bytes).sum::<usize>();
            if bytes + self.index.as_ref().map_or(0, |i| i.logical_bytes) > limits.max_overlay_bytes
            {
                return Err(WorldError::WorldBudgetExceeded);
            }
            Some(Arc::new(LightIndex {
                entries: entries.into_boxed_slice(),
                bytes,
                _charge: budget.reserve(root)?,
            }))
        };
        Ok(Self {
            index: self.index.clone(),
            lights,
        })
    }
    // Legacy standalone helper; managed worlds always use their shared budget.
    pub fn set_block(&mut self, pos: [i32; 3], m: Material) -> Result<(), InstanceError> {
        if !(crate::MIN_Y..crate::MIN_Y + crate::HEIGHT).contains(&pos[1]) {
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
