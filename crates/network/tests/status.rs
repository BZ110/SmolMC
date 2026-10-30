//! Real loopback TCP tests. Fixtures are independently encoded here rather than
//! round-tripped through the server codec, to catch codec/transport coupling bugs.
use smolmc_network::{Config, Metrics, serve};
use std::{io, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
    time::{sleep, timeout},
};

struct Server {
    address: SocketAddr,
    metrics: Arc<Metrics>,
    stop: oneshot::Sender<()>,
    task: JoinHandle<io::Result<()>>,
}

impl Server {
    async fn start(config: Config) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let metrics = Arc::new(Metrics::default());
        let task_metrics = Arc::clone(&metrics);
        let (stop, receive) = oneshot::channel();
        let task = tokio::spawn(serve(listener, config, task_metrics, async {
            let _ = receive.await;
        }));
        Self {
            address,
            metrics,
            stop,
            task,
        }
    }

    async fn stop(self) {
        self.stop.send(()).unwrap();
        timeout(Duration::from_secs(1), self.task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(self.metrics.snapshot().active, 0);
    }

    async fn connect(&self) -> TcpStream {
        TcpStream::connect(self.address).await.unwrap()
    }
}

fn test_config() -> Config {
    Config {
        accepts_per_second: 10_000,
        accepts_per_ip_per_second: 10_000,
        ..Config::default()
    }
}

fn packet(payload: &[u8]) -> Vec<u8> {
    let mut value = payload.len();
    let mut bytes = Vec::new();
    loop {
        let byte = (value & 127) as u8;
        value >>= 7;
        bytes.push(byte | if value > 0 { 128 } else { 0 });
        if value == 0 {
            break;
        }
    }
    bytes.extend_from_slice(payload);
    bytes
}

const HELLO: &[u8] = b"\x00\x89\x06\x09localhost\x63\xdd\x01";
const PING: &[u8] = b"\x01\x80\x12\x34\x56\x78\x9a\xbc\xde";

async fn receive(socket: &mut TcpStream) -> Vec<u8> {
    timeout(Duration::from_secs(1), async {
        let mut length = 0;
        for shift in [0, 7, 14] {
            let byte = socket.read_u8().await.unwrap();
            length |= usize::from(byte & 127) << shift;
            if byte & 128 == 0 {
                break;
            }
        }
        assert!((1..=4096).contains(&length));
        let mut bytes = vec![0; length];
        socket.read_exact(&mut bytes).await.unwrap();
        bytes
    })
    .await
    .unwrap()
}

fn json_string(payload: &[u8]) -> serde_json::Value {
    assert_eq!(payload[0], 0);
    let mut length = 0;
    let mut used = 1;
    for &byte in &payload[1..] {
        length |= usize::from(byte & 127) << (7 * (used - 1));
        used += 1;
        if byte & 128 == 0 {
            break;
        }
    }
    assert_eq!(payload.len(), used + length);
    serde_json::from_slice(&payload[used..]).unwrap()
}

async fn closed(socket: &mut TcpStream) {
    let result = timeout(Duration::from_secs(1), socket.read_u8())
        .await
        .unwrap();
    assert!(
        matches!(result, Err(ref error) if [io::ErrorKind::UnexpectedEof, io::ErrorKind::ConnectionReset].contains(&error.kind())),
        "{result:?}"
    );
}

