//! M2C optional authenticated local control plane. No JVM or world-owned worker.
#![cfg(unix)]
mod codec;
use codec::*;
use smolmc_sessions::{
    Service,
    control::{ControlError, ControlEvent},
};
use smolmc_world::{Material, WorldError, WorldHandle, WorldProfile};
use std::{
    io,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering::Relaxed},
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
    sync::{broadcast, watch},
    task::{JoinHandle, JoinSet},
};
use zeroize::Zeroizing;
const IO_TIMEOUT: Duration = Duration::from_secs(2);
const PEERS: usize = 4;
static NEXT_PEER: AtomicU64 = AtomicU64::new(1);
pub const API_VERSION: &str = "0.4.0";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Observe,
    Control,
}
struct Paths(PathBuf);
impl Drop for Paths {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0.join("socket"));
        let _ = std::fs::remove_file(self.0.join("token"));
        let _ = std::fs::remove_dir(&self.0);
    }
}
struct Rate {
    start: Instant,
    requests: usize,
    edits: usize,
}
impl Rate {
    fn take(&mut self, edits: usize) -> bool {
        if self.start.elapsed() >= Duration::from_secs(1) {
            self.start = Instant::now();
            self.requests = 0;
            self.edits = 0;
        }
        if self.requests >= 2048 || self.edits + edits > 16384 {
            return false;
        }
        self.requests += 1;
        self.edits += edits;
        true
    }
}
struct State {
    service: Arc<Service>,
    token: Zeroizing<[u8; 32]>,
    access: Access,
    rate: Mutex<Rate>,
}
/// Explicit bridge owner. Shutdown joins/aborts bounded peers and removes its files.
/// Dropping also signals shutdown; call shutdown() to observe complete cleanup.
pub struct Bridge {
    stop: watch::Sender<bool>,
    task: Option<JoinHandle<io::Result<()>>>,
}
impl Bridge {
    pub fn start(service: Arc<Service>, directory: &Path, access: Access) -> io::Result<Self> {
        std::fs::DirBuilder::new().mode(0o700).create(directory)?;
        let paths = Paths(directory.to_owned());
        let mut token = Zeroizing::new([0; 32]);
        openssl::rand::rand_bytes(&mut *token).map_err(io::Error::other)?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.join("token"))?;
        std::io::Write::write_all(&mut file, &*token)?;
        let listener = UnixListener::bind(directory.join("socket"))?;
        std::fs::set_permissions(
            directory.join("socket"),
            std::fs::Permissions::from_mode(0o600),
        )?;
        service.enable_control().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "bridge must be enabled before Play membership",
            )
        })?;
        let state = Arc::new(State {
            service,
            token,
            access,
            rate: Mutex::new(Rate {
                start: Instant::now(),
                requests: 0,
                edits: 0,
            }),
        });
        let (stop, mut done) = watch::channel(false);
        let task = tokio::spawn(async move {
            let _paths = paths;
            let mut peers = JoinSet::new();
            loop {
                tokio::select! {
                    _=done.changed()=>break,
                    Some(_)=peers.join_next(),if !peers.is_empty()=>{},
                    accepted=listener.accept()=>{
                        let (socket,_)=accepted?;
                        if peers.len()>=PEERS {drop(socket);continue;}
                        let owner=NEXT_PEER.try_update(Relaxed,Relaxed,|id|id.checked_add(1)).map_err(|_|io::Error::other("bridge identities exhausted"))?;
                        let state=Arc::clone(&state);peers.spawn(async move {let _=peer(socket,state,owner).await;});
                    }
                }
            }
            peers.abort_all();
            while peers.join_next().await.is_some() {}
            Ok(())
        });
        Ok(Self {
            stop,
            task: Some(task),
        })
    }
    pub async fn shutdown(mut self) -> io::Result<()> {
        let _ = self.stop.send(true);
        self.task.take().unwrap().await.map_err(io::Error::other)?
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}
struct Registration {
    service: Arc<Service>,
    owner: u64,
}
impl Drop for Registration {
    fn drop(&mut self) {
        self.service.unregister_commands(self.owner, None);
    }
}

