//! M2A explicit world identity, templates, instances and per-instance overlays.
//!
//! These types are pure synchronously-evaluated bookkeeping: they never perform,
//! await or block on I/O, sockets, compression, serialization or any network work.
//! A `WorldManager` owns a set of stable-identified instances that reference an
//! immutable template and hold only their own sparse edits. Sessions drive the
//! observable state machine (player enter/leave, disconnect-on-destroy) from short
//! critical sections, never holding these structures across an await.
//!
//! Three nouns stay distinct (see `docs/05_WORLD_INSTANCES.md` and
//! `docs/20_TEMPLATE_FORMAT_AND_LIFECYCLE.md`): a `TemplateId` references
//! logically immutable content, a `WorldId` references a running instance with its
//! own players/overlay/lifecycle, and these are never interchangeable.
use crate::{
    DEFAULT_OVERLAY_BUDGET, Feature, InstanceOverlay, Material, OverlayBudget, OverlayMemory,
    WorldProfile,
};
use std::{
    collections::HashMap,
    sync::Arc,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed},
};

/// Process-unique identifier for a running world instance. Monotonically
/// increasing; destroyed instances never recycle their id while outstanding
/// handles exist, and recycling is impossible by construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorldId(pub u64);

/// Reset-sensitive capability; an old generation cannot edit/reset/destroy a world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldHandle {
    pub id: WorldId,
    pub generation: u64,
}
static NEXT_WORLD: AtomicU64 = AtomicU64::new(1);
pub const MAX_INSTANCES: usize = 4096;
pub const MAX_TEMPLATES: usize = 16;

/// Process-unique identifier for an immutable world template.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TemplateId(pub u64);

/// Absolute world block coordinate. Chunk coordinates use floor division
/// (`world::ChunkPos`), never truncation, so negative columns are correct.
pub type BlockPos = [i32; 3];

/// Immutable dimension descriptor: what the client observes as a "world". The
/// `key` is the dimension-type index into `minecraft:dimension_type`; multiple
/// isolated arenas may legitimately share a key. It is not an instance id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dimension {
    pub key: i32,
    pub min_y: i32,
    pub height: i32,
    /// First player spawn in the instance; used when respawning/aligning.
    pub spawn: BlockPos,
}

/// Structural signature of a template's content. Non-cryptographic identity used
/// to detect two templates describing identical content; a future cache may share
/// precomputed chunk bytes when signatures match. Not security-sensitive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TemplateSignature {
    pub surface: i32,
    pub material_below: Material,
    pub dimension: Dimension,
}

/// Immutable template content descriptor. Never mutated after creation; instances
/// reference it and start from an empty overlay that reads through to it.
#[derive(Clone, Debug)]
pub struct WorldTemplate {
    pub id: TemplateId,
    pub signature: TemplateSignature,
}

impl WorldTemplate {
    /// Effective material at a world `y` layer for the template's geometry. Kept as
    /// a lookup so templates stay decoupled from any concrete chunk encoder.
    pub fn material_at_y(&self, y: i32) -> Material {
        if (self.signature.dimension.min_y..self.signature.surface).contains(&y) {
            self.signature.material_below
        } else {
            Material::Air
        }
    }
}

#[cfg(test)]
const MAX_OVERWRITE_EDITS: usize = 1 << 16;
/// Per-instance lifecycle. Transitions are idempotent and cancellation-aware from
/// the caller's perspective; players cannot enter a not-ready instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldState {
    /// Metadata allocated; template not yet bound.
    Declared,
    /// Template reference being resolved.
    LoadingTemplate,
    /// Ready to accept players.
    Ready,
    /// Has at least one outstanding player.
    Active,
    /// No players and no due work; idle but retained.
    Parked,
    /// Overlay/players being reset; generation advanced.
    Resetting,
    /// Being torn down; new work rejected.
    Destroying,
    /// Removed from the manager; id retired and never reused.
    Destroyed,
}