async fn wait_active(metrics: &Metrics, expected: usize) {
    timeout(Duration::from_secs(1), async {
        while metrics.snapshot().active != expected {
            sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
}

async fn ping(server: &Server) {
    let mut socket = server.connect().await;
    let bytes = [packet(HELLO), packet(&[0]), packet(PING)].concat();
    socket.write_all(&bytes).await.unwrap();
    let status = json_string(&receive(&mut socket).await);
    assert_eq!(status["version"]["protocol"], 777);
    assert_eq!(status["version"]["name"], "26.3");
    assert_eq!(status["players"]["online"], 0);
    assert_eq!(status["players"]["max"], 0);
    assert_eq!(status["enforcesSecureChat"], false);
    assert!(
        status["description"]["text"]
            .as_str()
            .unwrap()
            .contains("status only")
    );
    assert_eq!(receive(&mut socket).await, PING);
    closed(&mut socket).await;
}

#[tokio::test]
async fn coalesced_status_ping_and_cleanup() {
    let server = Server::start(test_config()).await;
    ping(&server).await;
    wait_active(&server.metrics, 0).await;
    let snapshot = server.metrics.snapshot();
    assert_eq!(snapshot.status_responses, 1);
    assert_eq!(snapshot.pongs, 1);
    assert_eq!(snapshot.protocol_errors, 0);
    server.stop().await;
}

#[tokio::test]
async fn fragmented_frames_and_all_nonce_bits() {
    let server = Server::start(test_config()).await;
    for nonce in [i64::MIN, i64::MAX, -1, 0, 1] {
        let mut socket = server.connect().await;
        for byte in [packet(HELLO), packet(&[0])].concat() {
            socket.write_all(&[byte]).await.unwrap();
            tokio::task::yield_now().await;
        }
        json_string(&receive(&mut socket).await);
        let mut payload = vec![1];
        payload.extend_from_slice(&nonce.to_be_bytes());
        for byte in packet(&payload) {
            socket.write_all(&[byte]).await.unwrap();
            tokio::task::yield_now().await;
        }
        assert_eq!(receive(&mut socket).await, payload);
        closed(&mut socket).await;
    }
    server.stop().await;
}

#[tokio::test]
async fn other_versions_can_discover_target_but_cannot_login() {
    let server = Server::start(test_config()).await;
    let mut socket = server.connect().await;
    let mut hello = HELLO.to_vec();
    hello[1] = 0x88; // 776
    socket
        .write_all(&[packet(&hello), packet(&[0]), packet(PING)].concat())
        .await
        .unwrap();
    assert_eq!(
        json_string(&receive(&mut socket).await)["version"]["protocol"],
        777
    );
    assert_eq!(receive(&mut socket).await, PING);
    for (version_low, intent) in [(0x89, 2), (0x88, 2), (0x89, 3)] {
        let mut socket = server.connect().await;
        let mut hello = HELLO.to_vec();
        hello[1] = version_low;
        *hello.last_mut().unwrap() = intent;
        socket.write_all(&packet(&hello)).await.unwrap();
        let reason = json_string(&receive(&mut socket).await);
        assert!(reason["text"].as_str().unwrap().contains("Stage 1"));
        closed(&mut socket).await;
    }
    assert_eq!(server.metrics.snapshot().login_rejections, 3);
    server.stop().await;
}

#[tokio::test]
async fn malicious_frames_do_not_poison_listener() {
    let server = Server::start(test_config()).await;
    let mut cases = vec![
        vec![0],
        vec![0x80; 3],
        vec![0x81, 8],
        packet(&[1]),
        packet(&[0, 0x80, 0x80, 0x80, 0x80, 0x10]),
    ];
    for bad_intent in [0, 4, 127] {
        let mut hello = HELLO.to_vec();
        *hello.last_mut().unwrap() = bad_intent;
        cases.push(packet(&hello));
    }
    cases.push(packet(b"\x00\x89\x06\x01\xff\x63\xdd\x01"));
    cases.push(packet(b"\x00\x89\x06\xff\xff\xff\xff\x0f"));
    cases.push([packet(HELLO), packet(&[0, 1])].concat());
    cases.push([packet(HELLO), packet(PING)].concat());
    for count in 0..128 {
        let payload = vec![2; count + 1]; // wrong handshake packet ID
        cases.push(packet(&payload));
    }
    let count = cases.len();
    for bytes in cases {
        let mut socket = server.connect().await;
        socket.write_all(&bytes).await.unwrap();
        closed(&mut socket).await;
    }
    wait_active(&server.metrics, 0).await;
    assert_eq!(server.metrics.snapshot().protocol_errors, count as u64);
    ping(&server).await;
    server.stop().await;
}

#[tokio::test]
async fn duplicate_request_and_wrong_ping_size_are_rejected() {
    let server = Server::start(test_config()).await;
    for bad_ping in [&[0][..], &[1; 8][..], &[1; 10][..]] {
        let mut socket = server.connect().await;
        socket
            .write_all(&[packet(HELLO), packet(&[0]), packet(bad_ping)].concat())
            .await
            .unwrap();
        json_string(&receive(&mut socket).await);
        closed(&mut socket).await;
    }
    assert_eq!(server.metrics.snapshot().protocol_errors, 3);
    ping(&server).await;
    server.stop().await;
}

#[tokio::test]
async fn partial_and_idle_connections_expire_and_capacity_recovers() {
    let config = Config {
        exchange_timeout: Duration::from_millis(100),
        max_connections: 2,
        ..test_config()
    };
    let server = Server::start(config).await;
    let mut a = server.connect().await;
    let mut b = server.connect().await;
    b.write_all(&[0x80]).await.unwrap();
    wait_active(&server.metrics, 2).await;
    let mut excess = server.connect().await;
    closed(&mut excess).await;
    closed(&mut a).await;
    closed(&mut b).await;
    wait_active(&server.metrics, 0).await;
    let snapshot = server.metrics.snapshot();
    assert_eq!(snapshot.peak_active, 2);
    assert_eq!(snapshot.rejected, 1);
    assert_eq!(snapshot.timeouts, 2);
    ping(&server).await;
    server.stop().await;
}

#[tokio::test]
async fn status_waiting_for_ping_has_absolute_deadline() {
    let server = Server::start(Config {
        exchange_timeout: Duration::from_millis(100),
        ..test_config()
    })
    .await;
    let mut socket = server.connect().await;
    socket
        .write_all(&[packet(HELLO), packet(&[0])].concat())
        .await
        .unwrap();
    json_string(&receive(&mut socket).await);
    closed(&mut socket).await;
    assert_eq!(server.metrics.snapshot().timeouts, 1);
    server.stop().await;
}

#[tokio::test]
async fn connection_rate_limit_rejects_without_allocating_task() {
    for (global, per_ip) in [(100, 1), (1, 100)] {
        let server = Server::start(Config {
            accepts_per_second: global,
            accepts_per_ip_per_second: per_ip,
            ..test_config()
        })
        .await;
        ping(&server).await;
        let mut excess = server.connect().await;
        closed(&mut excess).await;
        assert_eq!(server.metrics.snapshot().accepted, 1);
        assert_eq!(server.metrics.snapshot().rejected, 1);
        server.stop().await;
    }
}

#[tokio::test]
async fn shutdown_cancels_open_sockets_and_releases_listener() {
    let server = Server::start(test_config()).await;
    let address = server.address;
    let mut sockets = Vec::new();
    for _ in 0..8 {
        sockets.push(server.connect().await);
    }
    wait_active(&server.metrics, 8).await;
    server.stop().await;
    for mut socket in sockets {
        closed(&mut socket).await;
    }
    let rebound = TcpListener::bind(address).await.unwrap();
    drop(rebound);
}

#[tokio::test]
async fn truncated_body_releases_connection() {
    let server = Server::start(test_config()).await;
    let mut socket = server.connect().await;
    socket.write_all(&[100, 0]).await.unwrap();
    socket.shutdown().await.unwrap();
    closed(&mut socket).await;
    wait_active(&server.metrics, 0).await;
    assert_eq!(server.metrics.snapshot().peer_closes, 1);
    ping(&server).await;
    server.stop().await;
}

#[tokio::test]
async fn isolated_connection_panic_does_not_stop_the_listener() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let metrics = Arc::new(Metrics::default());
    let task_metrics = Arc::clone(&metrics);
    let (stop, receive_stop) = oneshot::channel();
    let task = tokio::spawn(smolmc_network::serve_with(
        listener,
        test_config(),
        task_metrics,
        async {
            let _ = receive_stop.await;
        },
        |mut connection, context| async move {
            assert_ne!(context.id, 1, "injected connection-owned failure");
            let packet = connection.inbound.read_packet().await?;
            connection.outbound.send(&packet).await
        },
    ));
    let mut first = TcpStream::connect(address).await.unwrap();
    closed(&mut first).await;
    timeout(Duration::from_secs(1), async {
        while metrics.snapshot().task_failures != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut second = TcpStream::connect(address).await.unwrap();
    second.write_all(&[1, 42]).await.unwrap();
    assert_eq!(receive(&mut second).await, [42]);
    stop.send(()).unwrap();
    task.await.unwrap().unwrap();
    assert_eq!(metrics.snapshot().active, 0);
    assert_eq!(metrics.snapshot().frame_bytes_retained, 0);
}
