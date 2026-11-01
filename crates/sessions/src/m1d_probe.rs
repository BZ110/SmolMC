//! Private test-only post-authentication Play probes. Never compiled into the
//! production server, no auth mode/endpoint override and no session-service load.
use super::*;
use smolmc_network::{Config, Metrics as NetworkMetrics, serve_with, transport::ByteBudget};
use std::{collections::HashMap, path::PathBuf};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
};

struct Server {
    address: std::net::SocketAddr,
    stop: oneshot::Sender<()>,
    task: JoinHandle<io::Result<()>>,
    service: Arc<Service>,
    network: Arc<NetworkMetrics>,
    worlds: Vec<WorldHandle>,
}
impl Server {
    async fn start() -> Self {
        Self::partitioned(1).await
    }
    async fn partitioned(count: usize) -> Self {
        Self::partitioned_mode(count, false).await
    }
    async fn partitioned_mode(count: usize, cloned: bool) -> Self {
        Self::configured(count, cloned, false).await
    }
    async fn configured(count: usize, cloned: bool, pvp: bool) -> Self {
        let service = Arc::new(Service::new().unwrap());
        if pvp {
            service
                .configure_pvp(gameplay::Config {
                    enabled: true,
                    ..Default::default()
                })
                .unwrap();
        }
        Self::serve_service(service, count, cloned).await
    }
    async fn serve_service(service: Arc<Service>, count: usize, cloned: bool) -> Self {
        let mut worlds = vec![service.default_world().unwrap()];
        if cloned && service.imported_terrain().is_none() {
            service
                .edit_world(worlds[0], [8, 62, 8], Material::Air)
                .unwrap();
        }
        for _ in 1..count {
            worlds.push(if cloned {
                service.clone_world(worlds[0]).unwrap()
            } else {
                service.create_world().unwrap()
            });
        }
        let routes = Arc::new(worlds.clone());
        let network = Arc::new(NetworkMetrics::default());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, done) = oneshot::channel();
        let sessions = Arc::clone(&service);
        let metrics = Arc::clone(&network);
        let next = Arc::new(AtomicI32::new(1));
        let task = tokio::spawn(serve_with(
            listener,
            Config {
                max_connections: 512,
                accepts_per_second: 10000,
                accepts_per_ip_per_second: 10000,
                ..Config::default()
            },
            metrics,
            async {
                let _ = done.await;
            },
            move |mut connection, context| {
                let service = Arc::clone(&sessions);
                let id = next.fetch_add(1, Relaxed);
                let world = routes[(id as usize - 1) % routes.len()].id;
                async move {
                    // Synthetic identities are internal fixtures only. This path is
                    // absent from the actual executable; it measures Play, not auth.
                    let mut uuid = [0; 16];
                    uuid[..4].copy_from_slice(&id.to_be_bytes());
                    let profile = Arc::new(login::Profile {
                        uuid,
                        name: format!("Probe{id}"),
                        properties: Vec::new(),
                    });
                    connection.enable_encryption(&[7; 16])?;
                    connection.enable_compression(256)?;
                    connection.inbound.set_limit(CONTROL_LIMIT)?;
                    {
                        let result = service
                            .play(&mut connection, &context, id, profile, world)
                            .await;
                        if let Err(error) = &result {
                            eprintln!("probe_entity={id} kind={:?} reason={error}", error.kind());
                        }
                        result
                    }
                }
            },
        ));
        Self {
            address,
            stop,
            task,
            service,
            network,
            worlds,
        }
    }
    async fn stop(self) -> (Snapshot, smolmc_network::Snapshot) {
        self.stop.send(()).unwrap();
        self.task.await.unwrap().unwrap();
        self.service.shutdown().await.unwrap();
        if let Ok(path) = std::env::var("SMOLMC_ITEMS_TIMING") {
            std::fs::write(path, format!("{:#}\n", crate::perf::snapshot())).unwrap();
        }
        (self.service.metrics.snapshot(), self.network.snapshot())
    }
}
#[derive(Default, Debug)]
struct Stats {
    chunks: usize,
    spawns: usize,
    removes: usize,
    positions: usize,
    poses: usize,
    crouches: usize,
    chats: usize,
    keepalives: usize,
    server_closed: bool,
}
#[derive(Default, Clone, Debug)]
struct Observation {
    entities: HashMap<i32, [u8; 16]>,
    blocks: HashMap<[i32; 3], i32>,
    chats: Vec<String>,
    system_messages: Vec<String>,
    health: Option<f32>,
    swings: usize,
    hurts: usize,
    deaths: usize,
    respawns: usize,
    velocities: usize,
    last_velocity: [f64; 3],
    inventory: Vec<i32>,
    stacks: Vec<wire::pvp::Item>,
    revision: i32,
    items: HashMap<i32, wire::pvp::Item>,
    item_positions: HashMap<i32, [f64; 3]>,
    pickups: usize,
    armor: HashMap<i32, [i32; 4]>,
    equipped: HashMap<i32, [i32; 2]>,
    sounds: HashMap<String, usize>,
    sweeps: usize,
    crits: usize,
    chunks: usize,
    unloads: usize,
    teleports: usize,
    position: [f64; 3],
    block_acks: Vec<i32>,
    light_updates: usize,
}
struct Client {
    observation: Arc<Mutex<Observation>>,
    stop: oneshot::Sender<()>,
    task: JoinHandle<io::Result<Stats>>,
    commands: tokio::sync::mpsc::Sender<Vec<Vec<u8>>>,
}
impl Client {
    async fn start(address: std::net::SocketAddr, chat: bool, spoof: bool) -> Self {
        Self::start_mode(address, chat, spoof, true).await
    }
    async fn stationary(address: std::net::SocketAddr) -> Self {
        Self::start_mode(address, false, false, false).await
    }
    async fn start_mode(
        address: std::net::SocketAddr,
        chat: bool,
        spoof: bool,
        moving: bool,
    ) -> Self {
        let budget = ByteBudget::new(1024 * 1024);
        let mut connection = Connection::new(TcpStream::connect(address).await.unwrap(), budget);
        connection.inbound.receive_server_packets().unwrap();
        connection.inbound.set_work_limit(8 * 1024 * 1024).unwrap();
        connection.enable_encryption(&[7; 16]).unwrap();
        connection.enable_compression(256).unwrap();
        let (stop, done) = oneshot::channel();
        let (ready, loaded) = oneshot::channel();
        let observation = Arc::new(Mutex::new(Observation::default()));
        let (commands, ingress) = tokio::sync::mpsc::channel(8);
        let task = tokio::spawn(pump(
            connection,
            done,
            ready,
            (chat, spoof, moving),
            Arc::clone(&observation),
            ingress,
        ));
        timeout(Duration::from_secs(20), loaded)
            .await
            .unwrap()
            .unwrap();
        Self {
            stop,
            task,
            observation,
            commands,
        }
    }
    async fn send(&self, packet: Vec<u8>) {
        self.commands.send(vec![packet]).await.unwrap();
    }
    async fn sequence(&self, packets: Vec<Vec<u8>>) {
        self.commands.send(packets).await.unwrap();
    }
    fn observe(&self) -> Observation {
        self.observation.lock().unwrap().clone()
    }
    async fn wait(&self, predicate: impl Fn(&Observation) -> bool) {
        timeout(Duration::from_secs(5), async {
            while !predicate(&self.observe()) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("observation predicate timed out: {:?}", self.observe()));
    }
    async fn stop(self) -> Stats {
        let _ = self.stop.send(());
        self.task.await.unwrap().unwrap()
    }
}
async fn pump(
    mut connection: Connection,
    mut done: oneshot::Receiver<()>,
    ready: oneshot::Sender<()>,
    modes: (bool, bool, bool),
    observation: Arc<Mutex<Observation>>,
    mut commands: tokio::sync::mpsc::Receiver<Vec<Vec<u8>>>,
) -> io::Result<Stats> {
    let (chat, spoof, moving) = modes;
    let mut ready = Some(ready);
    let mut stats = Stats::default();
    let mut info = HashSet::new();
    let mut self_id = None;
    let mut entities = HashMap::new();
    let mut ticks = 0u64;
    let mut pvp_mode = false;
    let combat = std::env::var("SMOLMC_PROBE_COMBAT").as_deref() == Ok("1");
    let imported = std::env::var_os("SMOLMC_PROBE_IMPORT").is_some();
    let mut command_sent = false;
    let mut interval = tokio::time::interval(Duration::from_millis(50));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let packet = tokio::select! {
            _=&mut done=>return Ok(stats),
            Some(packets)=commands.recv()=>{connection.outbound.send_batch(&packets).await?;continue;},
            _=interval.tick(),if ready.is_none()=>{
                if !moving {continue;}
                ticks+=1;
                let t=ticks as f64*0.05;
                let mut e=login::Encoder::new(0x1f);
                for v in [8.+2.*t.sin(),if imported {if combat{200.}else{201.}} else if combat {64.} else {65.},(if imported{-40.}else{8.})+2.*t.cos()] {e.raw(&v.to_be_bytes());}
                for v in [((ticks*3)%360) as f32,0.] {e.raw(&v.to_be_bytes());}e.byte(0);
                connection.outbound.send(&e.finish()).await?;
                if ticks.is_multiple_of(20) {connection.outbound.send(&[0x2b,if ticks.is_multiple_of(40) {0}else{32}]).await?;}
                if chat && ticks%40==20 {
                    let mut e=login::Encoder::new(9);e.string("probe hello");e.raw(&[0;16]);e.boolean(false);e.varint(0);e.raw(&[0;3]);e.byte(1);
                    connection.outbound.send(&e.finish()).await?;
                }
                if combat && ticks.is_multiple_of(13)&&let Some(&target)=entities.keys().min(){let mut attack=login::Encoder::new(1);attack.varint(target);connection.outbound.send(&attack.finish()).await?;}
                if spoof && ticks==2 {connection.outbound.send(&[0x2a,0xff,0xff,0xff,0xff,7,1,0]).await?;}
                continue;
            },
            packet=connection.inbound.read_packet()=>match packet {
                Ok(p)=>p,
                Err(e) if matches!(e.kind(),io::ErrorKind::UnexpectedEof|io::ErrorKind::ConnectionReset)=>{stats.server_closed=true;return Ok(stats);},
                Err(e)=>return Err(e),
            }
        };
        let mut r = Reader::new(&packet);
        let id = r.varint().map_err(malformed)?;
        match id {
            0x10 => {
                // Independent bounded command-tree decoder in the private client.
                let n = r.varint().unwrap();
                assert!((1..=129).contains(&n));
                let mut arena = false;
                for _ in 0..n {
                    let flags = r.byte().unwrap();
                    let children = r.varint().unwrap();
                    assert!((0..=64).contains(&children));
                    for _ in 0..children {
                        let child = r.varint().unwrap();
                        assert!((0..n).contains(&child));
                    }
                    match flags {
                        0 => {}
                        5 => {
                            arena |= r.string(32).unwrap() == "arena";
                        }
                        6 => {
                            assert_eq!(r.string(32).unwrap(), "args");
                            assert_eq!(r.varint().unwrap(), 5);
                            assert_eq!(r.varint().unwrap(), 2);
                        }
                        _ => panic!("invalid command flags"),
                    }
                }
                assert_eq!(r.varint().unwrap(), 0);
                r.finish().unwrap();
                if arena && !command_sent {
                    command_sent = true;
                    let mut command = login::Encoder::new(7);
                    command.string("arena b");
                    connection.outbound.send(&command.finish()).await?;
                }
            }
            0x49 => {
                let teleport_id = r.varint().unwrap();
                observation.lock().unwrap().teleports += 1;
                let position = r.take(24).unwrap();
                observation.lock().unwrap().position = std::array::from_fn(|i| {
                    f64::from_be_bytes(position[i * 8..i * 8 + 8].try_into().unwrap())
                });
                r.take(24).unwrap();
                let rotation = r.take(8).unwrap();
                assert_eq!(r.take(4).unwrap(), [0; 4]);
                r.finish().unwrap();
                let mut ack = vec![0];
                wire::encode_varint(teleport_id, &mut ack);
                ack.extend_from_slice(position);
                ack.extend_from_slice(rotation);
                connection.outbound.send(&ack).await?;
            }
            0x2e => {
                assert!(packet.len() > 1000 && packet.len() < 256 * 1024);
                stats.chunks += 1;
                observation.lock().unwrap().chunks += 1;
            }
            0x08 => {
                let pos = play::unpack_block_pos(u64::from_be_bytes(
                    r.take(8).unwrap().try_into().unwrap(),
                ));
                let state = r.varint().unwrap();
                r.finish().unwrap();
                observation.lock().unwrap().blocks.insert(pos, state);
            }
            0x26 => {
                let z = i32::from_be_bytes(r.take(4).unwrap().try_into().unwrap());
                let x = i32::from_be_bytes(r.take(4).unwrap().try_into().unwrap());
                r.finish().unwrap();
                let mut o = observation.lock().unwrap();
                o.unloads += 1;
                o.blocks
                    .retain(|p, _| p[0].div_euclid(16) != x || p[2].div_euclid(16) != z);
            }
            0x0b => {
                assert!((1..=4).contains(&r.varint().unwrap()));
                r.finish().unwrap();
                let mut ack = vec![0x0b];
                ack.extend_from_slice(&4f32.to_be_bytes());
                connection.outbound.send(&ack).await?;
                if stats.chunks >= 25
                    && let Some(ready) = ready.take()
                {
                    let _ = ready.send(());
                    if !pvp_mode {
                        connection.outbound.send(&[0x28, 2]).await?;
                    }
                }
            }
            0x2d => {
                let mut ack = vec![0x1c];
                ack.extend_from_slice(r.take(8).unwrap());
                r.finish().unwrap();
                connection.outbound.send(&ack).await?;
                stats.keepalives += 1;
            }
            0x47 => {
                assert_eq!(r.byte().unwrap(), 0x8f);
                assert_eq!(r.varint().unwrap(), 1);
                let uuid: [u8; 16] = r.take(16).unwrap().try_into().unwrap();
                if self_id.is_none() {
                    self_id = Some(i32::from_be_bytes(uuid[..4].try_into().unwrap()));
                }
                r.string(16).unwrap();
                let count = r.varint().unwrap();
                assert!(count <= 16);
                for _ in 0..count {
                    r.string(64).unwrap();
                    r.string(32767).unwrap();
                    if r.boolean().unwrap() {
                        r.string(1024).unwrap();
                    }
                }
                assert!(!r.boolean().unwrap());
                let mode = r.varint().unwrap();
                assert!((0..=2).contains(&mode));
                assert!(r.boolean().unwrap());
                assert!(r.boolean().unwrap());
                r.finish().unwrap();
                info.insert(uuid);
            }
            0x01 => {
                let entity = r.varint().unwrap();
                let uuid: [u8; 16] = r.take(16).unwrap().try_into().unwrap();
                let kind = r.varint().unwrap();
                if kind == wire::items::ITEM_ENTITY {
                    let pos = std::array::from_fn(|_| {
                        f64::from_be_bytes(r.take(8).unwrap().try_into().unwrap())
                    });
                    wire::pvp::read_lp(&mut r).unwrap();
                    r.take(3).unwrap();
                    assert_eq!(r.varint().unwrap(), 0);
                    r.finish().unwrap();
                    let mut o = observation.lock().unwrap();
                    o.items.insert(entity, wire::pvp::Item::Empty);
                    o.item_positions.insert(entity, pos);
                    continue;
                }
                assert!(info.contains(&uuid));
                assert_eq!(kind, 159);
                r.take(24).unwrap();
                assert_eq!(r.byte().unwrap(), 0);
                r.take(3).unwrap();
                assert_eq!(r.varint().unwrap(), 0);
                r.finish().unwrap();
                assert!(entities.insert(entity, uuid).is_none());
                stats.spawns += 1;
            }
            0x23 => {
                let entity = r.varint().unwrap();
                assert_eq!(r.varint().unwrap(), 0);
                let pos = std::array::from_fn(|_| {
                    f64::from_be_bytes(r.take(8).unwrap().try_into().unwrap())
                });
                r.take(8).unwrap();
                if observation.lock().unwrap().items.contains_key(&entity) {
                    observation
                        .lock()
                        .unwrap()
                        .item_positions
                        .insert(entity, pos);
                } else {
                    assert!(entities.contains_key(&entity));
                }
                r.boolean().unwrap();
                r.finish().unwrap();
                stats.positions += 1;
            }
            0x55 => {
                assert!(entities.contains_key(&r.varint().unwrap()));
                r.byte().unwrap();
                r.finish().unwrap();
            }
            0x65 => {
                let entity = r.varint().unwrap();
                let first = r.byte().unwrap();
                if first == 8 {
                    assert_eq!(r.varint().unwrap(), wire::items::ITEM_SERIALIZER);
                    let item = probe_item(&mut r);
                    assert_eq!(r.byte().unwrap(), 255);
                    r.finish().unwrap();
                    assert!(
                        observation
                            .lock()
                            .unwrap()
                            .items
                            .insert(entity, item)
                            .is_some()
                    );
                    continue;
                }
                if first == 9 {
                    assert_eq!(r.varint().unwrap(), 3);
                    let health = f32::from_be_bytes(r.take(4).unwrap().try_into().unwrap());
                    assert!((0.0..=20.).contains(&health));
                    assert_eq!(r.byte().unwrap(), 255);
                    r.finish().unwrap();
                    continue;
                }
                assert!(entities.contains_key(&entity) || Some(entity) == self_id);
                assert_eq!(first, 0);
                assert_eq!(r.varint().unwrap(), 0);
                r.byte().unwrap();
                assert_eq!(r.byte().unwrap(), 6);
                assert_eq!(r.varint().unwrap(), 20);
                let pose = r.varint().unwrap();
                assert!(pose == 0 || pose == 5);
                stats.crouches += usize::from(pose == 5);
                assert_eq!(r.byte().unwrap(), 255);
                r.finish().unwrap();
                stats.poses += 1;
            }
            0x4e => {
                assert_eq!(r.varint().unwrap(), 1);
                let entity = r.varint().unwrap();
                let item = observation.lock().unwrap().items.remove(&entity).is_some();
                observation.lock().unwrap().item_positions.remove(&entity);
                assert!(item || entities.remove(&entity).is_some());
                r.finish().unwrap();
                stats.removes += 1;
            }
            0x46 => {
                assert_eq!(r.varint().unwrap(), 1);
                let uuid: [u8; 16] = r.take(16).unwrap().try_into().unwrap();
                assert!(info.remove(&uuid));
                r.finish().unwrap();
            }
            0x7c => {
                assert_eq!(r.byte().unwrap(), 8);
                let len = u16::from_be_bytes(r.take(2).unwrap().try_into().unwrap());
                let text = r.take(len as usize).unwrap();
                assert!(text.starts_with(b"[System"));
                let mut observation = observation.lock().unwrap();
                if text.starts_with(b"[System relay]") {
                    stats.chats += 1;
                    observation
                        .chats
                        .push(String::from_utf8(text.to_vec()).unwrap());
                } else {
                    observation
                        .system_messages
                        .push(String::from_utf8(text.to_vec()).unwrap());
                }
                assert!(!r.boolean().unwrap());
                r.finish().unwrap();
            }
            0x6a => {
                pvp_mode = true;
                let health = f32::from_be_bytes(r.take(4).unwrap().try_into().unwrap());
                assert!((0.0..=20.).contains(&health));
                assert_eq!(r.varint().unwrap(), 20);
                r.take(4).unwrap();
                r.finish().unwrap();
                observation.lock().unwrap().health = Some(health);
            }
            0x12 => {
                assert_eq!(r.varint().unwrap(), 0);
                let revision = r.varint().unwrap();
                assert!(revision >= 0);
                assert_eq!(r.varint().unwrap(), 46);
                let stacks: Vec<_> = (0..46).map(|_| probe_item(&mut r)).collect();
                let items = stacks.iter().map(|i| i.id()).collect();
                {
                    let mut o = observation.lock().unwrap();
                    o.stacks = stacks;
                    o.revision = revision;
                }
                probe_stack(&mut r);
                r.finish().unwrap();
                observation.lock().unwrap().inventory = items;
            }
            0x68 => {
                let id = r.varint().unwrap();
                assert!(
                    entities.contains_key(&id) || Some(id) == self_id,
                    "equipment id={id} self={self_id:?}"
                );
                let mut items = [0; 6];
                loop {
                    let slot = r.byte().unwrap();
                    let index = (slot & 127) as usize;
                    assert!(index < 6);
                    items[index] = probe_stack(&mut r);
                    if slot & 128 == 0 {
                        break;
                    }
                }
                r.finish().unwrap();
                let mut o = observation.lock().unwrap();
                o.equipped.insert(id, [items[0], items[1]]);
                o.armor.insert(id, [items[2], items[3], items[4], items[5]]);
            }
            0x7f => {
                let id = r.varint().unwrap();
                assert!(observation.lock().unwrap().items.contains_key(&id));
                r.varint().unwrap();
                let n = r.varint().unwrap();
                assert!((1..=64).contains(&n));
                r.finish().unwrap();
                observation.lock().unwrap().pickups += n as usize;
            }
            0x7b => {
                let id = r.varint().unwrap();
                assert!(entities.contains_key(&id));
                assert!((0..=1).contains(&r.varint().unwrap()));
                assert_eq!(r.varint().unwrap(), 1);
                assert_eq!(r.varint().unwrap(), 6);
                r.finish().unwrap();
                observation.lock().unwrap().swings += 1;
            }
            0x19 => {
                r.varint().unwrap();
                r.varint().unwrap();
                r.varint().unwrap();
                r.varint().unwrap();
                assert!(!r.boolean().unwrap());
                r.finish().unwrap();
                observation.lock().unwrap().hurts += 1;
            }
            0x2b => {
                r.varint().unwrap();
                r.take(4).unwrap();
                r.finish().unwrap();
            }
            0x67 => {
                r.varint().unwrap();
                let v = wire::pvp::read_lp(&mut r).unwrap();
                assert!(v.iter().all(|v| v.is_finite() && v.abs() < 8.));
                r.finish().unwrap();
                let mut observed = observation.lock().unwrap();
                observed.velocities += 1;
                observed.last_velocity = v;
            }
            0x22 => {
                r.take(4).unwrap();
                assert_eq!(r.byte().unwrap(), 3);
                r.finish().unwrap();
            }
            0x45 => {
                r.varint().unwrap();
                assert_eq!(r.byte().unwrap(), 8);
                let n = u16::from_be_bytes(r.take(2).unwrap().try_into().unwrap());
                r.take(n as usize).unwrap();
                r.finish().unwrap();
                observation.lock().unwrap().deaths += 1;
                if combat {
                    connection.outbound.send(&[0xc, 0]).await?;
                }
            }
            0x54 => {
                r.varint().unwrap();
                r.string(64).unwrap();
                r.take(8).unwrap();
                assert!([0, 2].contains(&r.varint().unwrap()));
                assert_eq!(r.varint().unwrap(), 0);
                for _ in 0..3 {
                    assert!(!r.boolean().unwrap());
                }
                r.varint().unwrap();
                r.varint().unwrap();
                assert_eq!(r.byte().unwrap(), 0);
                r.finish().unwrap();
                observation.lock().unwrap().respawns += 1;
            }
            0x05 => {
                r.varint().unwrap();
                r.take(8).unwrap();
                let stage = r.byte().unwrap();
                assert!(stage <= 9 || stage == 255);
                r.finish().unwrap();
            }
            0x31 => {
                r.varint().unwrap();
                r.varint().unwrap();
                for _ in 0..4 {
                    r.array(4).unwrap();
                }
                for _ in 0..2 {
                    let n = r.varint().unwrap();
                    assert!((0..=26).contains(&n));
                    for _ in 0..n {
                        assert_eq!(r.array(2048).unwrap().len(), 2048);
                    }
                }
                r.finish().unwrap();
                observation.lock().unwrap().light_updates += 1;
            }
            0x04 => {
                observation
                    .lock()
                    .unwrap()
                    .block_acks
                    .push(r.varint().unwrap());
                r.finish().unwrap();
            }
            0x02 => {
                let entity = r.varint().unwrap();
                assert!(entities.contains_key(&entity) || Some(entity) == self_id);
                assert_eq!(r.byte().unwrap(), 1);
                observation.lock().unwrap().crits += 1;
                r.finish().unwrap();
            }
            0x86 => {
                r.varint().unwrap();
                let n = r.varint().unwrap();
                assert!(n == 1 || n == 3 || n == 4);
                for _ in 0..n {
                    let key = r.varint().unwrap();
                    assert!([1, 2, 5, wire::items::KNOCKBACK_ATTRIBUTE].contains(&key));
                    r.take(8).unwrap();
                    let m = r.varint().unwrap();
                    assert!((0..=1).contains(&m));
                    for _ in 0..m {
                        r.string(64).unwrap();
                        r.take(8).unwrap();
                        r.varint().unwrap();
                    }
                }
                r.finish().unwrap();
            }
            0x77 => {
                assert_eq!(r.varint().unwrap(), 0);
                let name = r.string(128).unwrap();
                assert!(name.starts_with("minecraft:entity.player."));
                assert!(!r.boolean().unwrap());
                assert_eq!(r.varint().unwrap(), 7);
                r.take(12).unwrap();
                assert_eq!(
                    f32::from_be_bytes(r.take(4).unwrap().try_into().unwrap()),
                    1.
                );
                assert_eq!(
                    f32::from_be_bytes(r.take(4).unwrap().try_into().unwrap()),
                    1.
                );
                r.take(8).unwrap();
                r.finish().unwrap();
                *observation
                    .lock()
                    .unwrap()
                    .sounds
                    .entry(name.to_owned())
                    .or_default() += 1;
            }
            0x30 => {
                let particle = r.varint().unwrap();
                assert!(particle == 77 || particle == 14);
                assert!(!r.boolean().unwrap());
                assert!(!r.boolean().unwrap());
                r.take(48).unwrap();
                let count = r.varint().unwrap();
                assert!((0..=10).contains(&count));
                assert_eq!(r.varint().unwrap(), 0);
                r.finish().unwrap();
                if particle == 77 {
                    assert_eq!(count, 0);
                    observation.lock().unwrap().sweeps += 1;
                }
            }
            0x63 | 0x41 | 0x73 | 0x60 | 0x27 | 0x0c => {}
            other => panic!("unexpected probe packet {other:x}"),
        }
        if matches!(id, 0x01 | 0x4e) {
            observation.lock().unwrap().entities = entities.clone();
        }
    }
}

