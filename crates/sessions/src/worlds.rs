//! Trusted in-process M2A control plane. Locks cover lifecycle/admin work only;
//! movement and chunk serialization consume per-world immutable watch snapshots.
use smolmc_world::{
    InstanceOverlay, Material, OverlayMemory, WorldError, WorldHandle, WorldId, WorldManager,
    WorldProfile, WorldState,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

pub type Terrain = InstanceOverlay;
struct Runtime {
    terrain: watch::Sender<Terrain>,
}
struct Player {
    world: WorldId,
    target: watch::Sender<WorldHandle>,
}
struct Catalog {
    manager: WorldManager,
    worlds: HashMap<WorldId, Runtime>,
    players: HashMap<i32, Player>,
    routes: Vec<WorldId>,
}
pub struct Worlds {
    catalog: Mutex<Catalog>,
}
pub struct Membership {
    worlds: Arc<Worlds>,
    entity: i32,
    pub handle: WorldHandle,
    pub target: watch::Receiver<WorldHandle>,
    pub terrain: watch::Receiver<Terrain>,
}
impl Worlds {
    pub fn new(manager: WorldManager, seed: WorldId) -> Self {
        Self {
            catalog: Mutex::new(Catalog {
                manager,
                worlds: HashMap::new(),
                players: HashMap::new(),
                routes: vec![seed],
            }),
        }
    }
    pub fn route(&self, sequence: usize) -> Result<WorldId, WorldError> {
        let c = self.catalog.lock().unwrap();
        if c.routes.is_empty() {
            return Err(WorldError::UnknownInstance);
        }
        Ok(c.routes[sequence % c.routes.len()])
    }
    pub fn handle(&self, id: WorldId) -> Result<WorldHandle, WorldError> {
        self.catalog.lock().unwrap().manager.handle(id)
    }
    pub fn create(&self) -> Result<WorldHandle, WorldError> {
        self.create_profiled(WorldProfile::default())
    }
    pub fn create_profiled(&self, profile: WorldProfile) -> Result<WorldHandle, WorldError> {
        let mut c = self.catalog.lock().unwrap();
        // Exactly the existing virtual-stone template; generalized imports/profiles are M2B+.
        let id = c
            .manager
            .create_profiled(smolmc_world::TemplateId(1), [8, 65, 8], profile)?;
        c.routes.push(id);
        eprintln!("world={} event=created generation=1", id.0);
        c.manager.handle(id)
    }
    pub fn edit(
        &self,
        handle: WorldHandle,
        pos: [i32; 3],
        material: Material,
    ) -> Result<(), WorldError> {
        self.edit_batch(handle, &[(pos, material)])
    }
    pub fn edit_batch(
        &self,
        handle: WorldHandle,
        edits: &[([i32; 3], Material)],
    ) -> Result<(), WorldError> {
        let mut c = self.catalog.lock().unwrap();
        c.manager.set_blocks(handle, edits)?;
        if let Some(runtime) = c.worlds.get(&handle.id) {
            runtime
                .terrain
                .send_replace(c.manager.instance(handle.id)?.overlay.clone());
        }
        Ok(())
    }
    pub fn clone_world(&self, source: WorldHandle) -> Result<WorldHandle, WorldError> {
        let mut c = self.catalog.lock().unwrap();
        let handle = c.manager.clone_instance(source)?;
        c.routes.push(handle.id);
        Ok(handle)
    }
    pub fn park(&self, handle: WorldHandle) -> Result<(), WorldError> {
        self.catalog.lock().unwrap().manager.park(handle)
    }
    pub fn memory(&self) -> OverlayMemory {
        self.catalog.lock().unwrap().manager.overlay_memory()
    }
    pub fn resources(&self, handle: WorldHandle) -> Result<WorldResources, WorldError> {
        let c = self.catalog.lock().unwrap();
        let i = c.manager.validate(handle)?;
        Ok(WorldResources {
            state: i.state,
            profile: i.profile,
            players: i.players,
            edits: i.overlay.edit_count(),
            modified_chunks: i.overlay.modified_chunks(),
            overlay_charged_bytes: i.overlay.logical_bytes(),
            owned_tasks: 0,
            owned_timers: 0,
        })
    }
    pub fn reset(&self, handle: WorldHandle) -> Result<WorldHandle, WorldError> {
        let mut c = self.catalog.lock().unwrap();
        let new = c.manager.reset(handle)?;
        // Occupied reset is rejected, so idle worlds retain no watch allocation.
        c.worlds.remove(&handle.id);
        eprintln!(
            "world={} event=reset generation={}",
            handle.id.0, new.generation
        );
        Ok(new)
    }
    pub fn destroy(&self, handle: WorldHandle) -> Result<(), WorldError> {
        let mut c = self.catalog.lock().unwrap();
        c.manager.destroy(handle)?;
        c.worlds.remove(&handle.id);
        c.routes.retain(|id| *id != handle.id);
        if c.routes.is_empty() {
            c.routes = Vec::new();
        }
        if c.worlds.is_empty() {
            c.worlds = HashMap::new();
        }
        eprintln!(
            "world={} event=destroyed generation={}",
            handle.id.0, handle.generation
        );
        Ok(())
    }
    pub fn switch(&self, entity: i32, target: WorldHandle) -> Result<(), WorldError> {
        let c = self.catalog.lock().unwrap();
        c.manager.validate(target)?;
        c.players
            .get(&entity)
            .ok_or(WorldError::UnknownPlayer)?
            .target
            .send_replace(target);
        Ok(())
    }
    pub fn attach(
        self: &Arc<Self>,
        entity: i32,
        handle: WorldHandle,
    ) -> Result<Membership, WorldError> {
        let mut c = self.catalog.lock().unwrap();
        c.manager.validate(handle)?;
        if c.players.len() >= crate::MAX_PLAYERS || c.players.contains_key(&entity) {
            return Err(WorldError::CapacityExceeded);
        }
        c.manager.enter(handle.id)?;
        let (target, receiver) = watch::channel(handle);
        let terrain = subscribe(&mut c, handle.id);
        c.players.insert(
            entity,
            Player {
                world: handle.id,
                target,
            },
        );
        eprintln!("entity={entity} world={} event=entered", handle.id.0);
        Ok(Membership {
            worlds: Arc::clone(self),
            entity,
            handle,
            target: receiver,
            terrain,
        })
    }
    pub fn empty(&self) -> bool {
        self.catalog.lock().unwrap().players.is_empty()
    }
}
impl Membership {
    pub fn move_to(&mut self, target: WorldHandle) -> Result<(), WorldError> {
        let mut c = self.worlds.catalog.lock().unwrap();
        c.manager.validate(target)?;
        c.manager.enter(target.id)?;
        c.manager.leave(self.handle.id)?;
        c.players.get_mut(&self.entity).unwrap().world = target.id;
        if c.manager.instance(self.handle.id)?.players == 0 {
            c.worlds.remove(&self.handle.id);
        }
        self.handle = target;
        self.terrain = subscribe(&mut c, target.id);
        eprintln!(
            "entity={} world={} event=switched generation={}",
            self.entity, target.id.0, target.generation
        );
        Ok(())
    }
}
impl Drop for Membership {
    fn drop(&mut self) {
        let mut c = self.worlds.catalog.lock().unwrap();
        if let Some(player) = c.players.remove(&self.entity) {
            c.manager
                .leave(player.world)
                .expect("membership pins occupied world");
            if c.manager.instance(player.world).unwrap().players == 0 {
                c.worlds.remove(&player.world);
            }
            if c.worlds.is_empty() {
                c.worlds = HashMap::new();
            }
        }
        if c.players.is_empty() {
            c.players = HashMap::new();
        }
    }
}

fn subscribe(c: &mut Catalog, id: WorldId) -> watch::Receiver<Terrain> {
    let overlay = c.manager.instance(id).unwrap().overlay.clone();
    c.worlds
        .entry(id)
        .or_insert_with(|| {
            let (terrain, _) = watch::channel(overlay);
            Runtime { terrain }
        })
        .terrain
        .subscribe()
}
#[derive(Clone, Copy, Debug)]
pub struct WorldResources {
    pub state: WorldState,
    pub profile: WorldProfile,
    pub players: usize,
    pub edits: usize,
    pub modified_chunks: usize,
    pub overlay_charged_bytes: usize,
    /// Actual world-owned construction counts. The shared directory/session and
    /// runtime timers are not charged as per-world workers.
    pub owned_tasks: usize,
    pub owned_timers: usize,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn membership_pins_worlds_cancel_releases_and_stale_requests_fail() {
        let service = crate::Service::new().unwrap();
        let a = service.default_world().unwrap();
        let b = service.create_world().unwrap();
        let membership = service.worlds.attach(1, a).unwrap();
        assert_eq!(service.destroy_world(a), Err(WorldError::Occupied));
        assert_eq!(service.reset_world(a), Err(WorldError::Occupied));
        let pin = Arc::clone(&service.worlds);
        let task = tokio::spawn(async move {
            let _membership = membership;
            std::future::pending::<()>().await;
        });
        tokio::task::yield_now().await;
        task.abort();
        let _ = task.await;
        assert!(pin.empty());
        let new = service.reset_world(a).unwrap();
        assert!(matches!(
            service.worlds.attach(1, a),
            Err(WorldError::StaleHandle)
        ));
        let mut member = service.worlds.attach(1, new).unwrap();
        service.switch_player(1, b).unwrap();
        service.destroy_world(b).unwrap();
        assert_eq!(member.move_to(b), Err(WorldError::UnknownInstance));
        assert_eq!(service.destroy_world(new), Err(WorldError::Occupied));
        drop(member);
        service.destroy_world(new).unwrap();
        service.shutdown().await.unwrap();
    }
    #[test]
    fn reset_and_destroy_release_terrain_snapshots() {
        let service = crate::Service::new().unwrap();
        let a = service.default_world().unwrap();
        service.edit_world(a, [8, 62, 8], Material::Air).unwrap();
        let held = {
            let c = service.worlds.catalog.lock().unwrap();
            assert!(c.worlds.is_empty());
            c.manager.instance(a.id).unwrap().overlay.clone()
        };
        let used = service.world_overlay_memory().charged_bytes;
        assert!(used > 0);
        let a = service.reset_world(a).unwrap();
        assert_eq!(service.world_overlay_memory().charged_bytes, used); // retained reader still charged
        service.destroy_world(a).unwrap();
        drop(held);
        assert_eq!(service.world_overlay_memory().charged_bytes, 0);
    }
}

#[cfg(test)]
mod m2b_tests {
    use super::*;
    #[test]
    fn concurrent_lifecycle_clone_isolation_and_cleanup() {
        let service = Arc::new(crate::Service::new().unwrap());
        std::thread::scope(|scope| {
            for thread in 0..4 {
                let service = Arc::clone(&service);
                scope.spawn(move || {
                    for round in 0..100 {
                        let a = service.create_world().unwrap();
                        let pos = [thread * 16, 0, round];
                        service.edit_world(a, pos, Material::Air).unwrap();
                        let b = service.clone_world(a).unwrap();
                        service.edit_world(b, pos, Material::Stone).unwrap();
                        assert_eq!(service.world_resources(a).unwrap().edits, 1);
                        assert_eq!(service.world_resources(b).unwrap().edits, 0);
                        let fresh = service.reset_world(b).unwrap();
                        assert!(matches!(
                            service.world_resources(b),
                            Err(WorldError::StaleHandle)
                        ));
                        service.park_world(fresh).unwrap();
                        let r = service.world_resources(fresh).unwrap();
                        assert_eq!(r.owned_tasks + r.owned_timers, 0);
                        service.destroy_world(a).unwrap();
                        service.destroy_world(fresh).unwrap();
                    }
                });
            }
        });
        assert_eq!(service.world_overlay_memory().charged_bytes, 0);
        assert!(service.worlds.catalog.lock().unwrap().worlds.is_empty());
    }
    #[test]
    fn per_world_profile_and_lazy_watch_ownership() {
        let service = crate::Service::new().unwrap();
        let readonly = service
            .create_world_with_profile(WorldProfile::read_only())
            .unwrap();
        assert_eq!(
            service.edit_world(readonly, [0, 0, 0], Material::Air),
            Err(WorldError::FeatureDisabled)
        );
        let profile = WorldProfile::resolve(
            &[smolmc_world::Feature::WorldEdits],
            smolmc_world::WorldLimits {
                max_modified_chunks: 1,
                ..smolmc_world::WorldLimits::default()
            },
        )
        .unwrap();
        let world = service.create_world_with_profile(profile).unwrap();
        assert_eq!(
            service.edit_world_batch(
                world,
                &[([0, 0, 0], Material::Air), ([16, 0, 0], Material::Air)]
            ),
            Err(WorldError::WorldBudgetExceeded)
        );
        assert_eq!(service.world_resources(world).unwrap().edits, 0);
        assert_eq!(service.world_overlay_memory().charged_bytes, 0);
        assert!(service.worlds.catalog.lock().unwrap().worlds.is_empty());
        let member = service.worlds.attach(1, world).unwrap();
        assert_eq!(service.worlds.catalog.lock().unwrap().worlds.len(), 1);
        drop(member);
        assert!(service.worlds.catalog.lock().unwrap().worlds.is_empty());
        assert_eq!(
            service.world_resources(world).unwrap().state,
            WorldState::Parked
        );
        service.destroy_world(world).unwrap();
        service.destroy_world(readonly).unwrap();
    }
}
