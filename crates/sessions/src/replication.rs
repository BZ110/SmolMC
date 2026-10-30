//! M1D membership + M2A explicit worlds: one event-driven directory task that hosts
//! many world instances. Each instance owns its own membership, visibility and chat
//! fanout indices; because every member in one `InstanceDir` belongs to a single
//! world, no cross-instance replication is possible even under concurrency. Session
//! tasks retain authoritative controls and all socket I/O; immutable watch snapshots
//! coalesce state/rosters; only chat has a bounded message queue. No simulation/world
//! worker. Keys never recycle while outstanding, and a disabled instance's indices are
//! dropped wholesale rather than reused by another world.
use crate::{MAX_PLAYERS, Metrics, invalid};
use smolmc_protocol_777::{
    login::Profile,
    presence::{self, EntitySnapshot},
};
use smolmc_world::{ChunkPos, ChunkView, WorldId};
use std::{
    collections::{HashMap, HashSet},
    io,
    sync::{
        Arc, Mutex as SlotMutex,
        atomic::{AtomicU64, Ordering::Relaxed},
    },
};
use tokio::{
    sync::{Mutex, Notify, mpsc, oneshot, watch},
    task::JoinHandle,
    time::Instant,
};

const COMMAND_CAPACITY: usize = MAX_PLAYERS * 4;
const CHAT_CAPACITY: usize = 32;

/// Per-lease coalescing slot shared between the lease and its member. Writes are frequent
/// (every movement); the hub resets `queued` after it drains one, so a burst of updates
/// collapses into a single command carrying the latest snapshot.
struct Pending {
    latest: Replica,
    queued: bool,
}
fn pending(replica: Replica) -> Arc<SlotMutex<Pending>> {
    Arc::new(SlotMutex::new(Pending {
        latest: replica,
        queued: false,
    }))
}

#[derive(Clone, Copy)]
pub struct Replica {
    pub state: EntitySnapshot,
    pub view: ChunkView,
    pub ready: bool,
    pub version: u64,
    #[cfg(test)]
    pub at: Instant,
}
pub struct Peer {
    pub entity: i32,
    pub uuid: [u8; 16],
    pub info: Arc<[u8]>,
    pub state: watch::Receiver<Replica>,
}
pub struct Inbox {
    pub roster: watch::Receiver<Arc<Vec<Arc<Peer>>>>,
    pub chat: mpsc::Receiver<Arc<[u8]>>,
    pub wake: Arc<Notify>,
    pub closed: watch::Receiver<bool>,
}
enum Command {
    Join {
        entity: i32,
        instance: WorldId,
        token: u64,
        profile: Arc<Profile>,
        pending: Arc<SlotMutex<Pending>>,
        reply: oneshot::Sender<io::Result<Inbox>>,
    },
    Update {
        entity: i32,
        instance: WorldId,
        token: u64,
    },
    Chat {
        entity: i32,
        instance: WorldId,
        token: u64,
        text: String,
    },
    Leave(WorldId, i32, u64),
    Stop,
}
pub struct Hub {
    sender: mpsc::Sender<Command>,
    serial: AtomicU64,
    task: Mutex<Option<JoinHandle<()>>>, // shutdown only, never a hot-path lock
}
pub struct Lease {
    entity: i32,
    instance: WorldId,
    token: u64,
    sender: mpsc::Sender<Command>,
    leave: Option<mpsc::OwnedPermit<Command>>,
    pending: Arc<SlotMutex<Pending>>,
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(permit) = self.leave.take() {
            permit.send(Command::Leave(self.instance, self.entity, self.token));
        }
    }
}
impl Lease {
    pub fn update(&self, replica: Replica) -> io::Result<()> {
        let mut slot = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        slot.latest = replica;
        if slot.queued {
            // A snapshot is already queued for the hub; it carries this latest value, so no
            // second command is needed. Coalescing keeps a busy member from flooding ingress.
            return Ok(());
        }
        self.sender
            .try_send(Command::Update {
                entity: self.entity,
                instance: self.instance,
                token: self.token,
            })
            .map_err(|_| {
                io::Error::new(io::ErrorKind::WouldBlock, "replication ingress full/closed")
            })?;
        slot.queued = true;
        Ok(())
    }
    pub fn chat(&self, text: String) -> io::Result<()> {
        self.sender
            .try_send(Command::Chat {
                entity: self.entity,
                instance: self.instance,
                token: self.token,
                text,
            })
            .map_err(|_| {
                io::Error::new(io::ErrorKind::WouldBlock, "replication ingress full/closed")
            })?;
        Ok(())
    }
}
impl Hub {
    pub fn start(metrics: Arc<Metrics>) -> Self {
        let (sender, receiver) = mpsc::channel(COMMAND_CAPACITY);
        Self {
            sender,
            serial: AtomicU64::new(1),
            task: Mutex::new(Some(tokio::spawn(
                Directory::new(Arc::clone(&metrics)).run(receiver),
            ))),
        }
    }
    pub async fn join(
        &self,
        entity: i32,
        instance: WorldId,
        profile: Arc<Profile>,
        replica: Replica,
    ) -> io::Result<(Lease, Inbox)> {
        let leave = self
            .sender
            .clone()
            .try_reserve_owned()
            .map_err(|_| io::Error::new(io::ErrorKind::WouldBlock, "replication capacity"))?;
        let token = self
            .serial
            .try_update(Relaxed, Relaxed, |n| n.checked_add(1))
            .map_err(|_| io::Error::other("replication generation exhausted"))?;
        let pending = pending(replica);
        let lease = Lease {
            entity,
            instance,
            token,
            sender: self.sender.clone(),
            leave: Some(leave),
            pending: Arc::clone(&pending),
        };
        let (reply, answer) = oneshot::channel();
        self.sender
            .try_send(Command::Join {
                entity,
                instance,
                token,
                profile,
                pending,
                reply,
            })
            .map_err(|_| io::Error::new(io::ErrorKind::WouldBlock, "replication ingress"))?;
        let inbox = answer
            .await
            .map_err(|_| io::Error::other("replication directory stopped"))??;
        Ok((lease, inbox))
    }
    pub async fn shutdown(&self) -> io::Result<()> {
        let mut task = self.task.lock().await;
        if let Some(join) = task.take() {
            let _ = self.sender.send(Command::Stop).await;
            join.await
                .map_err(|_| io::Error::other("replication directory task failed"))?;
        }
        Ok(())
    }
}