#[tokio::test]
async fn two_peers_wire_chat_pose_rejoin_and_spoof_isolation() {
    let server = Server::start().await;
    let a = Client::start(server.address, true, false).await;
    let b = Client::start(server.address, true, false).await;
    tokio::time::sleep(Duration::from_millis(2200)).await;
    let b = b.stop().await;
    assert!(b.spawns >= 1 && b.positions > 5 && b.crouches > 0 && b.chats >= 2 && !b.server_closed);
    let bad = Client::start(server.address, false, true).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(bad.stop().await.server_closed);
    let c = Client::start(server.address, false, false).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!c.stop().await.server_closed);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let a = a.stop().await;
    assert!(
        a.spawns >= 3
            && a.removes >= 3
            && a.positions > 5
            && a.crouches > 0
            && a.chats >= 2
            && !a.server_closed
    );
    let (s, n) = server.stop().await;
    assert_eq!(s.active_replicas, 0);
    assert_eq!(s.visible_edges, 0);
    assert_eq!(s.slow_viewers, 0);
    assert_eq!(n.frame_bytes_retained, 0);
    assert_eq!(n.active, 0);
    assert_eq!(n.protocol_errors, 1);
}

/// Run as separate server/client test processes for RSS/CPU isolation. No
/// production binary can enable this private test harness.
#[tokio::test]
#[ignore = "explicit separate-process resource workload"]
async fn active_play_resource_probe() {
    let role = std::env::var("SMOLMC_PROBE_ROLE").expect("probe role");
    let ready = PathBuf::from(std::env::var("SMOLMC_PROBE_READY").unwrap());
    let report = PathBuf::from(std::env::var("SMOLMC_PROBE_REPORT").unwrap());
    if role == "server" {
        let worlds: usize = std::env::var("SMOLMC_PROBE_WORLDS")
            .unwrap_or_else(|_| "1".into())
            .parse()
            .unwrap();
        let pvp = std::env::var("SMOLMC_PROBE_PVP").as_deref() == Ok("1");
        let server = if let Ok(path) = std::env::var("SMOLMC_PROBE_IMPORT") {
            let service = Arc::new(
                Service::import_instances(
                    1,
                    WorldProfile::builder(),
                    std::path::Path::new(&path),
                    ImportBounds {
                        min: [-16, -64, -80],
                        max: [48, 320, -16],
                    },
                    None,
                    true,
                )
                .unwrap(),
            );
            let edits = (5..=11)
                .flat_map(|x| (-43..=-37).map(move |z| ([x, 199, z], Material::Stone)))
                .collect();
            service
                .edit_terrain(service.default_world().unwrap(), edits)
                .await
                .unwrap();
            service
                .configure_pvp(gameplay::Config {
                    enabled: pvp,
                    spawn: [8., if pvp { 200. } else { 201. }, -40.],
                })
                .unwrap();
            if std::env::var("SMOLMC_PROBE_ITEMS").as_deref() == Ok("1") {
                service
                    .configure_items(gameplay::ItemConfig {
                        policy: gameplay::InventoryPolicy::ItemsKeep,
                        ..Default::default()
                    })
                    .unwrap();
            }
            Server::serve_service(service, worlds, true).await
        } else {
            Server::configured(worlds, false, pvp).await
        };
        let bridge = std::env::var("SMOLMC_PROBE_BRIDGE_DIRECTORY")
            .ok()
            .map(|path| {
                crate::java_bridge_fixture::Bridge::start(
                    Arc::clone(&server.service),
                    std::path::Path::new(&path),
                    crate::java_bridge_fixture::Access::Control,
                )
                .unwrap()
            });
        std::fs::write(
            &ready,
            serde_json::json!({"address":server.address.to_string(),"pid":std::process::id()})
                .to_string(),
        )
        .unwrap();
        let mut stop =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
        let mut sample =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::user_defined1()).unwrap();
        loop {
            tokio::select! {
                _=stop.recv()=>break,
                _=sample.recv()=>{
                    let value=serde_json::json!({"perf":crate::perf::snapshot(),"sessions":format!("{:?}",server.service.metrics.snapshot()),"network":format!("{:?}",server.network.snapshot())});
                    let path=ready.with_extension("sample");let temporary=ready.with_extension("temporary");
                    std::fs::write(&temporary,value.to_string()).unwrap();std::fs::rename(temporary,path).unwrap();
                }
            }
        }
        if let Some(bridge) = bridge {
            bridge.shutdown().await.unwrap();
        }
        let (s, n) = server.stop().await;
        std::fs::write(
            report,
            serde_json::json!({"sessions":format!("{s:?}"),"network":format!("{n:?}")}).to_string(),
        )
        .unwrap();
    } else {
        let meta: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&ready).unwrap()).unwrap();
        let address = meta["address"].as_str().unwrap().parse().unwrap();
        let count: usize = std::env::var("SMOLMC_PROBE_PLAYERS")
            .unwrap()
            .parse()
            .unwrap();
        assert!((1..=512).contains(&count));
        let mut clients = Vec::new();
        for base in (0..count).step_by(16) {
            let mut joining = tokio::task::JoinSet::new();
            for i in base..(base + 16).min(count) {
                joining.spawn(Client::start(address, i == 0, false));
            }
            while let Some(client) = joining.join_next().await {
                clients.push(client.unwrap());
            }
        }
        std::fs::write(ready.with_extension("clients"), "ready").unwrap();
        let seconds: u64 = std::env::var("SMOLMC_PROBE_SECONDS")
            .unwrap()
            .parse()
            .unwrap();
        tokio::time::sleep(Duration::from_secs(seconds)).await;
        let mut totals = Stats::default();
        let mut closed_clients = 0usize;
        for client in clients {
            let s = client.stop().await;
            closed_clients += usize::from(s.server_closed);
            totals.chunks += s.chunks;
            totals.spawns += s.spawns;
            totals.removes += s.removes;
            totals.positions += s.positions;
            totals.poses += s.poses;
            totals.crouches += s.crouches;
            totals.chats += s.chats;
            totals.keepalives += s.keepalives;
        }
        std::fs::write(report,serde_json::json!({"totals":format!("{totals:?}"),"players":count,"measurement_seconds":seconds,"closed_clients":closed_clients}).to_string()).unwrap();
    }
}

