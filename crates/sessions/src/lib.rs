//! Session protocol phases and trusted world routing; no world simulation.
use smolmc_auth::Auth;
use smolmc_chunks::StoneChunk;
use smolmc_network::{
    ConnectionContext,
    transport::{CONTROL_LIMIT, Connection},
};
use smolmc_protocol_777::{self as wire, Reader, login, play, presence};
pub use smolmc_world::terrain::ImportBounds;
use smolmc_world::{
    ChunkPos, Dimension, MIN_Y, Material, SPAWN, SURFACE, TemplateId, TemplateSignature,
    WorldError, WorldHandle, WorldId, WorldManager,
};
pub mod control;
#[cfg(all(test, unix))]
extern crate self as smolmc_sessions;
pub mod gameplay;
#[cfg(all(test, unix))]
#[path = "../../bridge/src/lib.rs"]
mod java_bridge_fixture;
#[cfg(all(test, unix))]
mod m1d_probe;
#[cfg(test)]
mod perf;
mod player;
mod replication;
mod worlds;
pub use smolmc_world::WorldProfile;
use std::{
    collections::HashSet,
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI32, AtomicU64, AtomicUsize, Ordering::Relaxed},
    },
    time::Duration,
};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore},
    time::{Instant, timeout},
};
pub use worlds::WorldResources;

const HANDSHAKE_SECONDS: u64 = 5;
const LOGIN_SECONDS: u64 = 15;
const CONFIG_SECONDS: u64 = 20;
const MAX_PLAYERS: usize = 512;

fn malformed(error: wire::ProtocolError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
async fn deadline<T>(
    seconds: u64,
    future: impl std::future::Future<Output = io::Result<T>>,
) -> io::Result<T> {
    timeout(Duration::from_secs(seconds), future)
        .await
        .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?
}

#[derive(Default)]
pub struct Metrics {
    authenticated: AtomicU64,
    auth_failures: AtomicU64,
    config_completed: AtomicU64,
    play_entered: AtomicU64,
    play_packets_observed: AtomicU64,
    active_sessions: AtomicUsize,
    peak_sessions: AtomicUsize,
    chunks_sent: AtomicU64,
    chunks_unloaded: AtomicU64,
    chunk_batches_acked: AtomicU64,
    chunk_boundaries: AtomicU64,
    keepalives_acked: AtomicU64,
    teleports_acked: AtomicU64,
    movements: AtomicU64,
    flight_changes: AtomicU64,
    slow_viewers: AtomicU64,
    chat_deliveries: AtomicU64,
    chat_messages: AtomicU64,
    replica_updates: AtomicU64,
    combat_attacks: AtomicU64,
    combat_hits: AtomicU64,
    combat_deaths: AtomicU64,
    combat_respawns: AtomicU64,
    peak_visible_edges: AtomicUsize,
    visible_edges: AtomicUsize,
    active_replicas: AtomicUsize,
}
#[derive(Debug)]
pub struct Snapshot {
    pub authenticated: u64,
    pub auth_failures: u64,
    pub config_completed: u64,
    pub play_entered: u64,
    pub play_packets_observed: u64,
    pub active_sessions: usize,
    pub peak_sessions: usize,
    pub chunks_sent: u64,
    pub chunks_unloaded: u64,
    pub chunk_batches_acked: u64,
    pub chunk_boundaries: u64,
    pub keepalives_acked: u64,
    pub teleports_acked: u64,
    pub movements: u64,
    pub flight_changes: u64,
    pub slow_viewers: u64,
    pub chat_deliveries: u64,
    pub chat_messages: u64,
    pub replica_updates: u64,
    pub combat_attacks: u64,
    pub combat_hits: u64,
    pub combat_deaths: u64,
    pub combat_respawns: u64,
    pub peak_visible_edges: usize,
    pub visible_edges: usize,
    pub active_replicas: usize,
}
impl Metrics {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            authenticated: self.authenticated.load(Relaxed),
            auth_failures: self.auth_failures.load(Relaxed),
            config_completed: self.config_completed.load(Relaxed),
            play_entered: self.play_entered.load(Relaxed),
            play_packets_observed: self.play_packets_observed.load(Relaxed),
            active_sessions: self.active_sessions.load(Relaxed),
            peak_sessions: self.peak_sessions.load(Relaxed),
            chunks_sent: self.chunks_sent.load(Relaxed),
            chunks_unloaded: self.chunks_unloaded.load(Relaxed),
            chunk_batches_acked: self.chunk_batches_acked.load(Relaxed),
            chunk_boundaries: self.chunk_boundaries.load(Relaxed),
            keepalives_acked: self.keepalives_acked.load(Relaxed),
            teleports_acked: self.teleports_acked.load(Relaxed),
            movements: self.movements.load(Relaxed),
            flight_changes: self.flight_changes.load(Relaxed),
            slow_viewers: self.slow_viewers.load(Relaxed),
            chat_deliveries: self.chat_deliveries.load(Relaxed),
            chat_messages: self.chat_messages.load(Relaxed),
            replica_updates: self.replica_updates.load(Relaxed),
            combat_attacks: self.combat_attacks.load(Relaxed),
            combat_hits: self.combat_hits.load(Relaxed),
            combat_deaths: self.combat_deaths.load(Relaxed),
            combat_respawns: self.combat_respawns.load(Relaxed),
            peak_visible_edges: self.peak_visible_edges.load(Relaxed),
            visible_edges: self.visible_edges.load(Relaxed),
            active_replicas: self.active_replicas.load(Relaxed),
        }
    }
}

/// Only admission/leave use this short critical section. No packet processing,
/// network IO, compression, HTTP, or world operations execute under this lock.
struct SessionRegistry {
    ids: Mutex<HashSet<[u8; 16]>>,
    capacity: Arc<Semaphore>,
    metrics: Arc<Metrics>,
}
struct SessionLease {
    registry: Arc<SessionRegistry>,
    uuid: [u8; 16],
    _capacity: OwnedSemaphorePermit,
}
impl SessionRegistry {
    fn claim(self: &Arc<Self>, uuid: [u8; 16]) -> io::Result<SessionLease> {
        let permit = Arc::clone(&self.capacity)
            .try_acquire_owned()
            .map_err(|_| io::Error::new(io::ErrorKind::WouldBlock, "session capacity reached"))?;
        let mut ids = self.ids.lock().unwrap_or_else(|e| e.into_inner());
        if !ids.insert(uuid) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "account already connected",
            ));
        }
        drop(ids);
        let active = self.metrics.active_sessions.fetch_add(1, Relaxed) + 1;
        self.metrics.peak_sessions.fetch_max(active, Relaxed);
        Ok(SessionLease {
            registry: Arc::clone(self),
            uuid,
            _capacity: permit,
        })
    }
}
impl Drop for SessionLease {
    fn drop(&mut self) {
        self.registry
            .ids
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.uuid);
        self.registry.metrics.active_sessions.fetch_sub(1, Relaxed);
    }
}

pub struct RegistrySet {
    packets: Vec<Arc<[u8]>>,
    dimension_id: i32,
    pub encoded_bytes: usize,
    tags: Arc<[u8]>,
    biome_id: i32,
    clock_id: i32,
    player_damage_id: i32,
}
impl RegistrySet {
    pub fn load() -> io::Result<Self> {
        let entries: serde_json::Value =
            serde_json::from_str(include_str!("../data/registry_names.json"))
                .map_err(|_| invalid("registry inventory invalid"))?;
        let entries = entries
            .as_array()
            .ok_or_else(|| invalid("registry inventory invalid"))?;
        if entries.len() != 32 {
            return Err(invalid("registry inventory incomplete"));
        }
        let mut packets = Vec::with_capacity(entries.len());
        let mut dimension_id = None;
        let mut biome_id = None;
        let mut clock_id = None;
        let mut player_damage_id = None;
        for registry in entries {
            let name = registry["registry"]
                .as_str()
                .ok_or_else(|| invalid("registry key missing"))?;
            let names = registry["entries"]
                .as_array()
                .ok_or_else(|| invalid("registry entries missing"))?;
            let names: Vec<String> = names
                .iter()
                .map(|name| {
                    name.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| invalid("invalid registry entry"))
                })
                .collect::<io::Result<_>>()?;
            if names.len() > 4096 || names.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(invalid("registry order invalid"));
            }
            if name == "minecraft:dimension_type" {
                dimension_id = names
                    .iter()
                    .position(|name| name == "minecraft:overworld")
                    .map(|index| index as i32);
            }
            if name == "minecraft:worldgen/biome" {
                biome_id = names
                    .iter()
                    .position(|name| name == "minecraft:plains")
                    .map(|i| i as i32);
            }
            if name == "minecraft:damage_type" {
                player_damage_id = names
                    .iter()
                    .position(|name| name == "minecraft:player_attack")
                    .map(|n| n as i32);
            }
            if name == "minecraft:world_clock" {
                clock_id = names
                    .iter()
                    .position(|name| name == "minecraft:overworld")
                    .map(|i| i as i32);
            }
            packets.push(Arc::<[u8]>::from(login::registry_data(name, &names)));
        }
        let tags: serde_json::Value = serde_json::from_str(include_str!("../data/tag_names.json"))
            .map_err(|_| invalid("tag inventory invalid"))?;
        let mut tag_registries = Vec::new();
        for entry in tags
            .as_array()
            .ok_or_else(|| invalid("tag inventory invalid"))?
        {
            let registry = entry["registry"]
                .as_str()
                .ok_or_else(|| invalid("tag registry missing"))?
                .to_owned();
            let tags = entry["tags"]
                .as_array()
                .ok_or_else(|| invalid("tag entries missing"))?
                .iter()
                .map(|tag| {
                    tag.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| invalid("bad tag name"))
                })
                .collect::<io::Result<Vec<_>>>()?;
            tag_registries.push((registry, tags));
        }
        let tags = Arc::<[u8]>::from(login::empty_named_tags(&tag_registries));
        let encoded_bytes = packets.iter().map(|packet| packet.len()).sum::<usize>() + tags.len();
        Ok(Self {
            packets,
            dimension_id: dimension_id
                .ok_or_else(|| invalid("overworld dimension type missing"))?,
            encoded_bytes,
            player_damage_id: player_damage_id
                .ok_or_else(|| invalid("player attack damage type missing"))?,
            tags,
            biome_id: biome_id.ok_or_else(|| invalid("plains biome missing"))?,
            clock_id: clock_id.ok_or_else(|| invalid("overworld clock missing"))?,
        })
    }
}