/// One world instance's membership and visibility indices. Every member here belongs to
/// a single instance, so all keys are `ChunkPos` and carry no instance id: an index can
/// never see a peer in another world. The `metrics` Arc is shared with the hub (a clone
/// of one allocation), so counters stay global while index logic stays per-instance.
struct InstanceDir {
    members: HashMap<i32, Member>,
    occupants: HashMap<ChunkPos, HashSet<i32>>,
    subscribers: HashMap<ChunkPos, HashSet<i32>>,
    fanout_credit: f64,
    fanout_at: Instant,
    metrics: Arc<Metrics>,
}

/// Thin per-instance-agnostic orchestrator: a single directory task hosts every world.
/// The `WorldId` in each command selects which `InstanceDir` handles it; a fully empty
/// instance is dropped wholesale so its indices cannot be reused by another world.
struct Directory {
    fanout_credit: f64,
    fanout_at: Instant,
    instances: HashMap<WorldId, InstanceDir>,
    metrics: Arc<Metrics>,
}
impl Directory {
    fn new(metrics: Arc<Metrics>) -> Self {
        Self {
            fanout_credit: 8192.,
            fanout_at: Instant::now(),
            instances: HashMap::new(),
            metrics,
        }
    }
    async fn run(mut self, mut receiver: mpsc::Receiver<Command>) {
        'commands: while let Some(command) = receiver.recv().await {
            if !self.command(command) {
                break;
            }
            let start = Instant::now();
            for _ in 1..32 {
                if start.elapsed() >= std::time::Duration::from_millis(1) {
                    break;
                }
                let Ok(command) = receiver.try_recv() else {
                    break;
                };
                if !self.command(command) {
                    break 'commands;
                }
            }
            tokio::task::yield_now().await;
        }
        self.metrics.active_replicas.store(0, Relaxed);
        self.metrics.visible_edges.store(0, Relaxed);
    }
    fn command(&mut self, command: Command) -> bool {
        let touched = match &command {
            Command::Join { instance, .. }
            | Command::Update { instance, .. }
            | Command::Chat { instance, .. } => Some(*instance),
            Command::Leave(instance, ..) => Some(*instance),
            Command::Stop => None,
        };
        match command {
            Command::Join {
                entity,
                instance,
                token,
                profile,
                pending,
                reply,
            } => {
                if self.metrics.active_replicas.load(Relaxed) >= MAX_PLAYERS {
                    let _ = reply.send(Err(invalid("global replica capacity")));
                    return true;
                }
                let index = match self.instances.entry(instance) {
                    std::collections::hash_map::Entry::Occupied(o) => o.into_mut(),
                    std::collections::hash_map::Entry::Vacant(v) => {
                        v.insert(InstanceDir::new(Arc::clone(&self.metrics)))
                    }
                };
                let _ = reply.send(index.join_pending(entity, token, profile, pending));
            }
            Command::Update {
                entity,
                instance,
                token,
            } => {
                if let Some(index) = self.instances.get_mut(&instance) {
                    index.update(entity, token);
                }
            }
            Command::Chat {
                entity,
                instance,
                token,
                text,
            } => {
                if let Some(index) = self.instances.get_mut(&instance)
                    && index.members.get(&entity).is_some_and(|m| m.token == token)
                {
                    let now = Instant::now();
                    self.fanout_credit = (self.fanout_credit
                        + now.duration_since(self.fanout_at).as_secs_f64() * 8192.)
                        .min(8192.);
                    self.fanout_at = now;
                    if self.fanout_credit >= index.members.len() as f64 {
                        self.fanout_credit -= index.members.len() as f64;
                        index.chat(entity, text);
                    }
                }
            }
            Command::Leave(instance, entity, token) => {
                if let Some(index) = self.instances.get_mut(&instance) {
                    index.leave(entity, token);
                }
            }
            Command::Stop => return false,
        }
        // A just-left player can empty its instance; drop the whole directory so its
        // indices (never reused by another world) reclaim high-water capacity.
        if let Some(instance) = touched
            && self
                .instances
                .get(&instance)
                .is_some_and(|d| d.members.is_empty())
        {
            self.instances.remove(&instance);
        }
        if self.instances.is_empty() {
            self.instances = HashMap::new();
        }
        true
    }
}
struct Member {
    token: u64,
    pending: Arc<SlotMutex<Pending>>,
    profile: Arc<Profile>,
    peer: Arc<Peer>,
    state: watch::Sender<Replica>,
    roster: watch::Sender<Arc<Vec<Arc<Peer>>>>,
    chat: mpsc::Sender<Arc<[u8]>>,
    wake: Arc<Notify>,
    closed: watch::Sender<bool>,
    visible: HashSet<i32>,
    viewers: HashSet<i32>,
    chat_credit: f64,
    chat_at: Instant,
}