// Progress lives outside next() so selecting events cannot discard a partial frame.
#[derive(Default)]
struct Frames {
    prefix: [u8; 4],
    prefix_used: usize,
    body: Vec<u8>,
    used: usize,
    started: Option<tokio::time::Instant>,
}
impl Frames {
    async fn next(&mut self, socket: &mut tokio::net::unix::OwnedReadHalf) -> io::Result<Vec<u8>> {
        while self.prefix_used < 4 {
            let n = if let Some(started) = self.started {
                tokio::time::timeout_at(
                    started + IO_TIMEOUT,
                    socket.read(&mut self.prefix[self.prefix_used..]),
                )
                .await
                .map_err(|_| invalid())??
            } else {
                socket.read(&mut self.prefix[self.prefix_used..]).await?
            };
            self.started.get_or_insert_with(tokio::time::Instant::now);
            if n == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            self.prefix_used += n;
        }
        let len = u32::from_be_bytes(self.prefix) as usize;
        if !(10..=MAX_FRAME).contains(&len) {
            return Err(invalid());
        }
        if self.body.is_empty() {
            self.body.resize(len, 0);
        }
        while self.used < len {
            let n = tokio::time::timeout_at(
                self.started.unwrap() + IO_TIMEOUT,
                socket.read(&mut self.body[self.used..]),
            )
            .await
            .map_err(|_| invalid())??;
            if n == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            self.used += n;
        }
        self.prefix_used = 0;
        self.used = 0;
        self.started = None;
        Ok(std::mem::take(&mut self.body))
    }
}
async fn write(socket: &mut tokio::net::unix::OwnedWriteHalf, body: &[u8]) -> io::Result<()> {
    if body.len() > MAX_FRAME {
        return Err(invalid());
    }
    tokio::time::timeout(IO_TIMEOUT, async {
        socket.write_all(&(body.len() as u32).to_be_bytes()).await?;
        socket.write_all(body).await
    })
    .await
    .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?
}
fn response(id: u64, op: u8, status: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![2];
    u64(&mut out, id);
    out.push(op);
    u16(&mut out, status);
    out.extend(payload);
    out
}
fn status(error: ControlError) -> u16 {
    match error {
        ControlError::InvalidRequest => 1,
        ControlError::Backpressure => 9,
        ControlError::Conflict => 10,
        ControlError::Disabled => 8,
        ControlError::World(e) => match e {
            WorldError::StaleHandle => 4,
            WorldError::Busy => 9,
            WorldError::UnknownInstance
            | WorldError::UnknownPlayer
            | WorldError::UnknownTemplate => 5,
            WorldError::Occupied => 6,
            WorldError::FeatureDisabled | WorldError::UnsupportedFeature => 8,
            WorldError::InstanceNotReady => 11,
            WorldError::InvalidPosition | WorldError::InvalidBudget | WorldError::BatchTooLarge => {
                1
            }
            WorldError::TemplateConflict => 10,
            _ => 7,
        },
    }
}
fn failure(id: u64, op: u8, code: u16, message: &str) -> Vec<u8> {
    let mut p = Vec::new();
    string(&mut p, message);
    response(id, op, code, &p)
}
async fn peer(socket: UnixStream, state: Arc<State>, owner: u64) -> io::Result<()> {
    let (mut input, mut output) = socket.into_split();
    let mut frames = Frames::default();
    let mut hello = Zeroizing::new(
        tokio::time::timeout(IO_TIMEOUT, frames.next(&mut input))
            .await
            .map_err(|_| invalid())??,
    );
    let mut r = Reader(&hello);
    if r.u8()? != 1 {
        return Err(invalid());
    }
    let id = r.u64()?;
    if r.u8()? != 0 || id == 0 {
        return Err(invalid());
    }
    let major = r.u16()?;
    let minor = r.u16()?;
    let token = r.take(32)?;
    r.end()?;
    if !openssl::memcmp::eq(token, &*state.token) {
        write(&mut output, &failure(id, 0, 3, "authentication failed")).await?;
        return Err(invalid());
    }
    if major != 1 || minor > 3 {
        write(
            &mut output,
            &failure(id, 0, 2, "requires bridge schema 1.0, 1.1, 1.2 or 1.3"),
        )
        .await?;
        return Err(invalid());
    }
    hello.fill(0);
    let mut p = Vec::new();
    p.extend(state.service.server_id());
    u16(&mut p, 1);
    u16(&mut p, minor);
    string(
        &mut p,
        match minor {
            0 => "0.1.0",
            1 => "0.2.0",
            2 => "0.3.0",
            _ => API_VERSION,
        },
    );
    p.push(u8::from(state.access == Access::Control));
    write(&mut output, &response(id, 0, 0, &p)).await?;
    let _registration = Registration {
        service: Arc::clone(&state.service),
        owner,
    };
    let mut events = None;
    let mut mask = 0u32;
    let mut last = id;
    loop {
        let body = tokio::select! {
            bytes=frames.next(&mut input)=>bytes?,
            event=next_event(&mut events)=>{
                let event=event?;
                if event.owner.is_none_or(|id|id==owner)&&mask&(1<<event.kind)!=0 {
                    let mut out=vec![3];u64(&mut out,event.sequence);out.push(event.kind);world(&mut out,event.world);out.push(u8::from(event.player.is_some()));if let Some(player)=&event.player {codec::player(&mut out,player);}string(&mut out,&event.text);
                    if let Some(notice)=&event.combat {combat(&mut out,notice.state);out.extend(notice.source.to_be_bytes());out.extend(notice.damage.to_be_bytes());}
                    write(&mut output,&out).await?;
                }
                continue;
            }
        };
        let mut r = Reader(&body);
        if r.u8()? != 1 {
            return Err(invalid());
        }
        let id = r.u64()?;
        let op = r.u8()?;
        if id <= last {
            return Err(invalid());
        }
        last = id;
        if r.take(16)? != state.service.server_id() {
            write(
                &mut output,
                &failure(id, op, 4, "server incarnation expired"),
            )
            .await?;
            continue;
        }
        let request = match Request::parse(op, &mut r) {
            Ok(v) => v,
            Err(_) => {
                write(&mut output, &failure(id, op, 1, "invalid request payload")).await?;
                continue;
            }
        };
        if minor < 3 && op == 24 {
            write(
                &mut output,
                &failure(id, op, 8, "requires bridge schema 1.3"),
            )
            .await?;
            continue;
        }
        if minor == 0
            && matches!(
                request,
                Request::Combat(..) | Request::Subscribe(0x800..=u32::MAX)
            )
        {
            write(
                &mut output,
                &failure(id, op, 8, "requires bridge schema 1.1"),
            )
            .await?;
            continue;
        }
        if minor < 2
            && (op >= 20
                || matches!(&request,Request::Create(p) if p.enabled(smolmc_world::Feature::BlockInteractions)))
        {
            write(
                &mut output,
                &failure(id, op, 8, "requires bridge schema 1.2"),
            )
            .await?;
            continue;
        }
        if state.access != Access::Control && request.mutates() {
            write(&mut output, &failure(id, op, 3, "control grant required")).await?;
            continue;
        }
        if !state.rate.lock().unwrap().take(request.edits()) {
            write(
                &mut output,
                &failure(id, op, 9, "shared bridge work budget exceeded"),
            )
            .await?;
            tokio::time::sleep(Duration::from_millis(1)).await;
            continue;
        }
        let result = match request {
            Request::Subscribe(value) => {
                mask = value;
                events = Some(state.service.control_events().map_err(|_| invalid())?);
                Ok(Vec::new())
            }
            Request::TerrainEdit(world, edits) => state
                .service
                .edit_terrain(world, edits)
                .await
                .map(|_| Vec::new())
                .map_err(ControlError::from),
            Request::Unsubscribe => {
                mask = 0;
                events = None;
                Ok(Vec::new())
            }
            request => request.execute(&state.service, owner),
        };
        let out = match result {
            Ok(p) => response(id, op, 0, &p),
            Err(e) => failure(id, op, status(e), &format!("{e:?}")),
        };
        write(&mut output, &out).await?;
        tokio::task::yield_now().await;
    }
}
async fn next_event(
    events: &mut Option<broadcast::Receiver<Arc<ControlEvent>>>,
) -> io::Result<Arc<ControlEvent>> {
    match events {
        Some(rx) => rx
            .recv()
            .await
            .map_err(|_| io::Error::other("bridge event gap or shutdown; resnapshot required")),
        None => std::future::pending().await,
    }
}
fn combat(out: &mut Vec<u8>, s: smolmc_sessions::control::CombatSnapshot) {
    out.extend(s.health.to_be_bytes());
    out.push(u8::from(s.dead));
    out.push(s.selected);
    out.extend(s.main.to_be_bytes());
    out.extend(s.off.to_be_bytes());
}
enum Request {
    Default,
    Create(WorldProfile),
    Clone(WorldHandle),
    Reset(WorldHandle),
    Destroy(WorldHandle),
    Park(WorldHandle),
    Resources(WorldHandle),
    Edit(WorldHandle, Vec<([i32; 3], Material)>),
    Read(WorldHandle, Vec<[i32; 3]>),
    Players,
    Transfer(i32, [u8; 16], WorldHandle),
    Message(i32, [u8; 16], String),
    Subscribe(u32),
    Unsubscribe,
    Register(String),
    Unregister(String),
    Invoke(String),
    Worlds,
    Combat(i32, [u8; 16]),
    Inventory(i32, [u8; 16]),
    TerrainRead(WorldHandle, Vec<[i32; 3]>),
    TerrainEdit(WorldHandle, Vec<([i32; 3], Material)>),
    Template,
    State(i32),
}
impl Request {
    fn parse(op: u8, r: &mut Reader<'_>) -> io::Result<Self> {
        let result = match op {
            1 => Self::Default,
            2 => {
                if r.u64()? != 1 {
                    return Err(invalid());
                }
                Self::Create(match r.u8()? {
                    0 => WorldProfile::read_only(),
                    1 => WorldProfile::editable(),
                    2 => WorldProfile::builder(),
                    _ => return Err(invalid()),
                })
            }
            3 => Self::Clone(r.world()?),
            4 => Self::Reset(r.world()?),
            5 => Self::Destroy(r.world()?),
            6 => Self::Park(r.world()?),
            7 => Self::Resources(r.world()?),
            8 => {
                let w = r.world()?;
                let count = r.u16()? as usize;
                if count > 4096 {
                    return Err(invalid());
                }
                let mut edits = Vec::with_capacity(count);
                for _ in 0..count {
                    let p = [r.i32()?, r.i32()?, r.i32()?];
                    let m = match r.u8()? {
                        0 => Material::Air,
                        1 => Material::Stone,
                        _ => return Err(invalid()),
                    };
                    edits.push((p, m));
                }
                Self::Edit(w, edits)
            }
            9 => Self::Read(r.world()?, r.positions()?),
            10 => Self::Players,
            11 | 12 => {
                let id = r.i32()?;
                let uuid = r.take(16)?.try_into().unwrap();
                if op == 11 {
                    Self::Transfer(id, uuid, r.world()?)
                } else {
                    Self::Message(id, uuid, r.string(1024)?.to_owned())
                }
            }
            13 => {
                let mask = r.u32()?;
                if mask & !0x3ffe != 0 {
                    return Err(invalid());
                }
                Self::Subscribe(mask)
            }
            14 => Self::Unsubscribe,
            15 => Self::Register(r.string(32)?.to_owned()),
            16 => Self::Unregister(r.string(32)?.to_owned()),
            17 => Self::Invoke(r.string(256)?.to_owned()),
            18 => Self::Worlds,
            24 => Self::Inventory(r.i32()?, r.take(16)?.try_into().unwrap()),
            19 => Self::Combat(r.i32()?, r.take(16)?.try_into().unwrap()),
            20 => Self::Template,
            21 => Self::State(r.i32()?),
            22 => Self::TerrainRead(r.world()?, r.positions()?),
            23 => {
                let world = r.world()?;
                let n = r.u16()? as usize;
                if n > 4096 {
                    return Err(invalid());
                }
                let mut edits = Vec::with_capacity(n);
                for _ in 0..n {
                    let p = [r.i32()?, r.i32()?, r.i32()?];
                    let m = Material::from_state(r.i32()?).ok_or_else(invalid)?;
                    edits.push((p, m));
                }
                Self::TerrainEdit(world, edits)
            }
            _ => return Err(invalid()),
        };
        r.end()?;
        Ok(result)
    }
    fn mutates(&self) -> bool {
        !matches!(
            self,
            Self::Default
                | Self::Resources(_)
                | Self::Read(..)
                | Self::Players
                | Self::Subscribe(_)
                | Self::Unsubscribe
                | Self::Worlds
                | Self::Combat(..)
                | Self::Inventory(..)
                | Self::Template
                | Self::State(..)
                | Self::TerrainRead(..)
        )
    }
    fn edits(&self) -> usize {
        if let Self::Edit(_, e) | Self::TerrainEdit(_, e) = self {
            e.len()
        } else {
            0
        }
    }
    fn execute(self, s: &Service, owner: u64) -> Result<Vec<u8>, ControlError> {
        let mut out = Vec::new();
        match self {
            Self::Template => {
                u64(&mut out, 1);
                string(
                    &mut out,
                    if s.imported_terrain().is_some() {
                        "imported"
                    } else {
                        "stone_flat"
                    },
                );
            }
            Self::State(id) => {
                let m = Material::from_state(id).ok_or(ControlError::InvalidRequest)?;
                out.extend(id.to_be_bytes());
                string(&mut out, m.info().name);
                string(&mut out, m.info().properties);
                out.push(u8::from(m.solid()));
            }
            Self::TerrainRead(w, p) => {
                let materials = s.read_blocks(w, &p)?;
                u16(&mut out, materials.len() as u16);
                for m in materials {
                    out.extend(m.state().to_be_bytes());
                }
            }
            Self::TerrainEdit(..) => return Err(ControlError::InvalidRequest),
            Self::Combat(entity, uuid) => combat(&mut out, s.combat_snapshot(entity, uuid)?),
            Self::Inventory(entity, uuid) => {
                let inv = s.inventory_snapshot(entity, uuid)?;
                out.extend(inv.revision.to_be_bytes());
                out.push(inv.selected);
                u16(&mut out, 46);
                for item in inv.slots.into_iter().chain(std::iter::once(inv.cursor)) {
                    out.extend(item.id().to_be_bytes());
                    out.push(item.count());
                    u16(&mut out, item.damage());
                    u16(&mut out, item.info().map_or(0, |i| i.max_damage));
                }
                let (armor, toughness) = inv.armor();
                out.extend(armor.to_be_bytes());
                out.extend(toughness.to_be_bytes());
            }
            Self::Default => world(&mut out, s.default_world()?),
            Self::Create(p) => world(&mut out, s.create_world_with_profile(p)?),
            Self::Clone(w) => world(&mut out, s.clone_world(w)?),
            Self::Reset(w) => world(&mut out, s.reset_world(w)?),
            Self::Destroy(w) => s.destroy_world(w)?,
            Self::Park(w) => s.park_world(w)?,
            Self::Resources(w) => {
                let r = s.world_resources(w)?;
                string(&mut out, &format!("{:?}", r.state));
                out.push(u8::from(
                    r.profile.enabled(smolmc_world::Feature::WorldEdits),
                ));
                for n in [
                    r.players,
                    r.edits,
                    r.modified_chunks,
                    r.overlay_charged_bytes,
                    r.owned_tasks,
                    r.owned_timers,
                ] {
                    u64(&mut out, n as u64);
                }
            }
            Self::Edit(w, e) => s.edit_world_batch(w, &e)?,
            Self::Read(w, p) => {
                let m = s.read_blocks(w, &p)?;
                u16(&mut out, m.len() as u16);
                for material in m {
                    out.push(if material == Material::Air {
                        0
                    } else if material == Material::Stone {
                        1
                    } else {
                        return Err(ControlError::Disabled);
                    });
                }
            }
            Self::Players => {
                let players = s.list_players();
                u16(&mut out, players.len() as u16);
                for p in players {
                    codec::player(&mut out, &p);
                }
            }
            Self::Transfer(id, uuid, w) => s.control_player(id, uuid, Some(w), None)?,
            Self::Message(id, uuid, text) => s.control_player(id, uuid, None, Some(text))?,
            Self::Register(name) => s.register_command(owner, &name)?,
            Self::Unregister(name) => s.unregister_commands(owner, Some(&name)),
            Self::Invoke(command) => s.invoke_command(&command)?,
            Self::Worlds => {
                let worlds = s.list_worlds();
                u16(&mut out, worlds.len() as u16);
                for w in worlds {
                    world(&mut out, w);
                }
            }
            Self::Subscribe(_) | Self::Unsubscribe => unreachable!(),
        };
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn directory(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "smc-{}-{}-{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    async fn connect(
        path: &Path,
    ) -> (
        tokio::net::unix::OwnedReadHalf,
        tokio::net::unix::OwnedWriteHalf,
        Frames,
    ) {
        let socket = UnixStream::connect(path.join("socket")).await.unwrap();
        let (mut r, mut w) = socket.into_split();
        let mut hello = vec![1];
        u64(&mut hello, 1);
        hello.push(0);
        u16(&mut hello, 1);
        u16(&mut hello, 0);
        hello.extend(std::fs::read(path.join("token")).unwrap());
        write(&mut w, &hello).await.unwrap();
        let mut f = Frames::default();
        let body = f.next(&mut r).await.unwrap();
        assert_eq!(&body[10..12], &[0, 0]);
        (r, w, f)
    }
    #[test]
    fn shared_work_and_edit_budgets_reject_then_refill() {
        let mut rate = Rate {
            start: Instant::now(),
            requests: 0,
            edits: 0,
        };
        for _ in 0..2048 {
            assert!(rate.take(0));
        }
        assert!(!rate.take(0));
        rate.start = Instant::now() - Duration::from_secs(2);
        assert!(rate.take(4096));
        for _ in 0..3 {
            assert!(rate.take(4096));
        }
        assert!(!rate.take(1));
        rate.start = Instant::now() - Duration::from_secs(2);
        assert!(rate.take(1));
    }
    #[tokio::test]
    async fn peer_ownership_never_reuses_across_bridge_owners() {
        let service = Arc::new(Service::new().unwrap());
        let a = directory("peer-a");
        let b = directory("peer-b");
        let bridge_a = Bridge::start(Arc::clone(&service), &a, Access::Control).unwrap();
        let bridge_b = Bridge::start(Arc::clone(&service), &b, Access::Control).unwrap();
        let (mut r, mut w, mut f) = connect(&a).await;
        let mut register = vec![1];
        u64(&mut register, 2);
        register.push(15);
        register.extend(service.server_id());
        string(&mut register, "kept");
        write(&mut w, &register).await.unwrap();
        assert_eq!(&f.next(&mut r).await.unwrap()[10..12], &[0, 0]);
        let (r2, w2, _f) = connect(&b).await;
        drop((r2, w2));
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(service.invoke_command("kept").is_ok());
        bridge_a.shutdown().await.unwrap();
        assert_eq!(
            service.invoke_command("kept"),
            Err(ControlError::InvalidRequest)
        );
        bridge_b.shutdown().await.unwrap();
        service.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn off_has_no_bus_private_paths_exclusive_owner_and_clean_shutdown() {
        let service = Arc::new(Service::new().unwrap());
        assert!(matches!(
            service.control_events(),
            Err(ControlError::Disabled)
        ));
        let path = directory("owner");
        let bridge = Bridge::start(Arc::clone(&service), &path, Access::Observe).unwrap();
        for (name, mode) in [("", 0o700), ("socket", 0o600), ("token", 0o600)] {
            assert_eq!(
                std::fs::metadata(path.join(name))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                mode
            );
        }
        assert!(Bridge::start(Arc::clone(&service), &path, Access::Control).is_err());
        assert!(path.join("socket").exists());
        let (_r, _w, _f) = connect(&path).await;
        bridge.shutdown().await.unwrap();
        assert!(!path.exists());
        service.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn event_flood_disconnects_gap_and_core_recovers() {
        let service = Arc::new(Service::new().unwrap());
        let path = directory("lag");
        let bridge = Bridge::start(Arc::clone(&service), &path, Access::Control).unwrap();
        let (mut r, mut w, mut f) = connect(&path).await;
        let mut sub = vec![1];
        u64(&mut sub, 2);
        sub.push(13);
        sub.extend(service.server_id());
        sub.extend(0x7feu32.to_be_bytes());
        write(&mut w, &sub).await.unwrap();
        f.next(&mut r).await.unwrap();
        // No await: more than the bounded ring before the sidecar can consume it.
        let mut world = service.default_world().unwrap();
        for _ in 0..256 {
            world = service.reset_world(world).unwrap();
        }
        assert!(
            tokio::time::timeout(Duration::from_secs(3), f.next(&mut r))
                .await
                .unwrap()
                .is_err()
        );
        let (_r, _w, _f) = connect(&path).await;
        assert_eq!(service.world_resources(world).unwrap().edits, 0);
        bridge.shutdown().await.unwrap();
        service.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn partial_frame_survives_cancel_and_trailing_mutation_is_atomic() {
        let service = Arc::new(Service::new().unwrap());
        let path = directory("partial");
        let bridge = Bridge::start(Arc::clone(&service), &path, Access::Control).unwrap();
        let (mut r, mut w, mut f) = connect(&path).await;
        let mut body = vec![1];
        u64(&mut body, 2);
        body.push(2);
        body.extend(service.server_id());
        u64(&mut body, 1);
        body.push(1);
        body.push(99);
        let n = (body.len() as u32).to_be_bytes();
        w.write_all(&n[..2]).await.unwrap();
        // Authenticated partial input is bounded and times out independently of core.
        tokio::time::sleep(Duration::from_millis(10)).await;
        w.write_all(&n[2..]).await.unwrap();
        w.write_all(&body).await.unwrap();
        let reply = f.next(&mut r).await.unwrap();
        assert_eq!(&reply[10..12], &[0, 1]);
        assert_eq!(service.list_worlds().len(), 1);
        bridge.shutdown().await.unwrap();
        service.shutdown().await.unwrap();
    }
}
