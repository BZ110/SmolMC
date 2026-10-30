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
        let service = Arc::new(Service::new().unwrap());
        let mut worlds = vec![service.default_world().unwrap()];
        if cloned {
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
    chunks: usize,
    unloads: usize,
    teleports: usize,
}
struct Client {
    observation: Arc<Mutex<Observation>>,
    stop: oneshot::Sender<()>,
    task: JoinHandle<io::Result<Stats>>,
}
impl Client {
    async fn start(address: std::net::SocketAddr, chat: bool, spoof: bool) -> Self {
        let budget = ByteBudget::new(1024 * 1024);
        let mut connection = Connection::new(TcpStream::connect(address).await.unwrap(), budget);
        connection.inbound.set_limit(CONTROL_LIMIT).unwrap();
        connection.inbound.set_work_limit(8 * 1024 * 1024).unwrap();
        connection.enable_encryption(&[7; 16]).unwrap();
        connection.enable_compression(256).unwrap();
        let (stop, done) = oneshot::channel();
        let (ready, loaded) = oneshot::channel();
        let observation = Arc::new(Mutex::new(Observation::default()));
        let task = tokio::spawn(pump(
            connection,
            done,
            ready,
            chat,
            spoof,
            Arc::clone(&observation),
        ));
        timeout(Duration::from_secs(20), loaded)
            .await
            .unwrap()
            .unwrap();
        Self {
            stop,
            task,
            observation,
        }
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
    chat: bool,
    spoof: bool,
    observation: Arc<Mutex<Observation>>,
) -> io::Result<Stats> {
    let mut ready = Some(ready);
    let mut stats = Stats::default();
    let mut info = HashSet::new();
    let mut entities = HashMap::new();
    let mut ticks = 0u64;
    let mut interval = tokio::time::interval(Duration::from_millis(50));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let packet = tokio::select! {
            _=&mut done=>return Ok(stats),
            _=interval.tick(),if ready.is_none()=>{
                ticks+=1;
                let t=ticks as f64*0.05;
                let mut e=login::Encoder::new(0x1f);
                for v in [8.+2.*t.sin(),65.,8.+2.*t.cos()] {e.raw(&v.to_be_bytes());}
                for v in [((ticks*3)%360) as f32,0.] {e.raw(&v.to_be_bytes());}e.byte(0);
                connection.outbound.send(&e.finish()).await?;
                if ticks.is_multiple_of(20) {connection.outbound.send(&[0x2b,if ticks.is_multiple_of(40) {0}else{32}]).await?;}
                if chat && ticks%40==20 {
                    let mut e=login::Encoder::new(9);e.string("probe hello");e.raw(&[0;16]);e.boolean(false);e.varint(0);e.raw(&[0;3]);e.byte(1);
                    connection.outbound.send(&e.finish()).await?;
                }
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
            0x49 => {
                let teleport_id = r.varint().unwrap();
                observation.lock().unwrap().teleports += 1;
                let position = r.take(24).unwrap();
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
                assert!(packet.len() > 35000 && packet.len() < 40000);
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
                    connection.outbound.send(&[0x28, 2]).await?;
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
                assert_eq!(r.varint().unwrap(), 1);
                assert!(r.boolean().unwrap());
                assert!(r.boolean().unwrap());
                r.finish().unwrap();
                info.insert(uuid);
            }
            0x01 => {
                let entity = r.varint().unwrap();
                let uuid: [u8; 16] = r.take(16).unwrap().try_into().unwrap();
                assert!(info.contains(&uuid));
                assert_eq!(r.varint().unwrap(), 159);
                r.take(24).unwrap();
                assert_eq!(r.byte().unwrap(), 0);
                r.take(3).unwrap();
                assert_eq!(r.varint().unwrap(), 0);
                r.finish().unwrap();
                assert!(entities.insert(entity, uuid).is_none());
                stats.spawns += 1;
            }
            0x23 => {
                assert!(entities.contains_key(&r.varint().unwrap()));
                assert_eq!(r.varint().unwrap(), 0);
                r.take(32).unwrap();
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
                assert!(entities.contains_key(&r.varint().unwrap()));
                assert_eq!(r.byte().unwrap(), 0);
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
                assert!(entities.remove(&r.varint().unwrap()).is_some());
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
                if text.starts_with(b"[System relay]") {
                    stats.chats += 1;
                    observation
                        .lock()
                        .unwrap()
                        .chats
                        .push(String::from_utf8(text.to_vec()).unwrap());
                }
                assert!(!r.boolean().unwrap());
                r.finish().unwrap();
            }
            0x63 | 0x41 | 0x73 | 0x60 | 0x27 | 0x0c => {}
            other => panic!("unexpected probe packet {other:x}"),
        }
        observation.lock().unwrap().entities = entities.clone();
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
        let server = Server::start().await;
        std::fs::write(
            ready,
            serde_json::json!({"address":server.address.to_string(),"pid":std::process::id()})
                .to_string(),
        )
        .unwrap();
        let mut stop =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
        stop.recv().await;
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