impl InstanceDir {
    fn new(metrics: Arc<Metrics>) -> Self {
        Self {
            members: HashMap::new(),
            occupants: HashMap::new(),
            subscribers: HashMap::new(),
            fanout_credit: 8192.,
            fanout_at: Instant::now(),
            metrics,
        }
    }
    fn join_pending(
        &mut self,
        entity: i32,
        token: u64,
        profile: Arc<Profile>,
        pending: Arc<SlotMutex<Pending>>,
    ) -> io::Result<Inbox> {
        if self.members.len() >= MAX_PLAYERS || self.members.contains_key(&entity) {
            return Err(invalid("duplicate/capacity replica"));
        }
        let initial = pending.lock().unwrap_or_else(|e| e.into_inner()).latest;
        let (state, state_rx) = watch::channel(initial);
        let peer = Arc::new(Peer {
            entity,
            uuid: profile.uuid,
            info: presence::player_info(&profile).into(),
            state: state_rx,
        });
        let (roster, roster_rx) = watch::channel(Arc::new(Vec::new()));
        let (chat, chat_rx) = mpsc::channel(CHAT_CAPACITY);
        let wake = Arc::new(Notify::new());
        let (closed, closed_rx) = watch::channel(false);
        self.members.insert(
            entity,
            Member {
                token,
                pending,
                profile,
                peer,
                state,
                roster,
                chat,
                wake: Arc::clone(&wake),
                closed,
                visible: HashSet::new(),
                viewers: HashSet::new(),
                chat_credit: 3.,
                chat_at: Instant::now(),
            },
        );
        self.metrics.active_replicas.fetch_add(1, Relaxed);
        self.index_add(entity, initial);
        self.reconcile(entity);
        self.reconcile_target(entity);
        Ok(Inbox {
            roster: roster_rx,
            chat: chat_rx,
            wake,
            closed: closed_rx,
        })
    }
    fn index_add(&mut self, id: i32, r: Replica) {
        if !r.ready {
            return;
        }
        let pos = ChunkPos::from_position(r.state.position[0], r.state.position[2])
            .expect("validated session coordinates");
        self.occupants.entry(pos).or_default().insert(id);
        for pos in r.view.positions() {
            self.subscribers.entry(pos).or_default().insert(id);
        }
    }
    fn index_remove(&mut self, id: i32, r: Replica) {
        if !r.ready {
            return;
        }
        let pos = ChunkPos::from_position(r.state.position[0], r.state.position[2])
            .expect("validated session coordinates");
        remove_index(&mut self.occupants, pos, id);
        for pos in r.view.positions() {
            remove_index(&mut self.subscribers, pos, id);
        }
    }
    fn update(&mut self, id: i32, token: u64) {
        #[cfg(test)]
        let _timer = crate::perf::Timer::new(&crate::perf::DIRECTORY);
        let Some(member) = self.members.get(&id) else {
            return;
        };
        if member.token != token {
            // Stale lease: this Update belongs to a previous generation that re-joined the same entity id.
            // It must not mutate the current member; by construction this InstanceDir holds exactly one world.
            return;
        }
        if *member.closed.borrow() {
            return;
        }
        // Reset the ingress coalescing flag so a later update can be sent once more, and read
        // the latest snapshot that collapsed during the coalescing window. The slot is shared
        // with the lease (a clone of member.pending), so this reset clears the flag future updates consult.
        let mut next = {
            let mut slot = member.pending.lock().unwrap_or_else(|e| e.into_inner());
            slot.queued = false;
            slot.latest
        };
        let old = *member.state.borrow();
        next.version = old.version.wrapping_add(1);
        let old_pos = ChunkPos::from_position(old.state.position[0], old.state.position[2]);
        let new_pos = ChunkPos::from_position(next.state.position[0], next.state.position[2]);
        let changed_view = old.ready != next.ready || old.view != next.view;
        let changed_chunk = old.ready != next.ready || old_pos != new_pos;
        // Remove old memberships before publishing the new snapshot.
        if changed_view || changed_chunk {
            self.index_remove(id, old);
        }
        self.members[&id].state.send_replace(next);
        if changed_view || changed_chunk {
            self.index_add(id, next);
        }
        if changed_view {
            self.reconcile(id);
        }
        if changed_chunk {
            let mut candidates = HashSet::new();
            for pos in [old_pos, new_pos].into_iter().flatten() {
                if let Some(ids) = self.subscribers.get(&pos) {
                    candidates.extend(ids.iter().copied());
                }
            }
            candidates.extend(self.members[&id].viewers.iter().copied());
            for viewer in candidates {
                self.reconcile_pair(viewer, id);
            }
        }
        for viewer in &self.members[&id].viewers {
            self.members[viewer].wake.notify_one();
        }
        self.metrics.replica_updates.fetch_add(1, Relaxed);
    }