#[tokio::test]
async fn m2a_wire_isolation_edits_switch_reset_destroy_and_cleanup() {
    let server = Server::partitioned(2).await;
    let world_a = server.worlds[0];
    let world_b = server.worlds[1];
    // Round-robin trusted routing, simultaneous players in identical coordinates.
    let a = Client::start(server.address, true, false).await; // entity 1, A
    let b = Client::start(server.address, true, false).await; // entity 2, B
    let c = Client::start(server.address, false, false).await; // entity 3, A
    let d = Client::start(server.address, false, false).await; // entity 4, B
    a.wait(|o| {
        o.entities.keys().copied().collect::<HashSet<_>>() == HashSet::from([3])
            && !o.chats.is_empty()
    })
    .await;
    b.wait(|o| {
        o.entities.keys().copied().collect::<HashSet<_>>() == HashSet::from([4])
            && !o.chats.is_empty()
    })
    .await;
    for client in [&a, &c] {
        assert!(
            client
                .observe()
                .chats
                .iter()
                .all(|m| m.contains("<Probe1>"))
        );
    }
    for client in [&b, &d] {
        assert!(
            client
                .observe()
                .chats
                .iter()
                .all(|m| m.contains("<Probe2>"))
        );
    }
    let block = [8, 62, 8];
    server
        .service
        .edit_world(world_a, block, Material::Air)
        .unwrap();
    a.wait(|o| o.blocks.get(&block) == Some(&0)).await;
    c.wait(|o| o.blocks.get(&block) == Some(&0)).await;
    assert!(!b.observe().blocks.contains_key(&block));
    assert!(!d.observe().blocks.contains_key(&block));
    server
        .service
        .edit_world(world_a, block, Material::Stone)
        .unwrap();
    a.wait(|o| o.blocks.get(&block) == Some(&1)).await;
    assert_eq!(
        server.service.reset_world(world_a),
        Err(WorldError::Occupied)
    );
    assert_eq!(
        server.service.destroy_world(world_b),
        Err(WorldError::Occupied)
    );
    let target_block = [-1, 62, -17];
    server
        .service
        .edit_world(world_b, target_block, Material::Air)
        .unwrap();
    b.wait(|o| o.blocks.get(&target_block) == Some(&0)).await;
    server.service.switch_player(1, world_b).unwrap();
    a.wait(|o| {
        o.teleports == 2
            && o.unloads == 25
            && o.chunks == 50
            && o.entities.keys().copied().collect::<HashSet<_>>() == HashSet::from([2, 4])
    })
    .await;
    c.wait(|o| o.entities.is_empty()).await;
    b.wait(|o| o.entities.keys().copied().collect::<HashSet<_>>() == HashSet::from([1, 4]))
        .await;
    a.wait(|o| o.blocks.get(&target_block) == Some(&0)).await;
    assert!(!a.observe().blocks.contains_key(&block));
    assert!(!a.stop().await.server_closed);
    assert!(!b.stop().await.server_closed);
    assert!(!c.stop().await.server_closed);
    assert!(!d.stop().await.server_closed);
    timeout(Duration::from_secs(2), async {
        while !server.service.worlds.empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let renewed = server.service.reset_world(world_a).unwrap();
    assert_eq!(
        server.service.edit_world(world_a, block, Material::Air),
        Err(WorldError::StaleHandle)
    );
    server.service.destroy_world(renewed).unwrap();
    server.service.destroy_world(world_b).unwrap();
    assert_eq!(
        server.service.edit_world(world_b, block, Material::Air),
        Err(WorldError::UnknownInstance)
    );
    let (s, n) = server.stop().await;
    assert_eq!(s.active_replicas, 0);
    assert_eq!(s.visible_edges, 0);
    assert_eq!(n.active, 0);
    assert_eq!(n.frame_bytes_retained, 0);
    assert_eq!(n.protocol_errors, 0);
    assert_eq!(n.timeouts, 0);
    assert_eq!(n.task_failures, 0);
}

#[tokio::test]
async fn m2b_cloned_worlds_replay_shared_edits_then_diverge_on_the_wire() {
    let server = Server::partitioned_mode(2, true).await;
    let a_world = server.worlds[0];
    let b_world = server.worlds[1];
    let pos = [8, 62, 8];
    let shared = server.service.world_overlay_memory().charged_bytes;
    let a = Client::start(server.address, false, false).await;
    let b = Client::start(server.address, false, false).await;
    let c = Client::start(server.address, false, false).await;
    let d = Client::start(server.address, false, false).await;
    for client in [&a, &b, &c, &d] {
        client.wait(|o| o.blocks.get(&pos) == Some(&0)).await;
    }
    assert_eq!(server.service.world_overlay_memory().charged_bytes, shared);
    server
        .service
        .edit_world_batch(
            a_world,
            &[(pos, Material::Stone), ([-1, 62, -17], Material::Air)],
        )
        .unwrap();
    for client in [&a, &c] {
        client
            .wait(|o| o.blocks.get(&pos) == Some(&1) && o.blocks.get(&[-1, 62, -17]) == Some(&0))
            .await;
    }
    for client in [&b, &d] {
        assert_eq!(client.observe().blocks.get(&pos), Some(&0));
        assert!(!client.observe().blocks.contains_key(&[-1, 62, -17]));
    }
    assert!(!a.stop().await.server_closed);
    assert!(!b.stop().await.server_closed);
    assert!(!c.stop().await.server_closed);
    assert!(!d.stop().await.server_closed);
    timeout(Duration::from_secs(2), async {
        while !server.service.worlds.empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    server.service.destroy_world(a_world).unwrap();
    server.service.destroy_world(b_world).unwrap();
    assert_eq!(server.service.world_overlay_memory().charged_bytes, 0);
    let (s, n) = server.stop().await;
    assert_eq!(s.active_replicas + s.visible_edges, 0);
    assert_eq!(
        (
            n.active,
            n.frame_bytes_retained,
            n.protocol_errors,
            n.timeouts,
            n.task_failures
        ),
        (0, 0, 0, 0, 0)
    );
}

/// The exact production bridge source is linked only into this private fixture,
/// against cfg(test) Service, so no offline authentication API enters the binary.
#[tokio::test]
#[ignore = "requires compiled Java SDK; scripts/java-check.sh then explicit invocation"]
async fn java_sidecar_controls_real_encrypted_play() {
    let server = Server::partitioned(2).await;
    let directory = std::env::temp_dir().join(format!("smc-java-{}", std::process::id()));
    let bridge = crate::java_bridge_fixture::Bridge::start(
        Arc::clone(&server.service),
        &directory,
        crate::java_bridge_fixture::Access::Control,
    )
    .unwrap();
    let a = Client::start(server.address, false, false).await;
    let b = Client::start(server.address, false, false).await;
    assert!(a.observe().entities.is_empty() && b.observe().entities.is_empty());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut java = std::process::Command::new("java")
        .arg("-ea")
        .arg("-cp")
        .arg(root.join("target/java/classes"))
        .arg("dev.smolmc.PlayerIntegration")
        .arg(&directory)
        .spawn()
        .unwrap();
    let status = timeout(Duration::from_secs(20), async {
        loop {
            if let Some(status) = java.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(status.success());
    assert!(
        server.service.metrics.snapshot().movements >= 60,
        "moving players progress during Java event flood"
    );
    a.wait(|o| {
        o.teleports >= 2
            && o.entities.contains_key(&2)
            && o.system_messages
                .iter()
                .any(|m| m.contains("Java arena message"))
            && o.blocks.get(&[8, 62, 8]) == Some(&0)
    })
    .await;
    b.wait(|o| {
        o.entities.contains_key(&1)
            && o.system_messages
                .iter()
                .any(|m| m.contains("Java arena message"))
            && o.blocks.get(&[8, 62, 8]) == Some(&0)
    })
    .await;
    assert!(a.observe().unloads >= 25);
    a.stop().await;
    b.stop().await;
    bridge.shutdown().await.unwrap();
    assert!(!directory.exists());
    let (s, n) = server.stop().await;
    assert_eq!(s.active_replicas, 0);
    assert_eq!(n.active, 0);
    assert_eq!(n.protocol_errors, 0);
}

fn probe_item(r: &mut Reader<'_>) -> wire::pvp::Item {
    let count = r.varint().unwrap();
    if count == 0 {
        return wire::pvp::Item::Empty;
    }
    assert!((1..=64).contains(&count));
    let id = r.varint().unwrap();
    let added = r.varint().unwrap();
    let removed = r.varint().unwrap();
    assert_eq!(removed, 0);
    assert!((0..=1).contains(&added));
    let damage = if added == 1 {
        assert_eq!(r.varint().unwrap(), wire::items::DAMAGE_COMPONENT);
        r.varint().unwrap()
    } else {
        0
    };
    wire::pvp::Item::new(id, count as u8, damage as u16).unwrap()
}
fn probe_stack(r: &mut Reader<'_>) -> i32 {
    probe_item(r).id()
}

#[tokio::test]
async fn pvp_encrypted_inventory_swing_combat_death_respawn_and_world_isolation() {
    pvp_scenario(false).await;
}
#[tokio::test]
#[ignore = "requires compiled Java SDK; explicit PvP/bridge integration"]
async fn pvp_java_sdk_observes_combat() {
    pvp_scenario(true).await;
}
async fn pvp_scenario(with_java: bool) {
    let server = Server::configured(2, false, true).await;
    let directory = std::env::temp_dir().join(format!("smc-pvp-java-{}", std::process::id()));
    let bridge = with_java.then(|| {
        crate::java_bridge_fixture::Bridge::start(
            Arc::clone(&server.service),
            &directory,
            crate::java_bridge_fixture::Access::Control,
        )
        .unwrap()
    });
    let a = Client::start(server.address, false, false).await;
    let isolated = Client::start(server.address, false, false).await;
    let b = Client::start(server.address, false, false).await;
    let ready = directory.with_extension("ready");
    let mut java = if with_java {
        let mut command = std::process::Command::new("java");
        command
            .arg("-ea")
            .arg("-cp")
            .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/java/classes"))
            .arg("dev.smolmc.PvpIntegration")
            .arg(&directory)
            .arg(&ready);
        let process = command.spawn().unwrap();
        timeout(Duration::from_secs(5), async {
            while !ready.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        Some(process)
    } else {
        None
    };
    a.wait(|o| o.equipped.get(&3) == Some(&[1045, 0])).await;
    assert_eq!(a.observe().inventory[36], 1045);
    assert_eq!(a.observe().inventory[9], 1050);
    a.send(vec![0x2e]).await;
    b.wait(|o| o.swings >= 1).await;
    a.send(vec![1, 2]).await;
    tokio::time::sleep(Duration::from_millis(70)).await;
    assert_eq!(isolated.observe().health, Some(20.));
    assert_eq!(isolated.observe().swings, 0);
    a.send(vec![0x29, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).await;
    b.wait(|o| o.equipped.get(&1) == Some(&[0, 1045])).await;
    a.send(vec![0x29, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).await;
    b.wait(|o| o.equipped.get(&1) == Some(&[1045, 0])).await;
    for _ in 0..4 {
        tokio::time::sleep(Duration::from_millis(660)).await;
        a.send(vec![1, 3]).await;
    }
    b.wait(|o| o.deaths == 1 && o.health == Some(0.) && o.velocities >= 1)
        .await;
    assert_eq!(isolated.observe().health, Some(20.));
    b.send(vec![0xc, 0]).await;
    b.wait(|o| o.respawns == 1 && o.teleports >= 2 && o.health == Some(20.))
        .await;
    a.wait(|o| o.equipped.get(&3) == Some(&[1045, 0])).await;
    assert_eq!(b.observe().inventory[36], 1045);
    tokio::time::sleep(Duration::from_millis(660)).await;
    b.send(vec![1, 1]).await;
    a.wait(|o| o.health.is_some_and(|h| h < 20.)).await;
    if let Some(java) = java.as_mut() {
        let status = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(status) = java.try_wait().unwrap() {
                    break status;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert!(status.success());
    }
    a.stop().await;
    b.stop().await;
    isolated.stop().await;
    let reconnect = Client::start(server.address, false, false).await;
    assert_eq!(reconnect.observe().health, Some(20.));
    reconnect.stop().await;
    if let Some(bridge) = bridge {
        bridge.shutdown().await.unwrap();
        std::fs::remove_file(ready).unwrap();
    }
    let (s, n) = server.stop().await;
    assert_eq!(s.active_replicas, 0);
    assert_eq!(n.protocol_errors, 0);
    assert_eq!(n.frame_bytes_retained, 0);
}

#[tokio::test]
async fn pvp_sweep_applies_secondary_damage_and_distinct_effects() {
    let server = Server::configured(1, false, true).await;
    let a = Client::stationary(server.address).await;
    let b = Client::stationary(server.address).await;
    let c = Client::stationary(server.address).await;
    a.wait(|o| o.entities.len() == 2).await;
    let mut movement = login::Encoder::new(0x1e);
    for v in [8_f64, 64., 8.] {
        movement.raw(&v.to_be_bytes());
    }
    movement.byte(1);
    a.sequence(vec![movement.finish(), vec![0x21, 1], vec![1, 2]])
        .await;
    b.wait(|o| o.health == Some(14.) && o.velocities > 0).await;
    c.wait(|o| o.health == Some(19.) && o.velocities > 0).await;
    a.wait(|o| {
        o.sweeps > 0
            && o.sounds
                .contains_key("minecraft:entity.player.attack.sweep")
    })
    .await;
    assert_eq!(a.observe().health, Some(20.));
    a.stop().await;
    b.stop().await;
    c.stop().await;
    let (s, n) = server.stop().await;
    assert_eq!(s.combat_hits, 2);
    assert_eq!(n.protocol_errors, 0);
    assert_eq!(n.active, 0);
}

#[tokio::test]
async fn pvp_sprint_cancellation_ignores_key_intent_then_allows_falling_critical() {
    let server = Server::configured(1, false, true).await;
    let a = Client::stationary(server.address).await;
    let b = Client::stationary(server.address).await;
    a.wait(|o| o.entities.len() == 1).await;
    fn movement(y: f64) -> Vec<u8> {
        let mut e = login::Encoder::new(0x1e);
        for v in [8_f64, y, 8.] {
            e.raw(&v.to_be_bytes());
        }
        e.byte(0);
        e.finish()
    }
    a.sequence(vec![
        vec![0x2a, 1, 1, 0],
        movement(65.2),
        movement(65.1),
        vec![0x2b, 64],
        vec![1, 2],
    ])
    .await;
    a.wait(|o| {
        o.sounds
            .contains_key("minecraft:entity.player.attack.knockback")
    })
    .await;
    b.wait(|o| o.health == Some(14.)).await;
    tokio::time::sleep(Duration::from_millis(660)).await;
    a.sequence(vec![
        movement(65.2),
        movement(65.1),
        vec![0x2b, 64],
        vec![1, 2],
    ])
    .await;
    a.wait(|o| o.sounds.contains_key("minecraft:entity.player.attack.crit"))
        .await;
    b.wait(|o| o.health == Some(5.) && o.crits > 0).await;
    assert_eq!(
        a.observe().sounds["minecraft:entity.player.attack.knockback"],
        1
    );
    a.stop().await;
    b.stop().await;
    let (s, n) = server.stop().await;
    assert_eq!(s.active_replicas, 0);
    assert_eq!(n.protocol_errors, 0);
}

#[tokio::test]
async fn pvp_landing_before_next_position_uses_grounded_knockback_on_wire() {
    let server = Server::configured(1, false, true).await;
    let a = Client::stationary(server.address).await;
    let b = Client::stationary(server.address).await;
    a.wait(|o| o.entities.len() == 1).await;
    let mut movement = login::Encoder::new(0x1e);
    for v in [8_f64, 64.05, 8.] {
        movement.raw(&v.to_be_bytes());
    }
    movement.byte(0);
    b.sequence(vec![
        movement.finish(),
        vec![0x29, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ])
    .await;
    a.wait(|o| o.equipped.get(&2) == Some(&[0, 1045])).await;
    tokio::time::sleep(Duration::from_millis(110)).await;
    a.send(vec![1, 2]).await;
    b.wait(|o| o.health == Some(14.) && o.velocities > 0).await;
    let y = b.observe().last_velocity[1];
    assert!((y - 0.36075).abs() < 0.0001, "vertical impulse {y}");
    a.stop().await;
    b.stop().await;
    let (s, n) = server.stop().await;
    assert_eq!(s.active_replicas, 0);
    assert_eq!(n.protocol_errors, 0);
}
fn terrain_fixture() -> Arc<smolmc_world::terrain::ImportedTerrain> {
    use smolmc_world::terrain::*;
    use std::collections::BTreeMap;
    let bounds = ImportBounds {
        min: [-16, 48, -16],
        max: [48, 96, 48],
    };
    let mut chunks = BTreeMap::new();
    for pos in bounds.chunks() {
        let mut sections: Vec<Option<Section>> = (0..24).map(|_| None).collect();
        sections[7] = Some(Section {
            blocks: Packed::singleton(1, 4096),
            biomes: Packed::singleton(0, 64),
            sky: None,
            block_light: None,
        });
        chunks.insert(
            pos,
            Arc::new(TerrainChunk {
                pos,
                sections: sections.into_boxed_slice(),
                light_sections: BTreeMap::new(),
                bytes: 512,
            }),
        );
    }
    let mut base = ImportedTerrain {
        bounds,
        chunks,
        bytes: 8192,
        input_bytes: 0,
        clipped_blocks: 0,
        source_files: vec![],
        removed_palette_entries: BTreeMap::new(),
    };
    smolmc_world::lighting::relight_base(&mut base).unwrap();
    Arc::new(base)
}
fn item_click(revision: i32, slot: i16, button: u8, kind: i32) -> Vec<u8> {
    let mut p = login::Encoder::new(0x12);
    p.varint(0);
    p.varint(revision);
    p.raw(&slot.to_be_bytes());
    p.byte(button);
    p.varint(kind);
    p.varint(0);
    p.boolean(false);
    p.finish()
}
fn item_dig(action: i32, pos: [i32; 3], sequence: i32) -> Vec<u8> {
    let mut p = login::Encoder::new(0x29);
    p.varint(action);
    p.raw(&play::pack_block_pos(pos).to_be_bytes());
    p.byte(1);
    p.varint(sequence);
    p.finish()
}
fn item_place(pos: [i32; 3], sequence: i32) -> Vec<u8> {
    let mut p = login::Encoder::new(0x42);
    p.varint(0);
    p.raw(&play::pack_block_pos(pos).to_be_bytes());
    p.varint(1);
    for v in [0.5f32, 1., 0.5] {
        p.raw(&v.to_be_bytes());
    }
    p.boolean(false);
    p.boolean(false);
    p.varint(sequence);
    p.finish()
}
#[tokio::test]
async fn placement_replaces_short_and_double_grass_but_keeps_solid_grass_blocks() {
    let service = Arc::new(
        Service::with_terrain_instances(
            1,
            WorldProfile::builder(),
            terrain_fixture(),
            [8., 64., 8.],
        )
        .unwrap(),
    );
    service
        .configure_pvp(gameplay::Config {
            enabled: true,
            spawn: [8., 64., 8.],
        })
        .unwrap();
    service
        .configure_items(gameplay::ItemConfig {
            policy: gameplay::InventoryPolicy::ItemsKeep,
            ..Default::default()
        })
        .unwrap();
    let server = Server::serve_service(service, 1, false).await;
    let short = Material::parse("minecraft:short_grass", None).unwrap();
    let lower = Material::parse("minecraft:tall_grass", Some("half=lower")).unwrap();
    let upper = Material::parse("minecraft:tall_grass", Some("half=upper")).unwrap();
    let solid = Material::parse("minecraft:grass_block", Some("snowy=false")).unwrap();
    server
        .service
        .worlds
        .edit_terrain(
            server.worlds[0],
            vec![
                ([10, 64, 8], short),
                ([12, 64, 8], lower),
                ([12, 65, 8], upper),
                ([10, 63, 10], solid),
            ],
        )
        .await
        .unwrap();
    let a = Client::stationary(server.address).await;
    for (seq, selected, pos) in [
        (1, 2, [10, 64, 8]),
        (2, 3, [12, 64, 8]),
        (3, 4, [10, 63, 10]),
    ] {
        a.send(vec![0x36, 0, selected]).await;
        a.send(item_place(pos, seq)).await;
        a.wait(|o| o.block_acks.contains(&seq) && o.stacks[36 + selected as usize].count() == 0)
            .await;
    }
    assert_eq!(
        server
            .service
            .read_blocks(
                server.worlds[0],
                &[
                    [10, 64, 8],
                    [10, 65, 8],
                    [12, 64, 8],
                    [12, 65, 8],
                    [10, 63, 10],
                    [10, 64, 10]
                ]
            )
            .unwrap(),
        vec![
            Material::Stone,
            Material::Air,
            Material::parse("minecraft:oak_planks", None).unwrap(),
            Material::Air,
            solid,
            Material::parse("minecraft:cobblestone", None).unwrap()
        ]
    );
    a.stop().await;
    let (_, n) = server.stop().await;
    assert_eq!(n.protocol_errors, 0);
}
#[tokio::test]
async fn pipelined_placement_mining_early_finish_and_armor_clicks_reconcile() {
    let service = Arc::new(
        Service::with_terrain_instances(
            1,
            WorldProfile::builder(),
            terrain_fixture(),
            [8., 64., 8.],
        )
        .unwrap(),
    );
    service
        .configure_pvp(gameplay::Config {
            enabled: true,
            spawn: [8., 64., 8.],
        })
        .unwrap();
    service
        .configure_items(gameplay::ItemConfig {
            policy: gameplay::InventoryPolicy::ItemsKeep,
            ..Default::default()
        })
        .unwrap();
    let server = Server::serve_service(service, 1, false).await;
    server
        .service
        .worlds
        .edit_terrain(server.worlds[0], vec![([11, 64, 8], Material::Stone)])
        .await
        .unwrap();
    let a = Client::stationary(server.address).await;
    let b = Client::stationary(server.address).await;
    a.sequence(vec![
        vec![0x36, 0, 2],
        item_place([10, 63, 8], 1),
        vec![0x36, 0, 1],
        item_dig(0, [10, 64, 8], 2),
        item_dig(3, [10, 64, 8], 3),
    ])
    .await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    a.send(item_dig(0, [11, 64, 8], 4)).await;
    tokio::time::sleep(Duration::from_millis(450)).await;
    a.send(item_dig(3, [11, 64, 8], 5)).await;
    a.wait(|o| {
        o.block_acks.contains(&3)
            && o.stacks[37].damage() == 2
            && o.stacks[38].count() == 0
            && o.blocks.get(&[10, 64, 8]) == Some(&0)
            && o.blocks.get(&[11, 64, 8]) == Some(&0)
    })
    .await;
    b.wait(|o| o.blocks.get(&[10, 64, 8]) == Some(&0) && !o.items.is_empty())
        .await;
    a.send(item_click(a.observe().revision, 14, 0, 1)).await;
    a.wait(|o| {
        o.stacks[5].id()
            == wire::pvp::Item::named("minecraft:iron_helmet", 1)
                .unwrap()
                .id()
    })
    .await;
    let revision = a.observe().revision;
    a.sequence(vec![
        item_click(revision, 5, 0, 0),
        item_click(revision, 26, 0, 0),
    ])
    .await;
    a.wait(|o| {
        o.stacks[5] == wire::pvp::Item::Empty
            && o.stacks[26].id()
                == wire::pvp::Item::named("minecraft:iron_helmet", 1)
                    .unwrap()
                    .id()
    })
    .await;
    // Independent players submit in the same lighting window: both are retried,
    // each placement consumes one block, and neither request is silently lost.
    a.sequence(vec![vec![0x36, 0, 3], item_place([11, 63, 8], 6)])
        .await;
    b.sequence(vec![vec![0x36, 0, 3], item_place([12, 63, 8], 1)])
        .await;
    a.wait(|o| o.block_acks.contains(&6) && o.stacks[39].count() == 0)
        .await;
    b.wait(|o| o.block_acks.contains(&1) && o.stacks[39].count() == 0)
        .await;
    assert_eq!(
        server
            .service
            .read_blocks(server.worlds[0], &[[11, 64, 8], [12, 64, 8]])
            .unwrap(),
        vec![Material::parse("minecraft:oak_planks", None).unwrap(); 2]
    );
    a.stop().await;
    b.stop().await;
    let (s, n) = server.stop().await;
    assert_eq!(s.slow_viewers, 0);
    assert_eq!(n.protocol_errors, 0);
    assert_eq!(n.task_failures, 0);
}
#[tokio::test]
async fn items_encrypted_mining_drop_pickup_equipment_armor_durability_and_isolation() {
    items_scenario(false).await;
}
#[tokio::test]
#[ignore = "requires compiled Java SDK; explicit item snapshot integration"]
async fn items_java_sdk_snapshots() {
    items_scenario(true).await;
}
async fn items_scenario(with_java: bool) {
    let service = Arc::new(
        Service::with_terrain_instances(
            1,
            WorldProfile::builder(),
            terrain_fixture(),
            [8., 64., 8.],
        )
        .unwrap(),
    );
    service
        .configure_pvp(gameplay::Config {
            enabled: true,
            spawn: [8., 64., 8.],
        })
        .unwrap();
    service
        .configure_items(gameplay::ItemConfig {
            policy: gameplay::InventoryPolicy::ItemsKeep,
            ..Default::default()
        })
        .unwrap();
    let directory = std::env::temp_dir().join(format!("smc-items-java-{}", std::process::id()));
    let bridge = with_java.then(|| {
        crate::java_bridge_fixture::Bridge::start(
            Arc::clone(&service),
            &directory,
            crate::java_bridge_fixture::Access::Observe,
        )
        .unwrap()
    });
    let server = Server::serve_service(service, 2, false).await;
    // Exposed stone at head height lets both clients view an item before pickup.
    server
        .service
        .worlds
        .edit_terrain(server.worlds[0], vec![([10, 64, 8], Material::Stone)])
        .await
        .unwrap();
    let a = Client::stationary(server.address).await;
    let isolated = Client::stationary(server.address).await;
    let b = Client::stationary(server.address).await;
    a.wait(|o| o.entities.contains_key(&3)).await;
    b.wait(|o| o.entities.contains_key(&1)).await;
    a.send(vec![0x36, 0, 1]).await;
    a.send(item_dig(0, [10, 64, 8], 1)).await;
    a.wait(|o| o.block_acks.contains(&1)).await;
    tokio::time::sleep(Duration::from_millis(450)).await;
    a.send(item_dig(3, [10, 64, 8], 2)).await;
    a.wait(|o| o.stacks[37].damage() == 1 && !o.items.is_empty())
        .await;
    b.wait(|o| !o.items.is_empty()).await;
    assert!(isolated.observe().items.is_empty());
    assert_eq!(
        a.observe().items.values().next().unwrap().id(),
        wire::pvp::Item::named("minecraft:cobblestone", 1)
            .unwrap()
            .id()
    );
    // Move to the item under ordinary movement validation, without trusted pickup.
    tokio::time::sleep(Duration::from_millis(600)).await;
    let mut p = login::Encoder::new(0x1e);
    for v in [9.3f64, 64., 8.] {
        p.raw(&v.to_be_bytes());
    }
    p.byte(1);
    a.send(p.finish()).await;
    a.wait(|o| o.pickups >= 1 && o.items.is_empty()).await;
    assert_eq!(
        a.observe()
            .stacks
            .iter()
            .filter(|s| s.id()
                == wire::pvp::Item::named("minecraft:cobblestone", 1)
                    .unwrap()
                    .id())
            .map(|s| s.count() as u32)
            .sum::<u32>(),
        2
    );
    // Equip iron helmet through a real shift click, and observe it on the peer.
    a.send(item_click(a.observe().revision, 14, 0, 1)).await;
    b.wait(|o| {
        o.armor.get(&1).is_some_and(|a| {
            a[3] == wire::pvp::Item::named("minecraft:iron_helmet", 1)
                .unwrap()
                .id()
        })
    })
    .await;
    a.send(vec![0x36, 0, 0]).await;
    b.send(vec![0x36, 0, 0]).await;
    tokio::time::sleep(Duration::from_millis(700)).await;
    b.send(vec![1, 1]).await;
    a.wait(|o| o.health.is_some_and(|h| h < 20.) && o.stacks[5].damage() > 0)
        .await;
    b.wait(|o| o.stacks[36].damage() == 1).await;
    assert!(a.observe().health.unwrap() > 14.); // helmet mitigation, same six raw damage
    if with_java {
        let mut java = std::process::Command::new("java")
            .args(["-Xmx64m", "-ea", "-cp"])
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/java/classes"))
            .arg("dev.smolmc.ItemsIntegration")
            .arg(&directory)
            .spawn()
            .unwrap();
        let status = timeout(Duration::from_secs(15), async {
            loop {
                if let Some(status) = java.try_wait().unwrap() {
                    break status;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert!(status.success());
    }
    // Q drops the authoritative sword and preserves its component state.
    let mut drop = login::Encoder::new(0x29);
    drop.varint(5);
    drop.raw(&0u64.to_be_bytes());
    drop.byte(0);
    drop.varint(0);
    a.send(drop.finish()).await;
    a.wait(|o| o.stacks[36] == wire::pvp::Item::Empty && !o.items.is_empty())
        .await;
    b.wait(|o| !o.items.is_empty()).await;
    assert!(isolated.observe().items.is_empty());
    a.stop().await;
    b.stop().await;
    isolated.stop().await;
    // Closing fixture sockets does not await the server's membership leases.
    // Reset remains correctly rejected until those sessions have left.
    timeout(Duration::from_secs(2), async {
        while !server.service.worlds.empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let fresh = server.service.reset_world(server.worlds[0]).unwrap();
    assert_eq!(
        server.service.read_blocks(fresh, &[[10, 64, 8]]).unwrap(),
        vec![Material::Air]
    );
    server.service.destroy_world(fresh).unwrap();
    if let Some(bridge) = bridge {
        bridge.shutdown().await.unwrap();
    }
    let (s, n) = server.stop().await;
    assert_eq!(s.active_replicas, 0);
    assert_eq!(n.active, 0);
    assert_eq!(n.protocol_errors, 0);
    assert_eq!(n.task_failures, 0);
    assert_eq!(s.slow_viewers, 0);
}
#[tokio::test]
async fn terrain_encrypted_build_collision_isolation_reset_and_readonly() {
    let service = Arc::new(
        Service::with_terrain_instances(
            1,
            WorldProfile::builder(),
            terrain_fixture(),
            [8., 64., 8.],
        )
        .unwrap(),
    );
    service
        .configure_pvp(gameplay::Config {
            enabled: true,
            spawn: [8., 64., 8.],
        })
        .unwrap();
    let server = Server::serve_service(service, 2, false).await;
    let a = Client::stationary(server.address).await;
    let isolated = Client::stationary(server.address).await;
    let b = Client::stationary(server.address).await;
    assert_eq!(a.observe().inventory[37], 1047);
    a.send(vec![0x36, 0, 2]).await;
    let mut place = login::Encoder::new(0x42);
    place.varint(0);
    place.raw(&play::pack_block_pos([10, 63, 8]).to_be_bytes());
    place.varint(1);
    for v in [0.5f32, 1., 0.5] {
        place.raw(&v.to_be_bytes());
    }
    place.boolean(false);
    place.boolean(false);
    place.varint(1);
    a.send(place.finish()).await;
    a.wait(|o| o.block_acks.contains(&1)).await;
    b.wait(|o| o.blocks.get(&[10, 64, 8]) == Some(&1)).await;
    assert!(!isolated.observe().blocks.contains_key(&[10, 64, 8]));
    assert_eq!(
        server
            .service
            .read_blocks(server.worlds[0], &[[10, 64, 8]])
            .unwrap(),
        vec![Material::Stone]
    );
    let teleports = a.observe().teleports;
    let mut movement = login::Encoder::new(0x1e);
    for v in [11f64, 64., 8.] {
        movement.raw(&v.to_be_bytes());
    }
    movement.byte(1);
    a.send(movement.finish()).await;
    a.wait(|o| o.teleports > teleports).await;
    assert!(a.observe().position[0] <= 9.70001);
    a.send(vec![0x36, 0, 1]).await;
    let mut dig = login::Encoder::new(0x29);
    dig.varint(0);
    dig.raw(&play::pack_block_pos([10, 64, 8]).to_be_bytes());
    dig.byte(1);
    dig.varint(2);
    a.send(dig.finish()).await;
    a.wait(|o| o.block_acks.contains(&2)).await;
    tokio::time::sleep(Duration::from_millis(450)).await;
    let mut finish = login::Encoder::new(0x29);
    finish.varint(3);
    finish.raw(&play::pack_block_pos([10, 64, 8]).to_be_bytes());
    finish.byte(1);
    finish.varint(3);
    a.send(finish.finish()).await;
    b.wait(|o| o.blocks.get(&[10, 64, 8]) == Some(&0)).await;

    a.send(vec![0x36, 0, 1]).await;
    for sequence in 10..55 {
        let mut dig = login::Encoder::new(0x29);
        dig.varint(0);
        dig.raw(&play::pack_block_pos([10, 63, 8]).to_be_bytes());
        dig.byte(1);
        dig.varint(sequence);
        a.sequence(vec![vec![0x2e], dig.finish()]).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    a.wait(|o| o.block_acks.contains(&54)).await;
    let teleports = a.observe().teleports;
    let mut fall = login::Encoder::new(0x1e);
    for v in [9f64, -61., 8.] {
        fall.raw(&v.to_be_bytes());
    }
    fall.byte(0);
    a.send(fall.finish()).await;
    a.wait(|o| o.teleports > teleports).await;
    assert_eq!(a.observe().position, [8., 64., 8.]);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let ro = server
        .service
        .create_world_with_profile(WorldProfile::read_only())
        .unwrap();
    server.service.switch_player(1, ro).unwrap();
    a.wait(|o| o.teleports > teleports + 1).await;
    let mut dig = login::Encoder::new(0x29);
    dig.varint(0);
    dig.raw(&play::pack_block_pos([9, 63, 8]).to_be_bytes());
    dig.byte(1);
    dig.varint(3);
    a.send(dig.finish()).await;
    a.wait(|o| o.block_acks.contains(&3)).await;
    assert_eq!(
        server.service.read_blocks(ro, &[[9, 63, 8]]).unwrap(),
        vec![Material::Stone]
    );
    a.stop().await;
    b.stop().await;
    isolated.stop().await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while !server.service.worlds.empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    server
        .service
        .edit_terrain(server.worlds[1], vec![([8, 64, 8], Material::Stone)])
        .await
        .unwrap();
    let reconnect = Client::stationary(server.address).await;
    assert_eq!(reconnect.observe().position, [8.5, 65., 8.5]);
    reconnect.stop().await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while !server.service.worlds.empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let old = server.worlds[0];
    let new = server.service.reset_world(old).unwrap();
    assert_eq!(
        server.service.read_blocks(new, &[[10, 64, 8]]).unwrap(),
        vec![Material::Air]
    );
    assert!(server.service.read_blocks(old, &[[10, 64, 8]]).is_err());
    println!("terrain_timing={}", crate::perf::snapshot());
    let (s, n) = server.stop().await;
    assert_eq!(s.active_replicas, 0);
    assert_eq!(n.active, 0);
    assert_eq!(n.frame_bytes_retained, 0);
}
#[tokio::test]
async fn terrain_cow_lighting_atomic_edits_reset_pending_and_bounded_failure() {
    let service = Service::with_terrain_instances(
        1,
        WorldProfile::builder(),
        terrain_fixture(),
        [8., 64., 8.],
    )
    .unwrap();
    let a = service.default_world().unwrap();
    let b = service.clone_world(a).unwrap();
    service
        .edit_terrain(a, vec![([0, 64, 0], Material::Stone)])
        .await
        .unwrap();
    assert_eq!(
        service.read_blocks(a, &[[0, 64, 0]]).unwrap(),
        vec![Material::Stone]
    );
    assert_eq!(
        service.read_blocks(b, &[[0, 64, 0]]).unwrap(),
        vec![Material::Air]
    );
    let original = service.worlds.terrain_snapshot(a).unwrap();
    assert!(original.light_sections().count() > 0);
    let clone = service.clone_world(a).unwrap();
    assert!(original.same_snapshot(&service.worlds.terrain_snapshot(clone).unwrap()));
    service
        .edit_terrain(clone, vec![([0, 64, 0], Material::Air)])
        .await
        .unwrap();
    assert!(original.same_snapshot(&service.worlds.terrain_snapshot(a).unwrap()));
    let snapshot = smolmc_world::collision::TerrainSnapshot {
        base: Arc::clone(service.imported_terrain().unwrap()),
        overlay: service.worlds.terrain_snapshot(a).unwrap(),
    };
    assert_eq!(snapshot.safe_spawn([0.5, 64., 0.5]).unwrap()[1], 65.);
    drop(snapshot);
    let current = service.reset_world(clone).unwrap();
    assert_eq!(
        service
            .worlds
            .terrain_snapshot(current)
            .unwrap()
            .light_sections()
            .count(),
        0
    );
    let (tx, rx) = oneshot::channel();
    service
        .worlds
        .submit_terrain(
            b,
            vec![([1, 64, 1], Material::Stone)],
            false,
            None,
            crate::worlds::lighting::Reply::Control(tx),
        )
        .unwrap();
    let fresh = service.reset_world(b).unwrap();
    assert!(rx.await.unwrap().is_err());
    assert_eq!(
        service.read_blocks(fresh, &[[1, 64, 1]]).unwrap(),
        vec![Material::Air]
    );
    let snapshot = service.worlds.terrain_snapshot(a).unwrap();
    assert!(
        service
            .edit_terrain(a, vec![([10000, 64, 0], Material::Stone)])
            .await
            .is_err()
    );
    assert!(snapshot.same_snapshot(&service.worlds.terrain_snapshot(a).unwrap()));
    service.destroy_world(a).unwrap();
    service.destroy_world(current).unwrap();
    service.destroy_world(fresh).unwrap();
    drop(original);
    drop(snapshot);
    assert_eq!(service.worlds.memory().charged_bytes, 0);
    service.shutdown().await.unwrap();
}
#[tokio::test]
async fn imported_falling_contact_does_not_become_sweep_before_landing() {
    let now = Instant::now();
    let mut g = gameplay::State::new(now, [8., 65., 8.]);
    g.terrain = Some(smolmc_world::collision::TerrainSnapshot {
        base: terrain_fixture(),
        overlay: Default::default(),
    });
    g.fall_distance = 0.75;
    g.velocity = [0., -0.4, 0.];
    let pose = presence::EntitySnapshot {
        position: [8., 64.25, 8.],
        rotation: [0.; 2],
        on_ground: false,
        crouching: false,
        sprinting: false,
    };
    g.pose(pose, now);
    g.fall_distance = 0.75;
    g.advance_motion(now + Duration::from_millis(50));
    assert!(!g.motion_state().2);
    assert_eq!(
        g.classify(pose, now + Duration::from_secs(1)),
        gameplay::AttackKind::Critical
    );
}
#[tokio::test]
#[ignore = "requires compiled Java SDK; imported terrain control integration"]
async fn terrain_java_sdk_controls_imported_worlds() {
    let service = Arc::new(
        Service::with_terrain_instances(
            1,
            WorldProfile::builder(),
            terrain_fixture(),
            [8., 64., 8.],
        )
        .unwrap(),
    );
    let directory = std::env::temp_dir().join(format!("smc-terrain-java-{}", std::process::id()));
    let bridge = crate::java_bridge_fixture::Bridge::start(
        Arc::clone(&service),
        &directory,
        crate::java_bridge_fixture::Access::Control,
    )
    .unwrap();
    let mut child = std::process::Command::new("java")
        .arg("-ea")
        .arg("-cp")
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/java/classes"))
        .arg("dev.smolmc.TerrainIntegration")
        .arg(&directory)
        .spawn()
        .unwrap();
    let code = timeout(Duration::from_secs(20), async {
        loop {
            if let Some(code) = child.try_wait().unwrap() {
                break code;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(code.success());
    bridge.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
}