/// A live world instance: its identity, template binding, lifecycle, per-instance
/// overlay and outstanding player bookkeeping. All fields are plain data; the
/// manager mutates them under short critical sections only.
#[derive(Debug)]
pub struct WorldInstance {
    pub id: WorldId,
    pub template: Arc<WorldTemplate>,
    pub profile: WorldProfile,
    pub state: WorldState,
    pub overlay: InstanceOverlay,
    /// Outstanding players currently in the instance (enter/leave), used by the
    /// destroy policy: relocate, disconnect or reject when occupied.
    pub players: usize,
    /// Advances on every reset; stale handles (generation-checked) cannot act.
    pub generation: u64,
}

/// Metrics for the world-instance subsystem. Aggregate counters; measured, not
/// claimed as production capacity.
#[derive(Debug, Default)]
pub struct WorldMetrics {
    templates_created: AtomicU64,
    instances_created: AtomicU64,
    instances_destroyed: AtomicU64,
    instances_reset: AtomicU64,
    active_instances: AtomicUsize,
    peak_active_instances: AtomicUsize,
    overlay_edits: AtomicU64,
}

impl WorldMetrics {
    pub fn snapshot(&self) -> InstanceSnapshot {
        InstanceSnapshot {
            templates_created: self.templates_created.load(Relaxed),
            instances_created: self.instances_created.load(Relaxed),
            instances_destroyed: self.instances_destroyed.load(Relaxed),
            instances_reset: self.instances_reset.load(Relaxed),
            active_instances: self.active_instances.load(Relaxed),
            peak_active_instances: self.peak_active_instances.load(Relaxed),
            overlay_edits: self.overlay_edits.load(Relaxed),
        }
    }
}

#[derive(Debug, Default)]
pub struct InstanceSnapshot {
    pub templates_created: u64,
    pub instances_created: u64,
    pub instances_destroyed: u64,
    pub instances_reset: u64,
    pub active_instances: usize,
    pub peak_active_instances: usize,
    pub overlay_edits: u64,
}

/// Outcomes for manager operations. `Occupied` drives the destroy policy; an
/// occupied instance must be relocated or its players disconnected first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldError {
    UnknownTemplate,
    TemplateConflict,
    CapacityExceeded,
    StaleHandle,
    InvalidPosition,
    UnknownInstance,
    UnknownPlayer,
    InstanceNotReady,
    /// Destroy rejected: players are still in the instance.
    Occupied,
    /// Overlay exceeded its bounded edit budget (mapped from `InstanceError`).
    OverlayCapacityExceeded,
    GlobalOverlayBudgetExceeded,
    WorldBudgetExceeded,
    BatchTooLarge,
    FeatureDisabled,
    UnsupportedFeature,
    InvalidBudget,
}

/// Owns templates and instances together with their lifecycle state machine.
/// Pure bookkeeping; no world tick, per-world worker or timer is scheduled here.
#[derive(Debug)]
pub struct WorldManager {
    templates: HashMap<TemplateId, Arc<WorldTemplate>>,
    instances: HashMap<WorldId, WorldInstance>,
    metrics: WorldMetrics,
    budget: Arc<OverlayBudget>,
}

impl Default for WorldManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldManager {
    pub fn new() -> Self {
        Self::with_overlay_budget(DEFAULT_OVERLAY_BUDGET)
    }
    pub fn with_overlay_budget(bytes: usize) -> Self {
        Self {
            budget: OverlayBudget::new(bytes),
            templates: HashMap::new(),
            instances: HashMap::new(),
            metrics: WorldMetrics::default(),
        }
    }

    /// Register immutable template content. Idempotent for identical ID/signature:
    /// returns without duplicating a shared template. Content is immutable once
    /// published; only identity and lifecycle are tracked here (content-addressing
    /// of chunk bytes is a later milestone).
    pub fn declare_template(
        &mut self,
        id: TemplateId,
        signature: TemplateSignature,
    ) -> Result<(), WorldError> {
        if let Some(existing) = self.templates.get(&id) {
            return if existing.signature == signature {
                Ok(())
            } else {
                Err(WorldError::TemplateConflict)
            };
        }
        if self.templates.len() >= MAX_TEMPLATES {
            return Err(WorldError::CapacityExceeded);
        }
        self.metrics.templates_created.fetch_add(1, Relaxed);
        self.templates
            .insert(id, Arc::new(WorldTemplate { id, signature }));
        Ok(())
    }