    fn reconcile_target(&mut self, id: i32) {
        let r = *self.members[&id].state.borrow();
        if !r.ready {
            return;
        }
        let pos = ChunkPos::from_position(r.state.position[0], r.state.position[2]).unwrap();
        let candidates: Vec<_> = self
            .subscribers
            .get(&pos)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        for viewer in candidates {
            self.reconcile_pair(viewer, id);
        }
    }
    fn reconcile_pair(&mut self, viewer: i32, target: i32) {
        if viewer == target {
            return;
        }
        let Some(member) = self.members.get(&viewer) else {
            return;
        };
        let view = *member.state.borrow();
        let desired = self.members.get(&target).is_some_and(|m| {
            let target = *m.state.borrow();
            view.ready
                && target.ready
                && ChunkPos::from_position(target.state.position[0], target.state.position[2])
                    .is_some_and(|p| view.view.contains(p))
        });
        if member.visible.contains(&target) == desired {
            return;
        }
        if desired {
            self.members
                .get_mut(&viewer)
                .unwrap()
                .visible
                .insert(target);
            self.members
                .get_mut(&target)
                .unwrap()
                .viewers
                .insert(viewer);
            let edges = self.metrics.visible_edges.fetch_add(1, Relaxed) + 1;
            self.metrics.peak_visible_edges.fetch_max(edges, Relaxed);
        } else {
            self.members
                .get_mut(&viewer)
                .unwrap()
                .visible
                .remove(&target);
            if let Some(m) = self.members.get_mut(&target) {
                m.viewers.remove(&viewer);
            }
            self.metrics.visible_edges.fetch_sub(1, Relaxed);
        }
        self.publish_roster(viewer);
    }
    fn publish_roster(&self, id: i32) {
        let mut roster: Vec<_> = self.members[&id]
            .visible
            .iter()
            .map(|peer| Arc::clone(&self.members[peer].peer))
            .collect();
        roster.sort_unstable_by_key(|peer| peer.entity);
        let member = &self.members[&id];
        member.roster.send_replace(Arc::new(roster));
        member.wake.notify_one();
    }
    fn reconcile(&mut self, id: i32) {
        let Some(member) = self.members.get(&id) else {
            return;
        };
        let r = *member.state.borrow();
        let mut desired = HashSet::new();
        if r.ready {
            for pos in r.view.positions() {
                if let Some(occupants) = self.occupants.get(&pos) {
                    desired.extend(occupants.iter().filter(|&&peer| peer != id).copied());
                }
            }
        }
        if desired == member.visible {
            return;
        }
        let old = std::mem::replace(&mut self.members.get_mut(&id).unwrap().visible, desired);
        let desired = &self.members[&id].visible;
        let removed: Vec<_> = old.difference(desired).copied().collect();
        let added: Vec<_> = desired.difference(&old).copied().collect();
        for peer in removed {
            if let Some(m) = self.members.get_mut(&peer) {
                m.viewers.remove(&id);
            }
            self.metrics.visible_edges.fetch_sub(1, Relaxed);
        }
        for peer in added {
            self.members.get_mut(&peer).unwrap().viewers.insert(id);
            let edges = self.metrics.visible_edges.fetch_add(1, Relaxed) + 1;
            self.metrics.peak_visible_edges.fetch_max(edges, Relaxed);
        }
        self.publish_roster(id);
    }
    fn leave(&mut self, id: i32, token: u64) {
        let Some(member) = self.members.get(&id) else {
            return;
        };
        if member.token != token {
            // Stale lease: this Leave belongs to a previous generation that re-joined the same entity id.
            // It must not remove the current member; by construction this InstanceDir holds exactly one world.
            return;
        }
        let old = *member.state.borrow();
        self.index_remove(id, old);
        let member = self.members.remove(&id).unwrap();
        for peer in &member.visible {
            if let Some(m) = self.members.get_mut(peer) {
                m.viewers.remove(&id);
            }
        }
        self.metrics
            .visible_edges
            .fetch_sub(member.visible.len(), Relaxed);
        for viewer in member.viewers {
            self.reconcile_pair(viewer, id);
        }
        self.metrics.active_replicas.fetch_sub(1, Relaxed);
        if self.members.is_empty() {
            // Drop hash-table high-water capacity after the last active player.
            self.members = HashMap::new();
            self.occupants = HashMap::new();
            self.subscribers = HashMap::new();
        }
    }