pub struct Service {
    auth: Auth,
    registries: RegistrySet,
    registry: Arc<SessionRegistry>,
    pub metrics: Arc<Metrics>,
    protocol_session: [u8; 16],
    next_entity: AtomicI32,
    stone: Option<StoneChunk>,
    imported: Option<Arc<smolmc_world::terrain::ImportedTerrain>>,
    imported_chunks: Option<smolmc_chunks::ImportedChunks>,
    spawn: [f64; 3],
    replication: tokio::sync::OnceCell<replication::Hub>,
    worlds: Arc<worlds::Worlds>,
    seed: WorldId,
    next_route: AtomicUsize,
    gameplay: std::sync::OnceLock<gameplay::Config>,
    gameplay_epoch: Instant,
}

impl Service {
    pub fn new() -> io::Result<Self> {
        Self::new_profile(smolmc_world::WorldProfile::default())
    }
    fn new_profile(profile: smolmc_world::WorldProfile) -> io::Result<Self> {
        Self::new_terrain(profile, None, SPAWN)
    }
    fn new_terrain(
        profile: smolmc_world::WorldProfile,
        imported: Option<Arc<smolmc_world::terrain::ImportedTerrain>>,
        spawn: [f64; 3],
    ) -> io::Result<Self> {
        let metrics = Arc::new(Metrics::default());
        let mut registries = RegistrySet::load()?;
        if imported.is_some() {
            let rows: serde_json::Value =
                serde_json::from_str(include_str!("../data/tag_names.json"))
                    .map_err(|_| invalid("tag inventory"))?;
            let registries_names = rows
                .as_array()
                .ok_or_else(|| invalid("tag inventory"))?
                .iter()
                .map(|row| {
                    (
                        row["registry"].as_str().unwrap().to_owned(),
                        row["tags"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|n| n.as_str().unwrap().to_owned())
                            .collect(),
                    )
                })
                .collect::<Vec<_>>();
            let members = serde_json::from_str(include_str!("../data/mining_tags.json"))
                .map_err(|_| invalid("mining tag metadata"))?;
            registries.encoded_bytes -= registries.tags.len();
            registries.tags = Arc::from(login::named_tags(&registries_names, &members));
            registries.encoded_bytes += registries.tags.len();
        }
        let stone = imported
            .is_none()
            .then(|| StoneChunk::new(registries.biome_id));
        let (mut worlds, mut seed) = Self::bootstrap_worlds(&registries, profile)?;
        let imported_chunks = if let Some(base) = &imported {
            let snapshot = smolmc_world::collision::TerrainSnapshot {
                base: Arc::clone(base),
                overlay: smolmc_world::InstanceOverlay::new(),
            };
            if !spawn.iter().all(|v| v.is_finite()) || !snapshot.grounded(spawn, false) {
                return Err(invalid("imported spawn requires supported ground"));
            }
            let mut manager = WorldManager::new();
            let signature = TemplateSignature {
                surface: SURFACE,
                material_below: Material::Air,
                dimension: Dimension {
                    key: registries.dimension_id,
                    min_y: MIN_Y,
                    height: smolmc_world::HEIGHT,
                    spawn: spawn.map(|n| n.floor() as i32),
                },
            };
            manager
                .declare_imported(TemplateId(1), signature, Arc::clone(base))
                .map_err(|_| invalid("imported template registration"))?;
            seed = manager
                .create_profiled(TemplateId(1), signature.dimension.spawn, profile)
                .map_err(|_| invalid("imported instance creation"))?;
            worlds = manager;
            Some(smolmc_chunks::ImportedChunks::new(
                base,
                registries.biome_id,
                Self::biome_names()?.len(),
            )?)
        } else {
            None
        };
        Ok(Self {
            auth: Auth::new()?,
            registries,
            registry: Arc::new(SessionRegistry {
                ids: Mutex::new(HashSet::new()),
                capacity: Arc::new(Semaphore::new(MAX_PLAYERS)),
                metrics: Arc::clone(&metrics),
            }),
            metrics,
            protocol_session: smolmc_auth::random_uuid()?,
            next_entity: AtomicI32::new(1),
            stone,
            imported,
            imported_chunks,
            spawn,
            replication: tokio::sync::OnceCell::new(),
            worlds: Arc::new(worlds::Worlds::new(worlds, seed)),
            seed,
            next_route: AtomicUsize::new(0),
            gameplay: std::sync::OnceLock::new(),
            gameplay_epoch: Instant::now(),
        })
    }
    pub fn biome_names() -> io::Result<std::collections::HashMap<String, i32>> {
        let rows: serde_json::Value =
            serde_json::from_str(include_str!("../data/registry_names.json"))
                .map_err(|_| invalid("biome inventory"))?;
        let names = rows
            .as_array()
            .and_then(|rows| {
                rows.iter()
                    .find(|r| r["registry"] == "minecraft:worldgen/biome")
            })
            .and_then(|r| r["entries"].as_array())
            .ok_or_else(|| invalid("biome inventory"))?;
        Ok(names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.as_str().unwrap().to_owned(), i as i32))
            .collect())
    }
    pub fn import_instances(
        count: usize,
        profile: smolmc_world::WorldProfile,
        path: &std::path::Path,
        bounds: smolmc_world::terrain::ImportBounds,
        spawn: Option<[f64; 3]>,
        convert: bool,
    ) -> io::Result<Self> {
        let base = smolmc_world::terrain::import_world_policy(
            path,
            bounds,
            &Self::biome_names()?,
            convert,
        )?;
        for (state, count) in &base.removed_palette_entries {
            eprintln!(
                "import_conversion target=minecraft:air palette_entries={count} source={state}"
            );
        }
        let spawn = spawn
            .or_else(|| {
                base.safe_spawn([
                    (bounds.min[0] + bounds.max[0]) / 2,
                    (bounds.min[2] + bounds.max[2]) / 2,
                ])
            })
            .ok_or_else(|| invalid("no safe imported spawn"))?;
        Self::with_terrain_instances(count, profile, base, spawn)
    }
    pub fn with_terrain_instances(
        count: usize,
        profile: smolmc_world::WorldProfile,
        base: Arc<smolmc_world::terrain::ImportedTerrain>,
        spawn: [f64; 3],
    ) -> io::Result<Self> {
        if !(1..=smolmc_world::instance_limit()).contains(&count) {
            return Err(invalid("instances must be 1..4096"));
        }
        let service = Self::new_terrain(profile, Some(base), spawn)?;
        for _ in 1..count {
            service
                .create_world_with_profile(profile)
                .map_err(|_| invalid("imported instance creation"))?;
        }
        Ok(service)
    }
    pub async fn edit_terrain(
        &self,
        world: WorldHandle,
        edits: Vec<([i32; 3], Material)>,
    ) -> Result<(), WorldError> {
        self.worlds.edit_terrain(world, edits).await
    }
    pub fn arena_spawn(&self) -> [f64; 3] {
        self.spawn
    }
    pub fn imported_terrain(&self) -> Option<&Arc<smolmc_world::terrain::ImportedTerrain>> {
        self.imported.as_ref()
    }
    fn world_mode(&self, world: WorldHandle) -> i32 {
        if self.imported.is_some() {
            if self
                .worlds
                .resources(world)
                .is_ok_and(|r| r.profile.enabled(smolmc_world::Feature::BlockInteractions))
            {
                0
            } else {
                2
            }
        } else if self.gameplay_config().enabled {
            2
        } else {
            1
        }
    }
    fn base_sky(&self, pos: ChunkPos, sy: i32) -> Arc<[u8]> {
        self.imported
            .as_ref()
            .and_then(|t| t.chunks.get(&pos))
            .and_then(|c| c.light_sections.get(&sy))
            .and_then(|l| l.0.clone())
            .unwrap_or_else(|| {
                smolmc_world::lighting::uniform(
                    if self
                        .imported
                        .as_ref()
                        .is_none_or(|t| !t.chunks.contains_key(&pos))
                        || sy >= 20
                    {
                        255
                    } else {
                        0
                    },
                )
            })
    }
    fn base_block(&self, pos: [i32; 3]) -> Material {
        self.imported.as_ref().map_or_else(
            || smolmc_world::VirtualStone.block(pos[1]),
            |t| t.material(pos),
        )
    }
    /// Create a bounded set of identical arenas and route successful joins round-robin.
    pub fn with_instances(count: usize) -> io::Result<Self> {
        Self::with_profiled_instances(count, smolmc_world::WorldProfile::default())
    }
    pub fn with_profiled_instances(
        count: usize,
        profile: smolmc_world::WorldProfile,
    ) -> io::Result<Self> {
        if !(1..=smolmc_world::instance_limit()).contains(&count) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "instances must be 1..4096",
            ));
        }
        let service = Self::new_profile(profile)?;
        for _ in 1..count {
            service
                .create_world_with_profile(profile)
                .map_err(|_| io::Error::other("world creation failed"))?;
        }
        Ok(service)
    }
    fn bootstrap_worlds(
        registries: &RegistrySet,
        profile: smolmc_world::WorldProfile,
    ) -> io::Result<(WorldManager, WorldId)> {
        // M2A: seed one template + a single bootstrap instance (process-unique identity). No mutable
        // world state flows through the replication layer here, so construction is benign.
        let mut worlds = WorldManager::new();
        // create_instance + the template dimension use BlockPos (i32) coordinates; SPAWN is
        // [f64; 3] Play-space, so cast to the world's coordinate type.
        let spawn = [SPAWN[0] as i32, SPAWN[1] as i32, SPAWN[2] as i32];
        let signature = TemplateSignature {
            surface: SURFACE,
            material_below: Material::Stone,
            dimension: Dimension {
                key: registries.dimension_id,
                min_y: MIN_Y,
                height: smolmc_world::HEIGHT,
                spawn,
            },
        };
        worlds
            .declare_template(TemplateId(1), signature)
            .map_err(|e| io::Error::other(format!("world bootstrap failed: {e:?}")))?;
        let seed = worlds
            .create_profiled(TemplateId(1), spawn, profile)
            .map_err(|e| io::Error::other(format!("world bootstrap failed: {e:?}")))?;
        Ok((worlds, seed))
    }
    /// Trusted in-process control API; the optional bridge applies its own authorization.
    pub fn default_world(&self) -> Result<WorldHandle, WorldError> {
        self.worlds.handle(self.seed)
    }
    pub fn create_world(&self) -> Result<WorldHandle, WorldError> {
        self.worlds.create()
    }
    pub fn edit_world(
        &self,
        world: WorldHandle,
        pos: [i32; 3],
        material: Material,
    ) -> Result<(), WorldError> {
        self.worlds.edit(world, pos, material)
    }
    pub fn reset_world(&self, world: WorldHandle) -> Result<WorldHandle, WorldError> {
        self.worlds.reset(world)
    }
    pub fn destroy_world(&self, world: WorldHandle) -> Result<(), WorldError> {
        self.worlds.destroy(world)
    }
    /// Accepted requests coalesce; membership commits after outstanding wire ACKs.
    /// A target destroyed/reset before commit causes safe session closure.
    pub fn switch_player(&self, entity: i32, world: WorldHandle) -> Result<(), WorldError> {
        self.worlds.switch(entity, world)
    }
    pub fn create_world_with_profile(
        &self,
        profile: smolmc_world::WorldProfile,
    ) -> Result<WorldHandle, WorldError> {
        self.worlds.create_profiled(profile)
    }
    pub fn clone_world(&self, source: WorldHandle) -> Result<WorldHandle, WorldError> {
        self.worlds.clone_world(source)
    }
    pub fn park_world(&self, world: WorldHandle) -> Result<(), WorldError> {
        self.worlds.park(world)
    }
    pub fn edit_world_batch(
        &self,
        world: WorldHandle,
        edits: &[([i32; 3], Material)],
    ) -> Result<(), WorldError> {
        self.worlds.edit_batch(world, edits)
    }
    pub fn world_resources(&self, world: WorldHandle) -> Result<WorldResources, WorldError> {
        self.worlds.resources(world)
    }
    pub fn world_overlay_memory(&self) -> smolmc_world::OverlayMemory {
        self.worlds.memory()
    }
    /// Only the explicit bridge startup constructs event/callback resources.
    pub fn enable_control(&self) -> Result<(), control::ControlError> {
        self.worlds.enable_control()
    }
    pub fn server_id(&self) -> [u8; 16] {
        self.protocol_session
    }
    pub fn control_events(
        &self,
    ) -> Result<tokio::sync::broadcast::Receiver<Arc<control::ControlEvent>>, control::ControlError>
    {
        self.worlds.events()
    }
    pub fn list_worlds(&self) -> Vec<WorldHandle> {
        self.worlds.list()
    }
    pub fn read_blocks(
        &self,
        world: WorldHandle,
        positions: &[[i32; 3]],
    ) -> Result<Vec<Material>, WorldError> {
        self.worlds.read(world, positions)
    }
    pub fn list_players(&self) -> Vec<control::PlayerSnapshot> {
        self.worlds.players()
    }
    pub fn control_player(
        &self,
        entity: i32,
        uuid: [u8; 16],
        target: Option<WorldHandle>,
        message: Option<String>,
    ) -> Result<(), control::ControlError> {
        self.worlds.control_player(entity, uuid, target, message)
    }
    pub fn register_command(&self, owner: u64, name: &str) -> Result<(), control::ControlError> {
        self.worlds.register_command(owner, name)
    }
    pub fn unregister_commands(&self, owner: u64, name: Option<&str>) {
        self.worlds.unregister_commands(owner, name)
    }
    pub fn invoke_command(&self, command: &str) -> Result<(), control::ControlError> {
        self.worlds.invoke_command(None, command)
    }
    /// Optional startup arena profile; no world/player timer is constructed.
    pub fn configure_pvp(&self, config: gameplay::Config) -> io::Result<()> {
        if !self.worlds.empty()
            || config.spawn.iter().any(|v| !v.is_finite())
            || config.spawn[0].abs() > 99950.
            || config.spawn[2].abs() > 99950.
            || !(if self.imported.is_some() {
                -64.0..=319.0
            } else {
                64.0..=512.0
            })
            .contains(&config.spawn[1])
        {
            return Err(invalid("invalid/late arena configuration"));
        }
        self.gameplay
            .set(config)
            .map_err(|_| invalid("arena already configured"))
    }
    fn gameplay_config(&self) -> gameplay::Config {
        self.gameplay.get().copied().unwrap_or(gameplay::Config {
            enabled: false,
            spawn: self.spawn,
        })
    }
    fn player_damage_type(&self) -> i32 {
        self.registries.player_damage_id
    }
    pub fn combat_snapshot(
        &self,
        entity: i32,
        uuid: [u8; 16],
    ) -> Result<control::CombatSnapshot, control::ControlError> {
        self.worlds.combat_snapshot(entity, uuid)
    }
    pub fn registry_bytes(&self) -> usize {
        self.registries.encoded_bytes
    }
    pub fn chunk_template_bytes(&self) -> usize {
        self.imported_chunks.as_ref().map_or_else(
            || self.stone.as_ref().unwrap().shared_bytes(),
            |c| c.bytes(),
        )
    }
    pub async fn shutdown(&self) -> io::Result<()> {
        self.auth.wait_idle().await?;
        self.worlds.stop_lighting().await?;
        if let Some(hub) = self.replication.get() {
            hub.shutdown().await?;
        }
        if self.auth.in_flight() != 0
            || self.metrics.snapshot().active_sessions != 0
            || !self.worlds.empty()
        {
            return Err(io::Error::other(
                "session resources still owned at shutdown",
            ));
        }
        Ok(())
    }
    pub async fn handle(
        &self,
        mut connection: Connection,
        context: ConnectionContext,
    ) -> io::Result<()> {
        let hello = deadline(HANDSHAKE_SECONDS, async {
            wire::decode_handshake(&connection.inbound.read_packet().await?).map_err(malformed)
        })
        .await?;
        match hello.intent {
            wire::Intent::Status => {
                return deadline(HANDSHAKE_SECONDS, self.status(&mut connection, &context)).await;
            }
            wire::Intent::Transfer => {
                context.login_rejected();
                deadline(
                    HANDSHAKE_SECONDS,
                    connection
                        .outbound
                        .send(&login::disconnect_login("Transfers are not supported.")),
                )
                .await?;
                return connection.outbound.shutdown().await;
            }
            wire::Intent::Login => {}
        }
        if hello.protocol != wire::PROTOCOL_VERSION {
            context.login_rejected();
            deadline(
                HANDSHAKE_SECONDS,
                connection.outbound.send(&login::disconnect_login(
                    "Minecraft 26.3 (protocol 777) is required.",
                )),
            )
            .await?;
            return connection.outbound.shutdown().await;
        }
        let result = deadline(LOGIN_SECONDS, self.login(&mut connection, &context)).await;
        let (lease, entity, profile) = match result {
            Ok(result) => result,
            Err(error) => {
                self.metrics.auth_failures.fetch_add(1, Relaxed);
                context.login_rejected();
                // Negotiation may be incomplete; close rather than invent the
                // client's compression/encryption state for a rejection packet.
                return Err(error);
            }
        };
        let _lease = lease;
        if context.trace {
            eprintln!("connection={} state=configuration", context.id);
        }
        if let Err(error) = deadline(CONFIG_SECONDS, self.configure(&mut connection)).await {
            let message = if error.kind() == io::ErrorKind::Unsupported {
                "SmolMC requires the vanilla minecraft:core:26.3 known pack."
            } else {
                "Configuration failed or exceeded its deadline."
            };
            let _ = deadline(
                1,
                connection
                    .outbound
                    .send(&login::disconnect_common(2, message)),
            )
            .await;
            return Err(error);
        }
        self.metrics.config_completed.fetch_add(1, Relaxed);
        let instance = self
            .worlds
            .route(self.next_route.fetch_add(1, Relaxed))
            .map_err(|_| invalid("no ready world"))?;
        deadline(
            5,
            connection.outbound.send(&login::play_login_with_mode(
                entity,
                self.registries.dimension_id,
                MAX_PLAYERS as i32,
                self.world_mode(
                    self.worlds
                        .handle(instance)
                        .map_err(|_| invalid("unknown world"))?,
                ),
            )),
        )
        .await?;
        self.metrics.play_entered.fetch_add(1, Relaxed);
        if context.trace {
            eprintln!(
                "connection={} state=play login_sent=true online_verified=true",
                context.id
            );
        }
        let result = self
            .play(&mut connection, &context, entity, profile, instance)
            .await;
        if context.trace {
            eprintln!("connection={} play_ended=true", context.id);
        }
        if context.trace
            && let Err(error) = &result
        {
            eprintln!("connection={} play_failure={error}", context.id);
        }
        if let Err(error) = &result
            && matches!(
                error.kind(),
                io::ErrorKind::InvalidData | io::ErrorKind::TimedOut
            )
        {
            let _ = deadline(
                1,
                connection.outbound.send(&login::disconnect_common(
                    0x20,
                    "Connection exceeded a protocol, resource or acknowledgement limit.",
                )),
            )
            .await;
        }
        result
    }
    async fn status(
        &self,
        connection: &mut Connection,
        context: &ConnectionContext,
    ) -> io::Result<()> {
        wire::decode_status_request(&connection.inbound.read_packet().await?).map_err(malformed)?;
        let count = self.metrics.active_sessions.load(Relaxed);
        connection
            .outbound
            .send(&wire::status_response_with(
                "SmolMC M4A: isolated arenas / imported terrain / PvP",
                count,
                MAX_PLAYERS,
            ))
            .await?;
        context.status_sent();
        let nonce =
            wire::decode_ping(&connection.inbound.read_packet().await?).map_err(malformed)?;
        connection.outbound.send(&wire::pong(nonce)).await?;
        context.pong_sent();
        connection.outbound.shutdown().await
    }
    async fn login(
        &self,
        connection: &mut Connection,
        context: &ConnectionContext,
    ) -> io::Result<(SessionLease, i32, Arc<login::Profile>)> {
        let (username, _untrusted_claim) =
            login::login_start(&connection.inbound.read_packet().await?).map_err(malformed)?;
        let challenge = self.auth.begin()?;
        connection
            .outbound
            .send(&login::encryption_request(
                self.auth.public_key(),
                &challenge.token,
            ))
            .await?;
        let (secret, token) =
            login::key_response(&connection.inbound.read_packet().await?).map_err(malformed)?;
        let key = self.auth.decrypt(secret, token, &challenge).await?;
        connection.enable_encryption(&key)?;
        let profile = match self.auth.verify(&username, &key).await {
            Ok(profile) => profile,
            Err(error) => {
                // Both ends have enabled encryption; no compression or success
                // packet has been sent, so this rejection has an exact wire state.
                let reason = if error.kind() == io::ErrorKind::PermissionDenied {
                    "Unable to verify your Minecraft account. Please sign in again."
                } else {
                    "Minecraft account verification is temporarily unavailable."
                };
                let _ = connection
                    .outbound
                    .send(&login::disconnect_login(reason))
                    .await;
                return Err(error);
            }
        };
        drop(key);
        drop(challenge);
        let lease = match self.registry.claim(profile.uuid) {
            Ok(lease) => lease,
            Err(error) => {
                let reason = if error.kind() == io::ErrorKind::AlreadyExists {
                    "This account is already connected."
                } else {
                    "The server has reached its authenticated session capacity."
                };
                let _ = connection
                    .outbound
                    .send(&login::disconnect_login(reason))
                    .await;
                return Err(error);
            }
        };
        let entity = self
            .next_entity
            .try_update(Relaxed, Relaxed, |value| value.checked_add(1))
            .map_err(|_| io::Error::other("entity identifiers exhausted"))?;
        self.metrics.authenticated.fetch_add(1, Relaxed);
        if context.trace {
            eprintln!("connection={} state=authenticated", context.id);
        }
        connection.outbound.send(&login::compression(256)).await?;
        connection.enable_compression(256)?;
        connection.inbound.set_limit(CONTROL_LIMIT)?;
        connection
            .outbound
            .send(&login::login_finished(&profile, &self.protocol_session))
            .await?;
        login::empty_packet(&connection.inbound.read_packet().await?, 3).map_err(malformed)?;
        Ok((lease, entity, Arc::new(profile)))
    }
    async fn configure(&self, connection: &mut Connection) -> io::Result<()> {
        connection.outbound.send(&login::enabled_features()).await?;
        connection
            .outbound
            .send(&login::select_known_packs())
            .await?;
        let mut known = false;
        for _ in 0..16 {
            let packet = connection.inbound.read_packet().await?;
            let mut reader = Reader::new(&packet);
            match reader.varint().map_err(malformed)? {
                0 => login::client_information(&packet, 0).map_err(malformed)?,
                2 => login::custom_payload(&packet, 2).map_err(malformed)?,
                7 => {
                    if !login::known_pack_response(&packet).map_err(malformed)? {
                        return Err(io::Error::new(
                            io::ErrorKind::Unsupported,
                            "known pack declined",
                        ));
                    }
                    known = true;
                    break;
                }
                _ => {
                    return Err(invalid(
                        "unexpected configuration packet before pack acknowledgement",
                    ));
                }
            }
        }
        if !known {
            return Err(invalid("too many pre-configuration packets"));
        }
        for packet in &self.registries.packets {
            connection.outbound.send(packet).await?;
            tokio::task::yield_now().await;
        }
        connection.outbound.send(&self.registries.tags).await?;
        connection.outbound.send(&[3]).await?; // Finish Configuration
        for _ in 0..16 {
            let packet = connection.inbound.read_packet().await?;
            let mut reader = Reader::new(&packet);
            match reader.varint().map_err(malformed)? {
                0 => login::client_information(&packet, 0).map_err(malformed)?,
                2 => login::custom_payload(&packet, 2).map_err(malformed)?,
                3 => {
                    login::empty_packet(&packet, 3).map_err(malformed)?;
                    return Ok(());
                }
                _ => return Err(invalid("unexpected configuration acknowledgement")),
            }
        }
        Err(invalid("too many configuration packets"))
    }
    async fn play(
        &self,
        connection: &mut Connection,
        context: &ConnectionContext,
        entity: i32,
        profile: Arc<login::Profile>,
        instance: WorldId,
    ) -> io::Result<()> {
        let mut config = self.gameplay_config();
        let arena = config.enabled || self.imported.is_some();
        let game = arena.then(|| {
            let mut state = gameplay::State::new(Instant::now(), config.spawn);
            state.align_clock(self.gameplay_epoch);
            state.trace = context.trace;
            state.combat_enabled = config.enabled;
            state.mode = self.world_mode(self.worlds.handle(instance).unwrap());
            Arc::new(Mutex::new(state))
        });
        let damage_type = if config.enabled {
            self.player_damage_type()
        } else {
            0
        };
        let mut respawning = false;
        let mut corpse_due: Option<Instant> = None;
        let mut player = player::Player::new(Instant::now());
        player.spawn_at(config.spawn);
        let mut membership = self
            .worlds
            .attach_game(
                entity,
                self.worlds
                    .handle(instance)
                    .map_err(|_| invalid("unknown world"))?,
                Some(Arc::clone(&profile)),
                game.clone(),
            )
            .map_err(|_| invalid("world membership rejected"))?;
        if let Some(game) = &game
            && self.worlds.control_enabled()
        {
            game.lock().unwrap().notices = Some(gameplay::Notices {
                worlds: Arc::clone(&self.worlds),
                world: membership.handle,
            });
        }
        if let Some(commands) = membership.commands.as_mut() {
            let names = commands.borrow_and_update().clone();
            deadline(5, connection.outbound.send(&play::commands(&names))).await?;
        }
        let mut terrain = membership.terrain.borrow_and_update().clone();
        if self.imported.is_some() {
            player.min_y = -64.;
        }
        let mut geometry =
            self.imported
                .as_ref()
                .map(|base| smolmc_world::collision::TerrainSnapshot {
                    base: Arc::clone(base),
                    overlay: terrain.clone(),
                });
        if let Some(current) = &geometry {
            config.spawn = current
                .safe_spawn(config.spawn)
                .ok_or_else(|| invalid("no safe terrain join point"))?;
            player.spawn_at(config.spawn);
        }
        if let Some(game) = &game {
            let mut g = game.lock().unwrap();
            g.terrain = geometry.clone();
            if geometry.is_some() {
                g.blocks = Some(gameplay::BlockOwner {
                    terrain: membership.terrain.clone(),
                    worlds: Arc::clone(&self.worlds),
                    handle: membership.handle,
                });
                g.build_kit();
                g.reposition(Instant::now(), config.spawn);
            }
        }
        let mut teleport_id = 1i32;
        for packet in [
            play::default_spawn(config.spawn.map(|v| v as i32)),
            if arena {
                wire::pvp::abilities()
            } else {
                play::abilities(false)
            },
            play::static_time(self.registries.clock_id),
            play::center(player.view.center().x, player.view.center().z),
            play::start_loading(),
            play::position(1, config.spawn, [0.0, 0.0]),
            presence::player_info_mode(&profile, self.world_mode(membership.handle)),
            presence::system(
                "[System] Chat is a server relay; player-chat signatures are not verified.",
            )
            .map_err(malformed)?,
        ] {
            deadline(5, connection.outbound.send(&packet)).await?;
        }
        if let Some(game) = &game {
            let (packet, attributes) = {
                let g = game.lock().unwrap();
                (
                    g.inventory.packet(),
                    wire::pvp::attributes(entity, g.inventory.main()),
                )
            };
            deadline(5, connection.outbound.send(&packet)).await?;
            deadline(5, connection.outbound.send(&attributes)).await?;
            deadline(5, connection.outbound.send(&wire::pvp::health(20.))).await?;
        }
        let hub = self
            .replication
            .get_or_init(|| async { replication::Hub::start(Arc::clone(&self.metrics)) })
            .await;
        let (mut replica_lease, mut inbox) = hub
            .join_game(
                entity,
                instance,
                Arc::clone(&profile),
                player.replica(),
                game.clone(),
            )
            .await?;
        let mut wake = Arc::clone(&inbox.wake);
        let mut peer_view = replication::View::default();
        let mut publish_dirty = false;
        let mut publish_due = Instant::now();
        let mut viewer_dirty = false;
        let mut viewer_due = Instant::now();
        let mut scratch =
            Vec::with_capacity(self.stone.as_ref().map_or(0, |s| s.shared_bytes() + 9));
        let mut window = Instant::now();
        let mut packets = 0usize;
        loop {
            let now = Instant::now();
            if corpse_due.is_some_and(|due| now >= due) {
                corpse_due = None;
                player.corpse_visible = false;
                replica_lease.update(player.replica())?;
            }
            player.check_deadlines(now)?;
            if *inbox.closed.borrow() {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "slow replication/chat reader",
                ));
            }
            // Same-dimension switch: drain old batch/teleport ACKs, leave the old
            // directory, remove old entities/Tab entries and chunks, then re-subscribe.
            let target = *membership.target.borrow_and_update();
            if (target != membership.handle || respawning)
                && (!player.dead || respawning)
                && player.pending_batch.is_none()
                && player.pending_teleport.is_none()
            {
                if target != membership.handle {
                    membership
                        .move_to(target)
                        .map_err(|_| invalid("switch target expired"))?;
                }
                if let Some(base) = &self.imported {
                    let current = smolmc_world::collision::TerrainSnapshot {
                        base: Arc::clone(base),
                        overlay: membership.terrain.borrow().clone(),
                    };
                    config.spawn = current
                        .safe_spawn(self.gameplay_config().spawn)
                        .ok_or_else(|| {
                            invalid("no safe terrain spawn near configured arena point")
                        })?;
                }
                if let Some(game) = &game
                    && let Some(notices) = game.lock().unwrap().notices.as_mut()
                {
                    notices.world = membership.handle;
                }
                drop(replica_lease);
                deadline(5, peer_view.clear(&mut connection.outbound)).await?;
                deadline(5, async {
                    for pos in player.view.positions() {
                        connection
                            .outbound
                            .send(&play::unload(pos.x, pos.z))
                            .await?;
                        self.metrics.chunks_unloaded.fetch_add(1, Relaxed);
                    }
                    Ok(())
                })
                .await?;
                teleport_id = teleport_id
                    .checked_add(1)
                    .ok_or_else(|| invalid("teleport IDs exhausted"))?;
                if respawning {
                    self.metrics.combat_respawns.fetch_add(1, Relaxed);
                    deadline(
                        5,
                        connection.outbound.send(&wire::pvp::respawn(
                            self.registries.dimension_id,
                            self.world_mode(target),
                        )),
                    )
                    .await?;
                    if let Some(game) = &game {
                        let mut g = game.lock().unwrap();
                        g.reset(now, config.spawn);
                        if let Some(notices) = &g.notices {
                            notices.worlds.combat_event(
                                13,
                                notices.world,
                                control::PlayerSnapshot::new(entity, &profile, notices.world),
                                control::CombatNotice {
                                    state: g.snapshot(),
                                    source: 0,
                                    damage: 0.,
                                },
                            );
                        }
                    }
                }
                deadline(
                    5,
                    connection
                        .outbound
                        .send(&play::game_mode(self.world_mode(target))),
                )
                .await?;
                if let Some(game) = &game {
                    game.lock().unwrap().mode = self.world_mode(target);
                }
                corpse_due = None;
                player.switch_world(now, teleport_id);
                player.spawn_at(config.spawn);
                if let Some(game) = &game {
                    game.lock().unwrap().reposition(now, config.spawn);
                }
                terrain = membership.terrain.borrow_and_update().clone();
                geometry =
                    self.imported
                        .as_ref()
                        .map(|base| smolmc_world::collision::TerrainSnapshot {
                            base: Arc::clone(base),
                            overlay: terrain.clone(),
                        });
                if let Some(game) = &game {
                    let mut g = game.lock().unwrap();
                    g.terrain = geometry.clone();
                    if let Some(owner) = &mut g.blocks {
                        owner.handle = membership.handle;
                        owner.terrain = membership.terrain.clone();
                    }
                }
                (replica_lease, inbox) = hub
                    .join_game(
                        entity,
                        target.id,
                        Arc::clone(&profile),
                        player.replica(),
                        game.clone(),
                    )
                    .await?;
                wake = Arc::clone(&inbox.wake);
                publish_dirty = false;
                viewer_dirty = false;
                deadline(5, async {
                    connection
                        .outbound
                        .send(&play::center(
                            player.view.center().x,
                            player.view.center().z,
                        ))
                        .await?;
                    connection
                        .outbound
                        .send(&play::default_spawn(config.spawn.map(|v| v as i32)))
                        .await?;
                    connection.outbound.send(&play::start_loading()).await?;
                    connection
                        .outbound
                        .send(&play::position(teleport_id, config.spawn, [0.; 2]))
                        .await
                })
                .await?;
                if let Some(game) = &game {
                    let (contents, health, attributes) = {
                        let g = game.lock().unwrap();
                        (
                            g.inventory.packet(),
                            g.health,
                            wire::pvp::attributes(entity, g.inventory.main()),
                        )
                    };
                    deadline(5, connection.outbound.send(&contents)).await?;
                    deadline(5, connection.outbound.send(&attributes)).await?;
                    deadline(5, connection.outbound.send(&wire::pvp::health(health))).await?;
                }
                respawning = false;
            }
            if !terrain.same_snapshot(&membership.terrain.borrow()) {
                let latest = membership.terrain.borrow_and_update().clone();
                deadline(5, async {
                    for (pos, _) in latest.edits().chain(
                        terrain
                            .edits()
                            .filter(|(pos, _)| latest.override_at(*pos).is_none()),
                    ) {
                        let chunk = ChunkPos {
                            x: pos[0].div_euclid(16),
                            z: pos[2].div_euclid(16),
                        };
                        if player.view.contains(chunk)
                            && terrain.override_at(pos) != latest.override_at(pos)
                        {
                            let material = latest
                                .override_at(pos)
                                .unwrap_or_else(|| self.base_block(pos));
                            connection
                                .outbound
                                .send(&play::block_correction(pos, material_state(material)))
                                .await?;
                        }
                    }
                    for pos in player.view.positions() {
                        let mut sections = Vec::new();
                        for sy in -4..20 {
                            let key = (pos, sy);
                            if terrain.sky_section(key) != latest.sky_section(key) {
                                let data = latest
                                    .sky_section(key)
                                    .map(Arc::<[u8]>::from)
                                    .unwrap_or_else(|| self.base_sky(pos, sy));
                                sections.push((sy, data));
                            }
                        }
                        if !sections.is_empty() {
                            connection
                                .outbound
                                .send(&smolmc_chunks::light_update(
                                    pos,
                                    sections.iter().map(|(sy, d)| (*sy, d.as_ref())),
                                ))
                                .await?;
                        }
                    }
                    Ok(())
                })
                .await?;
                terrain = latest;
                if let Some(g) = &mut geometry {
                    g.overlay = terrain.clone();
                }
                if let Some(game) = &game {
                    game.lock().unwrap().terrain = geometry.clone();
                }
            }
            if publish_dirty && now >= publish_due {
                replica_lease.update(player.replica())?;
                publish_dirty = false;
                publish_due = now + Duration::from_millis(50);
            }
            if viewer_dirty && now >= viewer_due {
                deadline(5, peer_view.flush(&mut connection.outbound, &inbox)).await?;
                viewer_dirty = false;
                viewer_due = Instant::now() + Duration::from_millis(50);
            }
            if player.keepalive_due(now) {
                let nonce = player.issue_keepalive(now);
                deadline(5, connection.outbound.send(&play::keep_alive(nonce))).await?;
            }
            // Control changes and bounded view reconciliation precede chunk sends.
            // No lifecycle lock or mutable world borrow covers socket I/O.
            if player.view.next_unload().is_some() {
                deadline(5, async {
                    while let Some(pos) = player.view.next_unload() {
                        connection
                            .outbound
                            .send(&play::unload(pos.x, pos.z))
                            .await?;
                        player.view.remove(pos);
                        self.metrics.chunks_unloaded.fetch_add(1, Relaxed);
                        publish_dirty = true;
                    }
                    connection
                        .outbound
                        .send(&play::center(
                            player.view.center().x,
                            player.view.center().z,
                        ))
                        .await
                })
                .await?;
            }
            if player.batch_ready(Instant::now()) {
                let mut sent = 0;
                deadline(5, async {
                    connection.outbound.send(&play::batch_start()).await?;
                    for _ in 0..player.batch_size {
                        let Some(pos) = player.view.next_missing() else {
                            break;
                        };
                        if let Some(packet) =
                            self.imported_chunks.as_ref().and_then(|c| c.packet(pos))
                        {
                            connection.outbound.send(packet).await?;
                        } else {
                            if let Some(chunks) = &self.imported_chunks {
                                chunks.encode(pos, &mut scratch)
                            } else {
                                self.stone.as_ref().unwrap().encode(pos, &mut scratch)
                            }
                            connection.outbound.send(&scratch).await?;
                        }
                        // Replay bounded instance edits after shared base bytes, before batch completion.
                        for (block, material) in terrain.chunk_edits(pos) {
                            connection
                                .outbound
                                .send(&play::block_correction(block, material_state(material)))
                                .await?;
                        }
                        let lights: Vec<_> = terrain
                            .light_sections()
                            .filter(|(key, _)| key.0 == pos)
                            .map(|(key, data)| (key.1, data))
                            .collect();
                        if !lights.is_empty() {
                            connection
                                .outbound
                                .send(&smolmc_chunks::light_update(pos, lights))
                                .await?;
                        }
                        if !player.view.insert(pos) {
                            return Err(invalid("chunk subscription capacity violated"));
                        }
                        sent += 1;
                        self.metrics.chunks_sent.fetch_add(1, Relaxed);
                        publish_dirty = true;
                        tokio::task::yield_now().await;
                    }
                    connection.outbound.send(&play::batch_finished(sent)).await
                })
                .await?;
                player.batch_sent(Instant::now());
                if self.imported_chunks.is_some() && player.view.next_missing().is_none() {
                    scratch = Vec::new();
                }
            }
            let mut due = player.next_deadline();
            if let Some(corpse_due) = corpse_due {
                due = due.min(corpse_due);
            }
            if publish_dirty {
                due = due.min(publish_due);
            }
            if viewer_dirty {
                due = due.min(viewer_due);
            }
            let packet = tokio::select! {
                _=tokio::time::sleep_until(due)=>continue,
                event=critical_message(&mut inbox.critical)=>{
                    match event? {
                        gameplay::Delivery::TerrainCommitted{world,packets,at}=>{if world==membership.handle {deadline(5,connection.outbound.send_batch(&packets)).await?;#[cfg(test)]crate::perf::BLOCK_WRITE.record(at.elapsed().as_micros()as u64);}#[cfg(not(test))]let _=at;},
                        gameplay::Delivery::Packets{entity:target,packets,at}=>{
                            if let Some(target)=target {
                                deadline(5,peer_view.flush(&mut connection.outbound,&inbox)).await?;
                                if !peer_view.contains(target){continue;}
                            }
                            deadline(5,connection.outbound.send_batch(&packets)).await?;
                            #[cfg(test)] crate::perf::CRITICAL_WRITE.record(at.elapsed().as_micros() as u64);
                            #[cfg(not(test))] let _=at;
                        },
                        gameplay::Delivery::Died=>{player.dead=true;player.corpse_visible=true;corpse_due=Some(Instant::now()+Duration::from_secs(1));replica_lease.update(player.replica())?;},
                        gameplay::Delivery::Respawn=>respawning=true,
                        gameplay::Delivery::SprintStopped=>{
                            player.sprinting=false;
                            deadline(5,connection.outbound.send(&presence::pose(entity,player.replica().state))).await?;
                            replica_lease.update(player.replica())?;
                        },
                    }
                    continue;
                },
                names=command_change(&mut membership.commands)=>{
                    deadline(5,connection.outbound.send(&play::commands(&names?))).await?;continue;
                },
                message=control_message(&mut membership.messages)=>{
                    deadline(5,connection.outbound.send(&presence::system(&format!("[System] {}",message?)).map_err(malformed)?)).await?;continue;
                },
                changed=membership.target.changed()=>{changed.map_err(|_| invalid("world control closed"))?;continue;},
                changed=membership.terrain.changed()=>{changed.map_err(|_| invalid("world terrain closed"))?;continue;},
                _=wake.notified(),if !viewer_dirty=>{viewer_dirty=true;continue;},
                message=inbox.chat.recv()=>{
                    let packet=message.ok_or_else(||io::Error::other("replication directory closed"))?;
                    deadline(5,connection.outbound.send(&packet)).await?;continue;
                },
                packet=connection.inbound.read_packet()=>packet?,
            };
            let now = Instant::now();
            if now.duration_since(window) >= Duration::from_secs(1) {
                window = now;
                packets = 0;
            }
            packets += 1;
            if packets > 256 {
                return Err(invalid("Play packet rate exceeded"));
            }
            self.metrics.play_packets_observed.fetch_add(1, Relaxed);
            if arena && let Some(action) = wire::pvp::decode(&packet).map_err(malformed)? {
                replica_lease.action(action, player.replica(), damage_type)?;
                continue;
            }
            if arena
                && packet.first() == Some(&0x0d)
                && let Some(game) = &game
            {
                game.lock().unwrap().finish_tick();
            }
            let event = play::decode(&packet).map_err(|error| {
                if context.trace {
                    let mut r = Reader::new(&packet);
                    eprintln!(
                        "connection={} rejected_play_packet_id={:?} bytes={}",
                        context.id,
                        r.varint().ok(),
                        packet.len()
                    );
                }
                malformed(error)
            })?;
            match event {
                play::Event::Command(command) => {
                    player.admit_chat(now)?;
                    if self.worlds.invoke_command(Some(entity), &command).is_err() {
                        deadline(
                            5,
                            connection.outbound.send(
                                &presence::system("[System] Command unavailable.")
                                    .map_err(malformed)?,
                            ),
                        )
                        .await?;
                    }
                }
                play::Event::Teleport {
                    id,
                    position,
                    rotation,
                } => {
                    player.teleport_ack(id, position, rotation)?;
                    publish_dirty = true;
                    self.metrics.teleports_acked.fetch_add(1, Relaxed);
                    if context.trace {
                        eprintln!("connection={} teleport_confirmed=true", context.id);
                    }
                }
                play::Event::KeepAlive(nonce) => {
                    player.keepalive_ack(nonce, now)?;
                    self.metrics.keepalives_acked.fetch_add(1, Relaxed);
                }
                play::Event::Batch(desired) => {
                    player.batch_ack(desired, now)?;
                    self.metrics.chunk_batches_acked.fetch_add(1, Relaxed);
                }
                play::Event::Move(mut movement) => {
                    #[cfg(test)]
                    let collision_start = Instant::now();
                    if let Some(current) = &geometry
                        && player.pending_teleport.is_none()
                        && movement
                            .position
                            .is_some_and(|p| p[1] < -60. && p.iter().all(|n| n.is_finite()))
                    {
                        config.spawn = current
                            .safe_spawn(self.gameplay_config().spawn)
                            .ok_or_else(|| invalid("no safe terrain void recovery point"))?;
                        teleport_id = teleport_id
                            .checked_add(1)
                            .ok_or_else(|| invalid("teleport IDs exhausted"))?;
                        player.switch_world(now, teleport_id);
                        player.spawn_at(config.spawn);
                        if let Some(game) = &game {
                            game.lock().unwrap().reposition(now, config.spawn);
                        }
                        deadline(
                            5,
                            connection.outbound.send(&play::position(
                                teleport_id,
                                config.spawn,
                                player.rotation,
                            )),
                        )
                        .await?;
                        replica_lease.update(player.replica())?;
                        continue;
                    }
                    let mut corrected = false;
                    if player.pending_teleport.is_none()
                        && let Some(geometry) = &geometry
                        && let Some(wanted) = movement.position
                    {
                        let delta = std::array::from_fn(|i| wanted[i] - player.position[i]);
                        let resolved = smolmc_world::collision::resolve(
                            player.position,
                            delta,
                            player.crouching,
                            player.on_ground,
                            |p| geometry.block(p),
                        )
                        .ok_or_else(|| invalid("collision work/coordinate budget"))?;
                        let position =
                            std::array::from_fn(|i| player.position[i] + resolved.delta[i]);
                        let inside = position[0] - 0.3 >= geometry.base.bounds.min[0] as f64
                            && position[0] + 0.3 <= geometry.base.bounds.max[0] as f64
                            && position[2] - 0.3 >= geometry.base.bounds.min[2] as f64
                            && position[2] + 0.3 <= geometry.base.bounds.max[2] as f64;
                        movement.position = Some(if !inside { player.position } else { position });
                        corrected = !inside
                            || (0..3)
                                .any(|i| (movement.position.unwrap()[i] - wanted[i]).abs() > 0.001);
                    }
                    if player.movement(movement, now)? {
                        self.metrics.chunk_boundaries.fetch_add(1, Relaxed);
                        if context.trace {
                            eprintln!(
                                "connection={} chunk_center={},{}",
                                context.id,
                                player.view.center().x,
                                player.view.center().z
                            );
                        }
                        // Send center even when no old chunks were sent (early movement).
                        if player.view.next_unload().is_none() {
                            deadline(
                                5,
                                connection.outbound.send(&play::center(
                                    player.view.center().x,
                                    player.view.center().z,
                                )),
                            )
                            .await?;
                        }
                    }
                    player.on_ground = geometry.as_ref().map_or(player.position[1] <= 64.01, |t| {
                        t.grounded(player.position, player.crouching)
                    });
                    if corrected {
                        teleport_id = teleport_id
                            .checked_add(1)
                            .ok_or_else(|| invalid("teleport IDs exhausted"))?;
                        player.pending_teleport = Some((teleport_id, now + player::ACK_TIMEOUT));
                        deadline(
                            5,
                            connection.outbound.send(&play::position(
                                teleport_id,
                                player.position,
                                player.rotation,
                            )),
                        )
                        .await?;
                    }
                    if let Some(game) = &game {
                        game.lock().unwrap().pose(player.replica().state, now);
                        replica_lease.pose(player.replica());
                    }
                    #[cfg(test)]
                    if geometry.is_some() {
                        crate::perf::COLLISION.record(collision_start.elapsed().as_micros() as u64);
                    }
                    self.metrics.movements.fetch_add(1, Relaxed);
                    publish_dirty = true;
                }
                play::Event::Flying(flying) => {
                    if arena {
                        if flying {
                            return Err(invalid("flight disabled in PvP"));
                        }
                        continue;
                    }
                    if player.flying != flying {
                        self.metrics.flight_changes.fetch_add(1, Relaxed);
                    }
                    player.flying = flying;
                }
                play::Event::PlayerCommand {
                    entity: claimed,
                    action,
                } => {
                    if claimed != entity {
                        return Err(invalid("client referenced another entity"));
                    }
                    if action == 1 || action == 2 {
                        player.sprinting = action == 1;
                        publish_dirty = true;
                    }
                }
                play::Event::Input(flags) => {
                    let crouching = flags & 32 != 0;
                    // The sprint key is input intent. Vanilla changes sprint state
                    // only through PlayerCommand, including after attack cancellation.
                    let sprinting = if arena {
                        player.sprinting
                    } else {
                        flags & 64 != 0
                    };
                    if player.crouching != crouching || player.sprinting != sprinting {
                        player.crouching = crouching;
                        player.sprinting = sprinting;
                        publish_dirty = true;
                    }
                }
                play::Event::Chat(text) => {
                    if context.trace
                        && text.eq_ignore_ascii_case("THAT RIGHT THERE")
                        && let Some(game) = &game
                    {
                        let marker_tick = game.lock().unwrap().tick(now);
                        eprintln!("pvp_marker connection={} tick={}", context.id, marker_tick);
                    }
                    player.admit_chat(now)?;
                    replica_lease.chat(text)?;
                }
                play::Event::ReadOnlyAction { pos, sequence } => {
                    let target = ChunkPos {
                        x: pos[0].div_euclid(16),
                        z: pos[2].div_euclid(16),
                    };
                    if !player.view.contains(target)
                        || pos
                            .iter()
                            .zip(player.position)
                            .map(|(a, b)| (f64::from(*a) - b).powi(2))
                            .sum::<f64>()
                            > 64.0
                    {
                        return Err(invalid("block action outside subscription or reach"));
                    }
                    let state = material_state(
                        terrain
                            .override_at(pos)
                            .unwrap_or_else(|| self.base_block(pos)),
                    );
                    deadline(5, async {
                        connection.outbound.send(&play::block_ack(sequence)).await?;
                        connection
                            .outbound
                            .send(&play::block_correction(pos, state))
                            .await
                    })
                    .await?;
                }
                play::Event::Ignored => {}
            }
        }
    }
}