    /// Create a fresh instance referencing an existing template. Assigns a
    /// non-recycling process-unique world id and marks the instance Ready.
    pub fn create_instance(
        &mut self,
        template: TemplateId,
        spawn: BlockPos,
    ) -> Result<WorldId, WorldError> {
        self.create_profiled(template, spawn, WorldProfile::default())
    }
    pub fn create_profiled(
        &mut self,
        template: TemplateId,
        spawn: BlockPos,
        profile: WorldProfile,
    ) -> Result<WorldId, WorldError> {
        // Revalidate externally assembled limits before allocation.
        WorldProfile::resolve(
            if profile.enabled(Feature::WorldEdits) {
                &[Feature::WorldEdits]
            } else {
                &[]
            },
            profile.limits,
        )?;
        if !self.templates.contains_key(&template) {
            return Err(WorldError::UnknownTemplate);
        }
        if self.instances.len() >= MAX_INSTANCES {
            return Err(WorldError::CapacityExceeded);
        }
        if spawn != self.templates[&template].signature.dimension.spawn {
            return Err(WorldError::InvalidPosition);
        }
        let id = WorldId(
            NEXT_WORLD
                .try_update(Relaxed, Relaxed, |n| n.checked_add(1))
                .map_err(|_| WorldError::CapacityExceeded)?,
        );
        self.instances.insert(
            id,
            WorldInstance {
                id,
                template: Arc::clone(&self.templates[&template]),
                profile,
                state: WorldState::Ready,
                overlay: InstanceOverlay::new(),
                players: 0,
                generation: 1,
            },
        );
        self.metrics.instances_created.fetch_add(1, Relaxed);
        let active = self.instances.len();
        self.metrics.active_instances.store(active, Relaxed);
        self.metrics
            .peak_active_instances
            .fetch_max(active, Relaxed);
        Ok(id)
    }

    pub fn template(&self, id: TemplateId) -> Option<&WorldTemplate> {
        self.templates.get(&id).map(Arc::as_ref)
    }

    fn check(&self, id: WorldId) -> Result<&WorldInstance, WorldError> {
        match self.instances.get(&id) {
            Some(instance) if instance.state == WorldState::Destroyed => {
                Err(WorldError::UnknownInstance)
            }
            Some(instance)
                if (instance.state == WorldState::Declared
                    || instance.state == WorldState::LoadingTemplate) =>
            {
                Err(WorldError::InstanceNotReady)
            }
            Some(instance) => Ok(instance),
            None => Err(WorldError::UnknownInstance),
        }
    }

    fn check_mut(&mut self, id: WorldId) -> Result<&mut WorldInstance, WorldError> {
        // Guard existence and readiness; each immutable borrow ends before the next.
        let exists = self.instances.contains_key(&id);
        if !exists {
            return Err(WorldError::UnknownInstance);
        }
        let instance = self.instances.get(&id).unwrap();
        if instance.state == WorldState::Destroyed {
            return Err(WorldError::UnknownInstance);
        }
        if matches!(
            instance.state,
            WorldState::Declared | WorldState::LoadingTemplate
        ) {
            return Err(WorldError::InstanceNotReady);
        }
        Ok(self.instances.get_mut(&id).expect("state validated above"))
    }

    /// Acquire one occupancy reference; each enter must have exactly one leave.
    /// Session RAII enforces identity/cancellation ownership above this pure model.
    pub fn enter(&mut self, id: WorldId) -> Result<(), WorldError> {
        let instance = self.check_mut(id)?;
        if instance.state == WorldState::Parked || instance.players == 0 {
            instance.state = WorldState::Active;
        }
        if instance.players >= instance.profile.limits.max_players {
            return Err(WorldError::WorldBudgetExceeded);
        }
        instance.players += 1;
        Ok(())
    }