    fn deliver(&self, id: i32, packet: Arc<[u8]>) {
        let member = &self.members[&id];
        if *member.closed.borrow() {
            return;
        }
        if member.chat.try_send(packet).is_err() {
            member.closed.send_replace(true);
            member.wake.notify_one();
            self.metrics.slow_viewers.fetch_add(1, Relaxed);
        } else {
            member.wake.notify_one();
            self.metrics.chat_deliveries.fetch_add(1, Relaxed);
        }
    }
    fn chat(&mut self, id: i32, text: String) {
        let now = Instant::now();
        let Some(member) = self.members.get_mut(&id) else {
            return;
        };
        if !member.state.borrow().ready || *member.closed.borrow() {
            return;
        }
        member.chat_credit =
            (member.chat_credit + now.duration_since(member.chat_at).as_secs_f64()).min(3.);
        member.chat_at = now;
        self.fanout_credit = (self.fanout_credit
            + now.duration_since(self.fanout_at).as_secs_f64() * 8192.)
            .min(8192.);
        self.fanout_at = now;
        if member.chat_credit < 1. || self.fanout_credit < self.members.len() as f64 {
            self.deliver(
                id,
                presence::system("[System] Chat rate/capacity limit; please wait.")
                    .unwrap()
                    .into(),
            );
            return;
        }
        let member = self.members.get_mut(&id).unwrap();
        member.chat_credit -= 1.;
        self.fanout_credit -= self.members.len() as f64;
        let packet: Arc<[u8]> = presence::system(&format!(
            "[System relay] <{}> {}",
            self.members[&id].profile.name, text
        ))
        .expect("bounded validated chat")
        .into();
        // Deliver only to ready peers that share this instance. Members live in exactly
        // one instance, so the fanout set is per-instance by construction.
        for &viewer in self.members.keys() {
            if self.members[&viewer].state.borrow().ready {
                self.deliver(viewer, Arc::clone(&packet));
            }
        }
        self.metrics.chat_messages.fetch_add(1, Relaxed);
    }
}
fn remove_index(index: &mut HashMap<ChunkPos, HashSet<i32>>, pos: ChunkPos, id: i32) {
    if let Some(ids) = index.get_mut(&pos) {
        ids.remove(&id);
        if ids.is_empty() {
            index.remove(&pos);
        }
    }
}

struct Sent {
    uuid: [u8; 16],
    state: EntitySnapshot,
    version: u64,
}
#[derive(Default)]
pub struct View {
    sent: HashMap<i32, Sent>,
    removals: Vec<i32>,
    last_roster: Option<Arc<Vec<Arc<Peer>>>>,
    packets: Vec<Vec<u8>>,
    packet_bytes: usize,
    #[cfg(test)]
    ages: Vec<Instant>,
    #[cfg(test)]
    sampled: usize,
}
impl View {
    async fn flush_packets(
        &mut self,
        output: &mut smolmc_network::transport::TcpOutbound,
    ) -> io::Result<()> {
        if !self.packets.is_empty() {
            output.send_batch(&self.packets).await?;
            self.packets.clear();
            self.packet_bytes = 0;
        }
        #[cfg(test)]
        for at in self.ages.drain(..) {
            crate::perf::AGE.record(at.elapsed().as_micros() as u64);
        }
        Ok(())
    }
    async fn packet(
        &mut self,
        output: &mut smolmc_network::transport::TcpOutbound,
        packet: Vec<u8>,
    ) -> io::Result<()> {
        use smolmc_network::transport::{BATCH_BYTES, BATCH_PACKETS};
        if self.packets.len() == BATCH_PACKETS || self.packet_bytes + packet.len() > BATCH_BYTES {
            self.flush_packets(output).await?;
        }
        if packet.len() > BATCH_BYTES {
            return output.send(&packet).await;
        }
        self.packet_bytes += packet.len();
        self.packets.push(packet);
        Ok(())
    }

