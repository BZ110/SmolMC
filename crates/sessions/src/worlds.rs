//! Trusted in-process M2A control plane. Locks cover lifecycle/admin work only;
//! movement and chunk serialization consume per-world immutable watch snapshots.
use smolmc_world::{Material, WorldError, WorldHandle, WorldId, WorldManager};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

pub type Terrain = HashMap<[i32; 3], Material>;
struct Runtime {
    terrain: watch::Sender<Arc<Terrain>>,
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
    pub terrain: watch::Receiver<Arc<Terrain>>,
}
impl Worlds {
    pub fn new(manager: WorldManager, seed: WorldId) -> Self {
        let (terrain, _) = watch::channel(Arc::new(Terrain::new()));
        Self {
            catalog: Mutex::new(Catalog {
                manager,
                worlds: HashMap::from([(seed, Runtime { terrain })]),
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
        let mut c = self.catalog.lock().unwrap();
        // Exactly the existing virtual-stone template; generalized imports/profiles are M2B+.
        let id = c
            .manager
            .create_instance(smolmc_world::TemplateId(1), [8, 65, 8])?;
        let (terrain, _) = watch::channel(Arc::new(Terrain::new()));
        c.worlds.insert(id, Runtime { terrain });
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
        let mut c = self.catalog.lock().unwrap();
        c.manager.set_block(handle, pos, material)?;
        let edits = c.manager.instance(handle.id)?.overlay.edits().collect();
        c.worlds[&handle.id].terrain.send_replace(Arc::new(edits));
        Ok(())
    }
    pub fn reset(&self, handle: WorldHandle) -> Result<WorldHandle, WorldError> {
        let mut c = self.catalog.lock().unwrap();
        let new = c.manager.reset(handle)?;
        c.worlds[&handle.id]
            .terrain
            .send_replace(Arc::new(Terrain::new()));
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
        let terrain = c.worlds[&handle.id].terrain.subscribe();
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
        self.handle = target;
        self.terrain = c.worlds[&target.id].terrain.subscribe();
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
        }
        if c.players.is_empty() {
            c.players = HashMap::new();
        }
    }
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
        let weak = {
            let c = service.worlds.catalog.lock().unwrap();
            Arc::downgrade(&c.worlds[&a.id].terrain.borrow())
        };
        let a = service.reset_world(a).unwrap();
        assert!(weak.upgrade().is_none());
        service.edit_world(a, [8, 62, 8], Material::Air).unwrap();
        let weak = {
            let c = service.worlds.catalog.lock().unwrap();
            Arc::downgrade(&c.worlds[&a.id].terrain.borrow())
        };
        service.destroy_world(a).unwrap();
        assert!(weak.upgrade().is_none());
    }
}