    /// Leave an instance. Transitions Active -> Parked when the last player departs;
    /// never removes a still-occupied instance. Idempotent no-op when empty.
    pub fn leave(&mut self, id: WorldId) -> Result<(), WorldError> {
        let instance = self.check_mut(id)?;
        if instance.players == 0 {
            return Ok(());
        }
        instance.players -= 1;
        if instance.players == 0 {
            instance.state = WorldState::Parked;
        }
        Ok(())
    }

    /// Reject occupied resets, discard the overlay and advance the generation.
    /// The caller receives a new handle; repeating with the old handle fails.
    pub fn handle(&self, id: WorldId) -> Result<WorldHandle, WorldError> {
        Ok(WorldHandle {
            id,
            generation: self.check(id)?.generation,
        })
    }
    pub fn validate(&self, handle: WorldHandle) -> Result<&WorldInstance, WorldError> {
        let instance = self.check(handle.id)?;
        if instance.generation != handle.generation {
            return Err(WorldError::StaleHandle);
        }
        Ok(instance)
    }
    pub fn reset(&mut self, handle: WorldHandle) -> Result<WorldHandle, WorldError> {
        if self.validate(handle)?.players != 0 {
            return Err(WorldError::Occupied);
        }
        let instance = self.check_mut(handle.id)?;
        let generation = instance
            .generation
            .checked_add(1)
            .ok_or(WorldError::CapacityExceeded)?;
        let removed = instance.overlay.edit_count();
        instance.overlay = InstanceOverlay::new();
        instance.generation = generation;
        instance.state = WorldState::Ready;
        self.metrics.instances_reset.fetch_add(1, Relaxed);
        self.metrics
            .overlay_edits
            .fetch_sub(removed as u64, Relaxed);
        self.handle(handle.id)
    }

    /// Apply a block edit within an instance's overlay. Reads in other instances and
    /// templates are unaffected; bounded by `MAX_OVERWRITE_EDITS`. Rejected when the
    /// instance is not ready or occupied edits would overflow.
    pub fn set_block(
        &mut self,
        handle: WorldHandle,
        pos: BlockPos,
        material: Material,
    ) -> Result<(), WorldError> {
        self.set_blocks(handle, &[(pos, material)])
    }
    pub fn set_blocks(
        &mut self,
        handle: WorldHandle,
        edits: &[(BlockPos, Material)],
    ) -> Result<(), WorldError> {
        let instance = self.validate(handle)?;
        if !instance.profile.enabled(Feature::WorldEdits) {
            return Err(WorldError::FeatureDisabled);
        }
        if edits.len() > crate::MAX_EDIT_BATCH {
            return Err(WorldError::BatchTooLarge);
        }
        let mut changes = Vec::with_capacity(edits.len());
        for &(pos, material) in edits {
            if pos[0].unsigned_abs() > crate::HORIZONTAL_BOUND as u32
                || pos[2].unsigned_abs() > crate::HORIZONTAL_BOUND as u32
                || !(crate::MIN_Y..crate::SURFACE - 1).contains(&pos[1])
            {
                return Err(WorldError::InvalidPosition);
            }
            let base = instance.template.material_at_y(pos[1]);
            changes.push((
                pos,
                if base == material {
                    None
                } else {
                    Some(material)
                },
            ));
        }
        let before = instance.overlay.edit_count();
        let overlay = instance
            .overlay
            .apply(&changes, instance.profile.limits, &self.budget)?;
        let after = overlay.edit_count();
        self.instances.get_mut(&handle.id).unwrap().overlay = overlay;
        if after >= before {
            self.metrics
                .overlay_edits
                .fetch_add((after - before) as u64, Relaxed);
        } else {
            self.metrics
                .overlay_edits
                .fetch_sub((before - after) as u64, Relaxed);
        }
        Ok(())
    }
    pub fn clone_instance(&mut self, source: WorldHandle) -> Result<WorldHandle, WorldError> {
        let src = self.validate(source)?;
        let (template, spawn, profile, overlay) = (
            src.template.id,
            src.template.signature.dimension.spawn,
            src.profile,
            src.overlay.clone(),
        );
        let id = self.create_profiled(template, spawn, profile)?;
        self.metrics
            .overlay_edits
            .fetch_add(overlay.edit_count() as u64, Relaxed);
        self.instances.get_mut(&id).unwrap().overlay = overlay;
        self.handle(id)
    }
    pub fn park(&mut self, handle: WorldHandle) -> Result<(), WorldError> {
        if self.validate(handle)?.players != 0 {
            return Err(WorldError::Occupied);
        }
        self.instances.get_mut(&handle.id).unwrap().state = WorldState::Parked;
        Ok(())
    }
    pub fn overlay_memory(&self) -> OverlayMemory {
        self.budget.snapshot()
    }