fn material_state(material: Material) -> i32 {
    material.state()
}

async fn critical_message(
    receiver: &mut Option<tokio::sync::mpsc::Receiver<gameplay::Delivery>>,
) -> io::Result<gameplay::Delivery> {
    match receiver {
        Some(rx) => rx
            .recv()
            .await
            .ok_or_else(|| invalid("gameplay directory closed")),
        None => std::future::pending().await,
    }
}
// Disabled bridge branches never wake or allocate a channel/cache.
async fn command_change(
    receiver: &mut Option<tokio::sync::watch::Receiver<Arc<[String]>>>,
) -> io::Result<Arc<[String]>> {
    match receiver {
        Some(rx) => {
            rx.changed()
                .await
                .map_err(|_| invalid("command control closed"))?;
            Ok(rx.borrow_and_update().clone())
        }
        None => std::future::pending().await,
    }
}
async fn control_message(
    receiver: &mut Option<tokio::sync::mpsc::Receiver<String>>,
) -> io::Result<String> {
    match receiver {
        Some(rx) => rx
            .recv()
            .await
            .ok_or_else(|| invalid("message control closed")),
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry_is_shared_names_only_and_dimension_id_is_valid() {
        let registry = RegistrySet::load().unwrap();
        assert_eq!(registry.packets.len(), 32);
        assert!(registry.encoded_bytes < 64 * 1024);
        assert!(registry.dimension_id >= 0);
    }
    #[test]
    fn duplicate_identity_and_capacity_release_are_atomic() {
        let metrics = Arc::new(Metrics::default());
        let registry = Arc::new(SessionRegistry {
            ids: Mutex::new(HashSet::new()),
            capacity: Arc::new(Semaphore::new(2)),
            metrics: Arc::clone(&metrics),
        });
        let a = registry.claim([1; 16]).unwrap();
        assert!(registry.claim([1; 16]).is_err());
        let b = registry.claim([2; 16]).unwrap();
        assert!(registry.claim([3; 16]).is_err());
        drop(a);
        drop(b);
        assert_eq!(metrics.snapshot().active_sessions, 0);
        assert_eq!(registry.capacity.available_permits(), 2);
        assert!(registry.ids.lock().unwrap().is_empty());
        assert!(registry.claim([1; 16]).is_ok());
    }

    async fn connection_pair() -> (Connection, Connection) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = tokio::net::TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (server, _) = listener.accept().await.unwrap();
        let budget = smolmc_network::transport::ByteBudget::new(1024 * 1024);
        let mut client = Connection::new(client, budget.clone());
        let mut server = Connection::new(server, budget);
        for connection in [&mut client, &mut server] {
            connection.inbound.set_limit(CONTROL_LIMIT).unwrap();
            connection.enable_compression(256).unwrap();
        }
        (server, client)
    }

    #[tokio::test]
    async fn configuration_negotiation_ack_and_named_tags() {
        let service = Arc::new(Service::new().unwrap());
        let (mut server, mut client) = connection_pair().await;
        let task_service = Arc::clone(&service);
        let task = tokio::spawn(async move { task_service.configure(&mut server).await });
        assert_eq!(client.inbound.read_packet().await.unwrap()[0], 13);
        assert_eq!(client.inbound.read_packet().await.unwrap()[0], 15);
        let mut answer = login::select_known_packs();
        answer[0] = 7;
        client.outbound.send(&answer).await.unwrap();
        let mut registries = 0;
        let mut tags_bound = false;
        loop {
            let packet = client.inbound.read_packet().await.unwrap();
            let mut reader = Reader::new(&packet);
            match reader.varint().unwrap() {
                7 => {
                    reader.string(32767).unwrap();
                    let count = reader.varint().unwrap();
                    for _ in 0..count {
                        reader.string(32767).unwrap();
                        assert!(!reader.boolean().unwrap());
                    }
                    reader.finish().unwrap();
                    registries += 1;
                }
                14 => {
                    let count = reader.varint().unwrap();
                    assert!(count >= 10);
                    for _ in 0..count {
                        let registry = reader.string(32767).unwrap().to_owned();
                        for _ in 0..reader.varint().unwrap() {
                            let tag = reader.string(32767).unwrap();
                            if registry == "minecraft:block"
                                && tag == "minecraft:infiniburn_overworld"
                            {
                                tags_bound = true;
                            }
                            assert_eq!(reader.varint().unwrap(), 0);
                        }
                    }
                    reader.finish().unwrap();
                }
                3 => {
                    reader.finish().unwrap();
                    break;
                }
                id => panic!("unexpected server configuration packet {id}"),
            }
        }
        assert_eq!(registries, 32);
        assert!(tags_bound);
        client.outbound.send(&[3]).await.unwrap();
        task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn declining_core_pack_is_explicitly_unsupported() {
        let service = Arc::new(Service::new().unwrap());
        let (mut server, mut client) = connection_pair().await;
        let task = tokio::spawn(async move { service.configure(&mut server).await });
        client.inbound.read_packet().await.unwrap();
        client.inbound.read_packet().await.unwrap();
        client.outbound.send(&[7, 0]).await.unwrap();
        assert_eq!(
            task.await.unwrap().unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
    }

    #[tokio::test]
    async fn encrypted_play_streams_bounded_batches_and_only_entering_edges() {
        use smolmc_network::{
            Config, Metrics as NetworkMetrics, serve_with, transport::ByteBudget,
        };
        use tokio::{
            net::{TcpListener, TcpStream},
            sync::oneshot,
        };
        let service = Arc::new(Service::new().unwrap());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, receive) = oneshot::channel();
        let network = Arc::new(NetworkMetrics::default());
        let network_task = Arc::clone(&network);
        let sessions = Arc::clone(&service);
        let key = [7; 16];
        let task = tokio::spawn(serve_with(
            listener,
            Config::default(),
            network_task,
            async {
                let _ = receive.await;
            },
            move |mut connection, context| {
                let sessions = Arc::clone(&sessions);
                async move {
                    // Unit harness enters the private Play handler directly. There is
                    // no auth bypass in the deployed service or public socket path.
                    connection.enable_encryption(&key)?;
                    connection.enable_compression(256)?;
                    connection.inbound.set_limit(CONTROL_LIMIT)?;
                    sessions
                        .play(
                            &mut connection,
                            &context,
                            1,
                            Arc::new(login::Profile {
                                uuid: [1; 16],
                                name: "Probe".into(),
                                properties: Vec::new(),
                            }),
                            sessions.seed,
                        )
                        .await
                }
            },
        ));
        let budget = ByteBudget::new(1024 * 1024);
        let mut client =
            Connection::new(TcpStream::connect(address).await.unwrap(), budget.clone());
        client.inbound.set_limit(CONTROL_LIMIT).unwrap();
        client.inbound.set_work_limit(8 * 1024 * 1024).unwrap();
        client.enable_encryption(&key).unwrap();
        client.enable_compression(256).unwrap();
        for _ in 0..8 {
            let packet = client.inbound.read_packet().await.unwrap();
            if packet[0] == 0x49 {
                let mut ack = vec![0, 1];
                for value in SPAWN {
                    ack.extend_from_slice(&value.to_be_bytes());
                }
                ack.extend_from_slice(&[0; 8]);
                client.outbound.send(&ack).await.unwrap();
            }
        }
        async fn receive_view(client: &mut Connection, expected: usize, unloads: usize) {
            let mut count = 0;
            let mut unloaded = 0;
            let mut in_batch = false;
            let mut batch_count = 0;
            while count < expected {
                let packet = deadline(2, client.inbound.read_packet()).await.unwrap();
                match packet[0] {
                    0x60 => {}
                    0x26 => {
                        unloaded += 1;
                        assert_eq!(packet.len(), 9);
                    }
                    0x0c => {
                        assert!(!in_batch);
                        in_batch = true;
                        batch_count = 0;
                    }
                    0x2e => {
                        assert!(in_batch);
                        assert!(packet.len() < 40 * 1024);
                        count += 1;
                        batch_count += 1;
                    }
                    0x0b => {
                        assert!(in_batch);
                        assert!((1..=4).contains(&batch_count));
                        let mut r = Reader::new(&packet);
                        assert_eq!(r.varint().unwrap(), 0x0b);
                        assert_eq!(r.varint().unwrap(), batch_count);
                        r.finish().unwrap();
                        // Server must await ACK; cancelled reads preserve frame state.
                        assert!(
                            timeout(Duration::from_millis(5), client.inbound.read_packet())
                                .await
                                .is_err()
                        );
                        let mut ack = vec![0x0b];
                        ack.extend_from_slice(&4f32.to_be_bytes());
                        client.outbound.send(&ack).await.unwrap();
                        in_batch = false;
                    }
                    id => panic!("unexpected Play packet {id:#x}"),
                }
            }
            if in_batch {
                let packet = client.inbound.read_packet().await.unwrap();
                assert_eq!(packet[0], 0x0b);
                let mut ack = vec![0x0b];
                ack.extend_from_slice(&4f32.to_be_bytes());
                client.outbound.send(&ack).await.unwrap();
            }
            assert_eq!(unloaded, unloads);
        }
        receive_view(&mut client, 25, 0).await;
        let mut movement = vec![0x1e];
        for value in [24f64, 65.0, 8.0] {
            movement.extend_from_slice(&value.to_be_bytes());
        }
        movement.push(0);
        client.outbound.send(&movement).await.unwrap();
        receive_view(&mut client, 5, 5).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        movement[1..9].copy_from_slice(&(-0.1f64).to_be_bytes());
        client.outbound.send(&movement).await.unwrap();
        // From center 1 to center -1, ten columns depart/enter; no full resend.
        receive_view(&mut client, 10, 10).await;
        // An unchanged view must idle without chunk retransmission; the next
        // outbound packet is a real keepalive, and the response restores health.
        let keepalive = deadline(13, client.inbound.read_packet()).await.unwrap();
        let mut r = Reader::new(&keepalive);
        assert_eq!(r.varint().unwrap(), 0x2d);
        let nonce = r.take(8).unwrap();
        let mut response = vec![0x1c];
        response.extend_from_slice(nonce);
        r.finish().unwrap();
        drop(keepalive);
        client.outbound.send(&response).await.unwrap();
        timeout(Duration::from_secs(1), async {
            while service.metrics.keepalives_acked.load(Relaxed) != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        stop.send(()).unwrap();
        task.await.unwrap().unwrap();
        drop(client);
        service.shutdown().await.unwrap();
        let snapshot = service.metrics.snapshot();
        assert_eq!(snapshot.chunks_sent, 40);
        assert_eq!(snapshot.chunks_unloaded, 15);
        assert_eq!(snapshot.chunk_boundaries, 2);
        assert_eq!(snapshot.teleports_acked, 1);
        assert_eq!(snapshot.keepalives_acked, 1);
        assert_eq!(network.snapshot().frame_bytes_retained, 0);
        assert_eq!(budget.available(), 1024 * 1024);
        assert_eq!(snapshot.authenticated, 0); // tests are not claimed as real logins
    }
}