    pub async fn clear(
        &mut self,
        output: &mut smolmc_network::transport::TcpOutbound,
    ) -> io::Result<()> {
        self.flush_packets(output).await?;
        self.last_roster = None;
        for (id, old) in self.sent.drain() {
            output.send(&presence::entity_remove(id)).await?;
            output.send(&presence::player_remove(&old.uuid)).await?;
        }
        Ok(())
    }
    /// Successful return means all staged frames were written. The session owner
    /// closes on error/cancellation, as with its existing five-second write deadline.
    pub async fn flush(
        &mut self,
        output: &mut smolmc_network::transport::TcpOutbound,
        inbox: &Inbox,
    ) -> io::Result<()> {
        #[cfg(test)]
        let _timer = crate::perf::Timer::new(&crate::perf::FLUSH);
        #[cfg(test)]
        let mut fanout = 0;
        #[cfg(test)]
        let mut positions = 0;
        let roster = Arc::clone(&inbox.roster.borrow());
        self.removals.clear();
        if self
            .last_roster
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, &roster))
        {
            self.removals.extend(
                self.sent
                    .keys()
                    .filter(|&&id| roster.binary_search_by_key(&id, |p| p.entity).is_err())
                    .copied(),
            );
            self.last_roster = Some(Arc::clone(&roster));
        }
        for id in self.removals.drain(..) {
            let old = self.sent.remove(&id).unwrap();
            output.send(&presence::entity_remove(id)).await?;
            output.send(&presence::player_remove(&old.uuid)).await?;
        }
        for (index, peer) in roster.iter().enumerate() {
            let r = *peer.state.borrow();
            let id = peer.entity;
            if let Some(sent) = self.sent.get(&id) {
                if sent.version == r.version {
                    continue;
                }
                let old = sent.state;
                if old.position != r.state.position
                    || old.rotation != r.state.rotation
                    || old.on_ground != r.state.on_ground
                {
                    self.packet(output, presence::position(id, r.state)).await?;
                    #[cfg(test)]
                    {
                        positions += 1;
                    }
                }
                if old.rotation[0] != r.state.rotation[0] {
                    self.packet(output, presence::head(id, r.state.rotation[0]))
                        .await?;
                }
                if old.crouching != r.state.crouching || old.sprinting != r.state.sprinting {
                    self.packet(output, presence::pose(id, r.state)).await?;
                }
                #[cfg(test)]
                {
                    fanout += 1;
                    self.sampled += 1;
                    if self.sampled.is_multiple_of(64) {
                        self.ages.push(r.at);
                    }
                }
                let sent = self.sent.get_mut(&id).unwrap();
                sent.state = r.state;
                sent.version = r.version;
            } else {
                self.flush_packets(output).await?;
                output.send(&peer.info).await?;
                output
                    .send(&presence::spawn(id, &peer.uuid, r.state))
                    .await?;
                output.send(&presence::pose(id, r.state)).await?;
                self.sent.insert(
                    id,
                    Sent {
                        uuid: peer.uuid,
                        state: r.state,
                        version: r.version,
                    },
                );
            }
            if index % 8 == 7 {
                tokio::task::yield_now().await;
            }
        }
        self.flush_packets(output).await?;
        #[cfg(test)]
        {
            crate::perf::FANOUT.fetch_add(fanout, Relaxed);
            crate::perf::POSITIONS.fetch_add(positions, Relaxed);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile(id: u8) -> Arc<Profile> {
        Arc::new(Profile {
            uuid: [id; 16],
            name: format!("Probe{id}"),
            properties: Vec::new(),
        })
    }
    fn replica(x: f64) -> Replica {
        let mut view = ChunkView::default();
        view.set_center(ChunkPos::from_position(x, 8.).unwrap());
        while let Some(p) = view.next_missing() {
            assert!(view.insert(p));
        }
        Replica {
            state: EntitySnapshot {
                position: [x, 8., 8.],
                rotation: [0f32; 2],
                on_ground: true,
                crouching: false,
                sprinting: false,
            },
            view,
            ready: true,
            version: 0,
            #[cfg(test)]
            at: Instant::now(),
        }
    }
    impl InstanceDir {
        fn test_join(
            &mut self,
            entity: i32,
            token: u64,
            profile: Arc<Profile>,
            replica: Replica,
        ) -> io::Result<Inbox> {
            self.join_pending(entity, token, profile, pending(replica))
        }
        fn test_update(&mut self, entity: i32, replica: Replica) {
            let token = self.members[&entity].token;
            self.members[&entity].pending.lock().unwrap().latest = replica;
            self.update(entity, token);
        }
    }
    #[test]
    fn interest_edges_negative_coordinates_coalescing_and_leave() {
        let metrics = Arc::new(Metrics::default());
        let mut d = InstanceDir::new(Arc::clone(&metrics));
        let mut a = d.test_join(1, 1, profile(1), replica(8.)).unwrap();
        let b = d.test_join(2, 2, profile(2), replica(9.)).unwrap();
        assert_eq!(a.roster.borrow_and_update().len(), 1);
        assert_eq!(b.roster.borrow().len(), 1);
        assert_eq!(metrics.visible_edges.load(Relaxed), 2);
        let mut r = replica(9.);
        r.state.crouching = true;
        for n in 0..1000 {
            r.state.rotation[0] = (n % 360) as f32;
            d.test_update(2, r);
        }
        assert!(!a.roster.has_changed().unwrap()); // no queued roster per movement
        let peer = Arc::clone(&a.roster.borrow()[0]);
        assert_eq!(peer.state.borrow().version, 1000);
        assert!(peer.state.borrow().state.crouching);
        assert!(a.chat.is_empty());
        d.test_update(2, replica(-80.));
        assert!(a.roster.borrow().is_empty());
        assert!(b.roster.borrow().is_empty());
        assert_eq!(metrics.visible_edges.load(Relaxed), 0);
        d.test_update(2, replica(-0.1));
        assert_eq!(metrics.visible_edges.load(Relaxed), 2);
        let weak = Arc::downgrade(&peer);
        drop(peer);
        d.leave(2, 2);
        assert!(a.roster.borrow().is_empty());
        d.leave(1, 1);
        drop(a);
        drop(b);
        assert!(weak.upgrade().is_none());
        assert!(d.members.is_empty() && d.occupants.is_empty() && d.subscribers.is_empty());
        assert_eq!(metrics.visible_edges.load(Relaxed), 0);
        assert_eq!(metrics.active_replicas.load(Relaxed), 0);
    }
    #[test]
    fn pending_teleport_and_unsent_chunks_do_not_expose_entities() {
        let metrics = Arc::new(Metrics::default());
        let mut d = InstanceDir::new(metrics);
        let a = d.test_join(1, 1, profile(1), replica(8.)).unwrap();
        let mut r = replica(9.);
        r.ready = false;
        let b = d.test_join(2, 2, profile(2), r).unwrap();
        assert!(a.roster.borrow().is_empty());
        assert!(b.roster.borrow().is_empty());
        r.ready = true;
        r.view = ChunkView::default();
        d.test_update(2, r);
        assert_eq!(a.roster.borrow().len(), 1);
        assert!(b.roster.borrow().is_empty());
        r.view = replica(9.).view;
        d.test_update(2, r);
        assert_eq!(b.roster.borrow().len(), 1);
    }
    #[test]
    fn slow_chat_reader_is_bounded_and_does_not_close_healthy_reader() {
        let metrics = Arc::new(Metrics::default());
        let mut d = InstanceDir::new(Arc::clone(&metrics));
        let slow = d.test_join(1, 1, profile(1), replica(8.)).unwrap();
        let mut healthy = d.test_join(2, 2, profile(2), replica(9.)).unwrap();
        let packet: Arc<[u8]> = presence::system("[System] test").unwrap().into();
        for _ in 0..1000 {
            d.deliver(1, Arc::clone(&packet));
            d.deliver(2, Arc::clone(&packet));
            healthy.chat.try_recv().unwrap();
        }
        assert_eq!(slow.chat.len(), CHAT_CAPACITY);
        assert!(*slow.closed.borrow());
        assert!(!*healthy.closed.borrow());
        assert_eq!(metrics.slow_viewers.load(Relaxed), 1);
    }
    #[test]
    fn chat_rate_is_per_sender_and_never_signed_player_chat() {
        let metrics = Arc::new(Metrics::default());
        let mut d = InstanceDir::new(Arc::clone(&metrics));
        let _a = d.test_join(1, 1, profile(1), replica(8.)).unwrap();
        let mut b = d.test_join(2, 2, profile(2), replica(9.)).unwrap();
        for _ in 0..10 {
            d.chat(1, "hello".into());
        }
        assert_eq!(metrics.chat_messages.load(Relaxed), 3);
        for _ in 0..3 {
            let p = b.chat.try_recv().unwrap();
            assert_eq!(p[0], 0x7c);
            assert!(p.windows(14).any(|s| s == b"[System relay]"));
        }
        assert!(b.chat.is_empty());
    }
    #[tokio::test]
    async fn joined_entities_conflict_within_a_world() {
        let metrics = Arc::new(Metrics::default());
        let hub = Hub::start(Arc::clone(&metrics));
        let (lease, inbox) = hub
            .join(1, WorldId(1), profile(1), replica(8.))
            .await
            .unwrap();
        assert!(
            hub.join(1, WorldId(1), profile(2), replica(8.))
                .await
                .is_err()
        );
        tokio::task::yield_now().await;
        assert_eq!(metrics.active_replicas.load(Relaxed), 1); // rejected join's leave cannot remove original
        while hub
            .sender
            .try_send(Command::Update {
                entity: 999,
                instance: WorldId(1),
                token: 0,
            })
            .is_ok()
        {}
        drop(lease);
        drop(inbox);
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while metrics.active_replicas.load(Relaxed) != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let (lease, inbox) = hub
            .join(1, WorldId(1), profile(1), replica(8.))
            .await
            .unwrap(); // new generation
        hub.sender
            .try_send(Command::Leave(WorldId(1), 1, 1))
            .unwrap(); // stale lease
        tokio::task::yield_now().await;
        assert_eq!(metrics.active_replicas.load(Relaxed), 1);
        drop(lease);
        drop(inbox);
        hub.shutdown().await.unwrap();
        assert_eq!(metrics.active_replicas.load(Relaxed), 0);
        assert_eq!(metrics.visible_edges.load(Relaxed), 0);
    }
    #[tokio::test]
    async fn cancelled_join_reclaims_its_directory_membership() {
        let metrics = Arc::new(Metrics::default());
        let hub = Arc::new(Hub::start(Arc::clone(&metrics)));
        let cloned = Arc::clone(&hub);
        let task =
            tokio::spawn(async move { cloned.join(1, WorldId(1), profile(1), replica(8.)).await });
        tokio::task::yield_now().await;
        task.abort();
        drop(task.await);
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                // FIFO barrier waits for an earlier cancelled Join/Leave pair.
                let pair = hub
                    .join(2, WorldId(1), profile(2), replica(8.))
                    .await
                    .unwrap();
                let active = metrics.active_replicas.load(Relaxed);
                drop(pair);
                if active == 1 {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        hub.shutdown().await.unwrap();
        assert_eq!(metrics.active_replicas.load(Relaxed), 0);
    }

    #[tokio::test]
    async fn instances_share_a_directory_but_do_not_see_each_other() {
        let metrics = Arc::new(Metrics::default());
        let hub = Hub::start(Arc::clone(&metrics));
        // Two identical worlds, each with one player at the SAME entity id (1): same
        // content, different instances. Membership must not cross the instance boundary.
        let (lease_a, mut inbox_a) = hub
            .join(1, WorldId(1), profile(1), replica(8.))
            .await
            .unwrap();
        let (lease_b, mut inbox_b) = hub
            .join(1, WorldId(2), profile(2), replica(8.))
            .await
            .unwrap();
        tokio::task::yield_now().await;
        assert_eq!(metrics.active_replicas.load(Relaxed), 2);
        // Each instance holds exactly one member, so each presence roster is empty: a player's
        // roster lists OTHER visible entities (never itself), otherwise flush() would spawn the
        // client's own entity. Two members in different worlds must each be invisible to one
        // another, which both rosters being empty proves.
        assert_eq!(inbox_a.roster.borrow().len(), 0);
        assert_eq!(inbox_b.roster.borrow().len(), 0);
        lease_a.chat("hi".to_string()).unwrap();
        tokio::task::yield_now().await;
        let got_b =
            tokio::time::timeout(std::time::Duration::from_millis(150), inbox_b.chat.recv()).await;
        assert!(
            got_b.is_err(),
            "instance B must not receive instance A chat"
        );
        let got_a =
            tokio::time::timeout(std::time::Duration::from_secs(1), inbox_a.chat.recv()).await;
        assert!(got_a.is_ok(), "instance A should receive its own relay");
        drop(lease_a);
        drop(inbox_a);
        drop(lease_b);
        drop(inbox_b);
        hub.shutdown().await.unwrap();
        assert_eq!(metrics.active_replicas.load(Relaxed), 0);
    }
    #[tokio::test]
    async fn ingress_coalesces_ten_thousand_snapshots_to_latest_state() {
        let metrics = Arc::new(Metrics::default());
        let hub = Hub::start(Arc::clone(&metrics));
        let (lease, inbox) = hub
            .join(1, WorldId(1), profile(1), replica(8.))
            .await
            .unwrap();
        let mut latest = replica(8.);
        for n in 0..10000 {
            latest.state.rotation[0] = (n % 360) as f32;
            lease.update(latest).unwrap();
        }
        let (other, observer) = hub
            .join(2, WorldId(1), profile(2), replica(9.))
            .await
            .unwrap();
        assert_eq!(metrics.replica_updates.load(Relaxed), 1);
        assert_eq!(
            observer.roster.borrow()[0].state.borrow().state.rotation,
            latest.state.rotation
        );
        drop(lease);
        drop(inbox);
        drop(other);
        drop(observer);
        hub.shutdown().await.unwrap();
        assert_eq!(metrics.active_replicas.load(Relaxed), 0);
    }
    #[test]
    fn stale_update_chat_leave_and_global_fanout_do_not_cross_generations() {
        let metrics = Arc::new(Metrics::default());
        let mut d = Directory::new(Arc::clone(&metrics));
        let (reply, _) = oneshot::channel();
        d.command(Command::Join {
            entity: 1,
            instance: WorldId(1),
            token: 2,
            profile: profile(1),
            pending: pending(replica(8.)),
            reply,
        });
        d.command(Command::Update {
            entity: 1,
            instance: WorldId(1),
            token: 1,
        });
        d.command(Command::Chat {
            entity: 1,
            instance: WorldId(1),
            token: 1,
            text: "stale".into(),
        });
        d.command(Command::Leave(WorldId(1), 1, 1));
        assert_eq!(metrics.active_replicas.load(Relaxed), 1);
        assert_eq!(metrics.replica_updates.load(Relaxed), 0);
        assert_eq!(metrics.chat_messages.load(Relaxed), 0);
        d.fanout_credit = 0.;
        d.fanout_at = Instant::now();
        d.command(Command::Chat {
            entity: 1,
            instance: WorldId(1),
            token: 2,
            text: "bounded".into(),
        });
        assert_eq!(metrics.chat_messages.load(Relaxed), 0);
        d.command(Command::Leave(WorldId(1), 1, 2));
        assert!(d.instances.is_empty());
        assert_eq!(metrics.active_replicas.load(Relaxed), 0);
    }
}