    /// Destroy an instance. Rejected if occupied (destroy policy: relocate,
    /// disconnect, or reject). A destroyed instance is removed and its id retired.
    pub fn destroy(&mut self, handle: WorldHandle) -> Result<(), WorldError> {
        self.validate(handle)?;
        let id = handle.id;
        let occupied = match self.instances.get(&id) {
            Some(instance) => instance.players > 0,
            None => return Err(WorldError::UnknownInstance),
        };
        if occupied {
            return Err(WorldError::Occupied);
        }
        let removed = self.instances.remove(&id).unwrap();
        self.metrics
            .overlay_edits
            .fetch_sub(removed.overlay.edit_count() as u64, Relaxed);
        self.metrics.instances_destroyed.fetch_add(1, Relaxed);
        let active = self.instances.len();
        self.metrics.active_instances.store(active, Relaxed);
        if self.instances.is_empty() {
            self.instances = HashMap::new();
        }
        Ok(())
    }

    fn active_instance_count(&self) -> usize {
        self.instances.len()
    }

    pub fn instance(&self, id: WorldId) -> Result<&WorldInstance, WorldError> {
        self.check(id)
    }

    /// Number of live (non-destroyed) instances.
    pub fn active_instances(&self) -> usize {
        self.active_instance_count()
    }

    /// Total overlay edits across all instances; drives shared-memory accounting.
    pub fn overlay_edits(&self) -> u64 {
        self.metrics.overlay_edits.load(Relaxed)
    }

    pub fn metrics(&self) -> &WorldMetrics {
        &self.metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InstanceError;

    fn overworld() -> Dimension {
        Dimension {
            key: 0,
            min_y: -64,
            height: 384,
            spawn: [8, 65, 8],
        }
    }

    fn stone_signature() -> TemplateSignature {
        TemplateSignature {
            surface: 64,
            material_below: Material::Stone,
            dimension: overworld(),
        }
    }

    #[test]
    fn create_reset_destroy_lifecycle() {
        let mut m = WorldManager::new();
        m.declare_template(TemplateId(1), stone_signature())
            .unwrap();
        let a = m.create_instance(TemplateId(1), [8, 65, 8]).unwrap();
        assert_eq!(m.instance(a).unwrap().state, WorldState::Ready);
        // Occupied destroy is rejected until empty.
        m.enter(a).unwrap();
        assert_eq!(m.destroy(m.handle(a).unwrap()), Err(WorldError::Occupied));
        m.leave(a).unwrap();
        assert_eq!(m.destroy(m.handle(a).unwrap()), Ok(()));
        // Destroyed instance is unknown and its id is retired.
        assert!(matches!(m.instance(a), Err(WorldError::UnknownInstance)));
    }

    #[test]
    fn occupancy_references_and_reset_generation() {
        let mut m = WorldManager::new();
        m.declare_template(TemplateId(1), stone_signature())
            .unwrap();
        let a = m.create_instance(TemplateId(1), [8, 65, 8]).unwrap();
        m.enter(a).unwrap();
        m.enter(a).unwrap();
        assert_eq!(m.instance(a).unwrap().players, 2);
        m.leave(a).unwrap();
        m.leave(a).unwrap();
        let gen_before = m.instance(a).unwrap().generation;
        m.reset(m.handle(a).unwrap()).unwrap();
        assert!(m.instance(a).unwrap().generation > gen_before);
    }

    #[test]
    fn unknown_template_and_instance_errors() {
        let mut m = WorldManager::new();
        assert_eq!(
            m.create_instance(TemplateId(9), [8, 65, 8]),
            Err(WorldError::UnknownTemplate)
        );
        m.declare_template(TemplateId(1), stone_signature())
            .unwrap();
        let a = m.create_instance(TemplateId(1), [8, 65, 8]).unwrap();
        assert_eq!(m.enter(WorldId(999)), Err(WorldError::UnknownInstance));
        assert_eq!(m.destroy(m.handle(a).unwrap()), Ok(()));
    }

    #[test]
    fn overlay_reads_through_to_template_and_isolated_per_instance() {
        let mut m = WorldManager::new();
        m.declare_template(TemplateId(1), stone_signature())
            .unwrap();
        let signature = m.template(TemplateId(1)).unwrap().signature; // Copy: avoids borrowing m
        let a = m.create_instance(TemplateId(1), [8, 65, 8]).unwrap();
        let b = m.create_instance(TemplateId(1), [8, 65, 8]).unwrap();
        assert_ne!(a, b);
        let at_y = |y: i32| {
            if (signature.dimension.min_y..signature.surface).contains(&y) {
                signature.material_below
            } else {
                Material::Air
            }
        };
        assert_eq!(
            m.instance(a).unwrap().overlay.get_block([8, 10, 8], at_y),
            Material::Stone
        );
        assert_eq!(
            m.instance(b).unwrap().overlay.get_block([8, 10, 8], at_y),
            Material::Stone
        );
        m.set_block(m.handle(a).unwrap(), [8, 10, 8], Material::Air)
            .unwrap();
        assert_eq!(
            m.instance(a).unwrap().overlay.get_block([8, 10, 8], at_y),
            Material::Air
        );
        assert_eq!(
            m.instance(b).unwrap().overlay.get_block([8, 10, 8], at_y),
            Material::Stone
        );
        assert_eq!(
            m.instance(a).unwrap().overlay.get_block([8, 200, 8], at_y),
            Material::Air
        );
        let mut huge = InstanceOverlay::new();
        let budget = OverlayBudget::new(DEFAULT_OVERLAY_BUDGET);
        for base in (0..MAX_OVERWRITE_EDITS).step_by(crate::MAX_EDIT_BATCH) {
            let edits: Vec<_> = (base..base + crate::MAX_EDIT_BATCH)
                .map(|n| {
                    let z = n as i32;
                    (
                        [
                            z % 16,
                            crate::MIN_Y + (z / 256) % 127,
                            (z / 16) % 16 + (z / (127 * 256)) * 16,
                        ],
                        Some(Material::Stone),
                    )
                })
                .collect();
            huge = huge
                .apply(&edits, crate::WorldLimits::default(), &budget)
                .unwrap();
        }
        assert_eq!(
            huge.set_block([0, crate::MIN_Y, 0], Material::Stone),
            Ok(())
        );
        assert_eq!(
            huge.set_block([100, 0, 0], Material::Air),
            Err(InstanceError::CapacityExceeded)
        );
        assert_eq!(m.overlay_edits(), 1);
    }

    #[test]
    fn metrics_track_active_instances_and_peak() {
        let mut m = WorldManager::new();
        m.declare_template(TemplateId(1), stone_signature())
            .unwrap();
        let a = m.create_instance(TemplateId(1), [8, 65, 8]).unwrap();
        let b = m.create_instance(TemplateId(1), [8, 65, 8]).unwrap();
        assert_eq!(m.metrics().snapshot().active_instances, 2);
        assert_eq!(m.metrics().snapshot().peak_active_instances, 2);
        m.destroy(m.handle(a).unwrap()).unwrap();
        assert_eq!(m.metrics().snapshot().active_instances, 1);
        m.destroy(m.handle(b).unwrap()).unwrap();
        assert_eq!(m.metrics().snapshot().active_instances, 0);
    }
    #[test]
    fn stale_handles_template_conflicts_and_churn_are_rejected() {
        let mut m = WorldManager::new();
        m.declare_template(TemplateId(1), stone_signature())
            .unwrap();
        let mut conflict = stone_signature();
        conflict.surface += 1;
        assert_eq!(
            m.declare_template(TemplateId(1), conflict),
            Err(WorldError::TemplateConflict)
        );
        let mut last = 0;
        for _ in 0..100 {
            let id = m.create_instance(TemplateId(1), [8, 65, 8]).unwrap();
            assert!(id.0 > last);
            last = id.0;
            let old = m.handle(id).unwrap();
            m.set_block(old, [-1, 62, -17], Material::Air).unwrap();
            assert_eq!(m.metrics().snapshot().overlay_edits, 1);
            assert_eq!(
                m.set_block(old, [8, 63, 8], Material::Air),
                Err(WorldError::InvalidPosition)
            );
            let current = m.reset(old).unwrap();
            assert_eq!(m.metrics().snapshot().overlay_edits, 0);
            assert_eq!(
                m.set_block(old, [8, 62, 8], Material::Air),
                Err(WorldError::StaleHandle)
            );
            assert_eq!(m.reset(old), Err(WorldError::StaleHandle));
            assert_eq!(m.destroy(old), Err(WorldError::StaleHandle));
            m.destroy(current).unwrap();
            assert_eq!(m.destroy(current), Err(WorldError::UnknownInstance));
        }
        assert_eq!(m.active_instances(), 0);
        assert_eq!(m.metrics().snapshot().instances_destroyed, 100);
    }
    #[test]
    fn clone_isolation_reset_reuse_and_budget_failures_are_atomic() {
        let mut m = WorldManager::new();
        m.declare_template(TemplateId(1), stone_signature())
            .unwrap();
        let a = m.create_instance(TemplateId(1), [8, 65, 8]).unwrap();
        let a = m.handle(a).unwrap();
        m.set_block(a, [0, 0, 0], Material::Air).unwrap();
        let used = m.overlay_memory().charged_bytes;
        let b = m.clone_instance(a).unwrap();
        assert!(Arc::ptr_eq(
            &m.validate(a).unwrap().template,
            &m.validate(b).unwrap().template
        ));
        assert!(
            m.validate(a)
                .unwrap()
                .overlay
                .same_snapshot(&m.validate(b).unwrap().overlay)
        );
        assert_eq!(m.overlay_memory().charged_bytes, used);
        m.set_block(b, [0, 0, 0], Material::Stone).unwrap();
        assert_eq!(
            m.validate(a).unwrap().overlay.override_at([0, 0, 0]),
            Some(Material::Air)
        );
        assert_eq!(m.validate(b).unwrap().overlay.edit_count(), 0);
        let b2 = m.reset(b).unwrap();
        assert!(matches!(m.validate(b), Err(WorldError::StaleHandle)));
        m.park(b2).unwrap();
        assert_eq!(m.validate(b2).unwrap().state, WorldState::Parked);
        m.destroy(a).unwrap();
        m.destroy(b2).unwrap();
        assert_eq!(m.overlay_memory().charged_bytes, 0);
        let profile = WorldProfile::resolve(
            &[Feature::WorldEdits],
            crate::WorldLimits {
                max_edits: 1,
                max_players: 1,
                ..crate::WorldLimits::default()
            },
        )
        .unwrap();
        let c = m
            .create_profiled(TemplateId(1), [8, 65, 8], profile)
            .unwrap();
        let c = m.handle(c).unwrap();
        assert_eq!(
            m.set_blocks(c, &[([0, 0, 0], Material::Air), ([1, 0, 0], Material::Air)]),
            Err(WorldError::OverlayCapacityExceeded)
        );
        assert_eq!(m.validate(c).unwrap().overlay.edit_count(), 0);
        m.enter(c.id).unwrap();
        assert_eq!(m.enter(c.id), Err(WorldError::WorldBudgetExceeded));
        m.leave(c.id).unwrap();
        let ro = m
            .create_profiled(TemplateId(1), [8, 65, 8], WorldProfile::read_only())
            .unwrap();
        assert_eq!(
            m.set_block(m.handle(ro).unwrap(), [0, 0, 0], Material::Air),
            Err(WorldError::FeatureDisabled)
        );
        assert_eq!(
            m.set_blocks(
                c,
                &[([0, 0, 0], Material::Air), ([0, 63, 0], Material::Air)]
            ),
            Err(WorldError::InvalidPosition)
        );
        assert_eq!(m.overlay_edits(), 0);
    }
}
