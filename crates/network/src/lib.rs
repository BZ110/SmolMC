//! Bounded connection ownership. The supervisor is the only admission writer;
//! each task owns one socket and cannot mutate world or extension state.

pub mod transport;

use smolmc_protocol_777::{self as wire, Intent};
use std::{
    collections::HashMap,
    future::Future,
    io,
    net::IpAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed},
    },
    time::{Duration, Instant},
};
use tokio::{net::TcpListener, task::JoinSet, time::timeout};

#[derive(Debug, Clone)]
pub struct Config {
    pub max_connections: usize,
    /// Absolute budget for handshake + status + ping + all writes (slowloris bound).
    pub exchange_timeout: Duration,
    pub accepts_per_second: usize,
    pub accepts_per_ip_per_second: usize,
    pub trace_connections: bool,
    pub frame_bytes: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_connections: 128,
            exchange_timeout: Duration::from_secs(5),
            accepts_per_second: 256,
            accepts_per_ip_per_second: 32,
            trace_connections: false,
            frame_bytes: 16 * 1024 * 1024,
        }
    }
}

impl Config {
    fn validate(&self) -> io::Result<()> {
        if !(1..=4096).contains(&self.max_connections)
            || self.exchange_timeout.is_zero()
            || self.exchange_timeout > Duration::from_secs(60)
            || self.accepts_per_second == 0
            || self.accepts_per_ip_per_second == 0
            || !(64 * 1024..=1024 * 1024 * 1024).contains(&self.frame_bytes)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid network limits",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct Metrics {
    accepted: AtomicU64,
    rejected: AtomicU64,
    active: AtomicUsize,
    peak_active: AtomicUsize,
    status_responses: AtomicU64,
    pongs: AtomicU64,
    login_rejections: AtomicU64,
    protocol_errors: AtomicU64,
    timeouts: AtomicU64,
    accept_errors: AtomicU64,
    task_failures: AtomicU64,
    peak_frame_reservation: AtomicUsize,
    frame_bytes_retained: AtomicUsize,
    peer_closes: AtomicU64,
    io_errors: AtomicU64,
}

#[derive(Debug, Clone, Copy)]
pub struct Snapshot {
    pub accepted: u64,
    pub rejected: u64,
    pub active: usize,
    pub peak_active: usize,
    pub status_responses: u64,
    pub pongs: u64,
    pub login_rejections: u64,
    pub protocol_errors: u64,
    pub timeouts: u64,
    pub accept_errors: u64,
    pub task_failures: u64,
    pub peak_frame_reservation: usize,
    pub frame_bytes_retained: usize,
    pub peer_closes: u64,
    pub io_errors: u64,
}

impl Metrics {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            accepted: self.accepted.load(Relaxed),
            rejected: self.rejected.load(Relaxed),
            active: self.active.load(Relaxed),
            peak_active: self.peak_active.load(Relaxed),
            status_responses: self.status_responses.load(Relaxed),
            pongs: self.pongs.load(Relaxed),
            login_rejections: self.login_rejections.load(Relaxed),
            protocol_errors: self.protocol_errors.load(Relaxed),
            timeouts: self.timeouts.load(Relaxed),
            accept_errors: self.accept_errors.load(Relaxed),
            task_failures: self.task_failures.load(Relaxed),
            peak_frame_reservation: self.peak_frame_reservation.load(Relaxed),
            frame_bytes_retained: self.frame_bytes_retained.load(Relaxed),
            peer_closes: self.peer_closes.load(Relaxed),
            io_errors: self.io_errors.load(Relaxed),
        }
    }
}

struct ConnectionGuard(Arc<Metrics>);

impl ConnectionGuard {
    fn new(metrics: Arc<Metrics>) -> Self {
        let active = metrics.active.fetch_add(1, Relaxed) + 1;
        metrics.peak_active.fetch_max(active, Relaxed);
        Self(metrics)
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Relaxed);
    }
}

/// Fixed one-second admission windows. No idle timer and at most 1024 IP keys.
struct Admission {
    started: Instant,
    total: usize,
    peers: HashMap<IpAddr, usize>,
}

impl Admission {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            total: 0,
            peers: HashMap::new(),
        }
    }

    fn allow(&mut self, ip: IpAddr, config: &Config) -> bool {
        if self.started.elapsed() >= Duration::from_secs(1) {
            self.started = Instant::now();
            self.total = 0;
            self.peers.clear();
        }
        if self.total >= config.accepts_per_second
            || (!self.peers.contains_key(&ip) && self.peers.len() >= 1024)
        {
            return false;
        }
        let count = self.peers.entry(ip).or_default();
        if *count >= config.accepts_per_ip_per_second {
            return false;
        }
        *count += 1;
        self.total += 1;
        true
    }
}

fn invalid(error: wire::ProtocolError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

async fn exchange(
    socket: &mut transport::Connection,
    metrics: &Metrics,
    id: u64,
    trace: bool,
) -> io::Result<()> {
    let hello = wire::decode_handshake(&socket.inbound.read_packet().await?).map_err(invalid)?;
    if trace {
        eprintln!("connection={id} transition=handshake->{:?}", hello.intent);
    }
    match hello.intent {
        Intent::Login | Intent::Transfer => {
            // This is a terminal rejection, not an implemented login stage.
            socket
                .outbound
                .send(&wire::login_rejection(hello.protocol))
                .await?;
            metrics.login_rejections.fetch_add(1, Relaxed);
        }
        Intent::Status => {
            // Status deliberately accepts other version numbers so the server list
            // can show the target version. This does not imply multi-version play.
            wire::decode_status_request(&socket.inbound.read_packet().await?).map_err(invalid)?;
            socket.outbound.send(&wire::status_response()).await?;
            metrics.status_responses.fetch_add(1, Relaxed);
            let nonce = wire::decode_ping(&socket.inbound.read_packet().await?).map_err(invalid)?;
            socket.outbound.send(&wire::pong(nonce)).await?;
            metrics.pongs.fetch_add(1, Relaxed);
        }
    }
    socket.outbound.shutdown().await
}

#[derive(Clone)]
pub struct ConnectionContext {
    pub id: u64,
    pub trace: bool,
    metrics: Arc<Metrics>,
}
impl ConnectionContext {
    pub fn status_sent(&self) {
        self.metrics.status_responses.fetch_add(1, Relaxed);
    }
    pub fn pong_sent(&self) {
        self.metrics.pongs.fetch_add(1, Relaxed);
    }
    pub fn login_rejected(&self) {
        self.metrics.login_rejections.fetch_add(1, Relaxed);
    }
}

/// Runs until shutdown resolves. All connection tasks are cancelled and joined
/// before return; listener and sockets are owned here, with no detached tasks.
pub async fn serve(
    listener: TcpListener,
    config: Config,
    metrics: Arc<Metrics>,
    shutdown: impl Future<Output = ()>,
) -> io::Result<()> {
    let budget = config.exchange_timeout;
    serve_with(
        listener,
        config,
        metrics,
        shutdown,
        move |mut connection, context| async move {
            timeout(
                budget,
                exchange(&mut connection, &context.metrics, context.id, context.trace),
            )
            .await
            .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?
        },
    )
    .await
}

/// The session handler is injected; networking has no authentication, player or
/// world ownership. Handler futures and their socket halves remain supervised.
pub async fn serve_with<F, Fut>(
    listener: TcpListener,
    config: Config,
    metrics: Arc<Metrics>,
    shutdown: impl Future<Output = ()>,
    handler: F,
) -> io::Result<()>
where
    F: Fn(transport::Connection, ConnectionContext) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = io::Result<()>> + Send + 'static,
{
    config.validate()?;
    let frame_budget = transport::ByteBudget::new(config.frame_bytes);
    let handler = Arc::new(handler);
    let mut tasks = JoinSet::new();
    let mut admission = Admission::new();
    let mut next_id = 0u64;
    let mut accept_after = None;
    let mut accept_backoff = Duration::from_millis(25);
    tokio::pin!(shutdown);
    let result = loop {
        tokio::select! {
            biased;
            _ = &mut shutdown => break Ok(()),
            joined = tasks.join_next(), if !tasks.is_empty() => {
                if let Some(Err(error)) = joined {
                    metrics.task_failures.fetch_add(1, Relaxed);
                    if config.trace_connections { eprintln!("connection_task_failed cancelled={}", error.is_cancelled()); }
                }
            }
            accepted = async {
                if let Some(deadline) = accept_after { tokio::time::sleep_until(deadline).await; }
                listener.accept().await
            } => {
                let (socket, address) = match accepted {
                    Ok(pair) => pair,
                    Err(error) if recoverable_accept_error(&error) => {
                        metrics.accept_errors.fetch_add(1, Relaxed);
                        accept_after = Some(tokio::time::Instant::now() + accept_backoff);
                        accept_backoff = (accept_backoff * 2).min(Duration::from_secs(1));
                        continue;
                    }
                    Err(error) => break Err(error),
                };
                accept_backoff = Duration::from_millis(25);
                accept_after = None;
                if tasks.len() >= config.max_connections || !admission.allow(address.ip(), &config) {
                    metrics.rejected.fetch_add(1, Relaxed);
                    continue;
                }
                next_id = match next_id.checked_add(1) {
                    Some(id) => id,
                    None => break Err(io::Error::other("connection ID exhausted")),
                };
                let id = next_id;
                metrics.accepted.fetch_add(1, Relaxed);
                let guard = ConnectionGuard::new(Arc::clone(&metrics));
                let task_metrics = Arc::clone(&metrics);
                let trace = config.trace_connections;
                let handler = Arc::clone(&handler);
                let connection = transport::Connection::new(socket, frame_budget.clone());
                let context = ConnectionContext { id, trace, metrics: Arc::clone(&task_metrics) };
                tasks.spawn(async move {
                    let _guard = guard;
                    if trace { eprintln!("connection={id} state=handshake"); }
                    let outcome = handler(connection, context).await;
                    let reason = match outcome {
                        Ok(()) => "complete",
                        Err(error) => match error.kind() {
                            io::ErrorKind::TimedOut => { task_metrics.timeouts.fetch_add(1, Relaxed); "timeout" }
                            io::ErrorKind::InvalidData => { task_metrics.protocol_errors.fetch_add(1, Relaxed); "invalid_packet" }
                            io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe => {
                                task_metrics.peer_closes.fetch_add(1, Relaxed); "peer_closed"
                            }
                            _ => { task_metrics.io_errors.fetch_add(1, Relaxed); "io_error" }
                        },
                    };
                    if trace { eprintln!("connection={id} state=closed reason={reason}"); }
                });
            }
        }
    };
    drop(listener);
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    metrics
        .peak_frame_reservation
        .store(frame_budget.peak_reserved(), Relaxed);
    let retained = config.frame_bytes - frame_budget.available();
    metrics.frame_bytes_retained.store(retained, Relaxed);
    if retained != 0 {
        return Err(io::Error::other(
            "connection frame leases remain after shutdown",
        ));
    }
    result
}

fn recoverable_accept_error(error: &io::Error) -> bool {
    if matches!(
        error.kind(),
        io::ErrorKind::Interrupted
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::WouldBlock
    ) {
        return true;
    }
    // OS resource pressure can recover while existing connections continue.
    #[cfg(unix)]
    if let Some(code) = error.raw_os_error() {
        return [libc::EMFILE, libc::ENFILE, libc::ENOBUFS, libc::ENOMEM].contains(&code);
    }
    #[cfg(windows)]
    if let Some(code) = error.raw_os_error() {
        return [10024, 10055].contains(&code); // WSAEMFILE, WSAENOBUFS
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[test]
    fn admission_is_bounded_and_limits_each_peer() {
        let config = Config {
            accepts_per_second: 3,
            accepts_per_ip_per_second: 2,
            ..Config::default()
        };
        let mut gate = Admission::new();
        let a = "127.0.0.1".parse().unwrap();
        let b = "127.0.0.2".parse().unwrap();
        assert!(gate.allow(a, &config));
        assert!(gate.allow(a, &config));
        assert!(!gate.allow(a, &config));
        assert!(gate.allow(b, &config));
        assert!(!gate.allow(b, &config));
        gate.started = Instant::now() - Duration::from_secs(2);
        assert!(gate.allow(a, &config));
        assert_eq!(gate.total, 1);
        assert_eq!(gate.peers.len(), 1);
        let large = Config {
            accepts_per_second: 10_000,
            ..Config::default()
        };
        let mut gate = Admission::new();
        for index in 0..1024u32 {
            assert!(gate.allow(IpAddr::V4(index.into()), &large));
        }
        assert!(!gate.allow(IpAddr::V4(1024u32.into()), &large));
        assert_eq!(gate.peers.len(), 1024);
    }

    #[tokio::test]
    async fn framing_rejects_length_before_waiting_for_body() {
        for bytes in [&[0u8][..], &[0x81, 8][..], &[0x80, 0x80, 0x80][..]] {
            let (mut writer, reader) = tokio::io::duplex(16);
            writer.write_all(bytes).await.unwrap();
            let mut input =
                transport::Inbound::new(reader, transport::ByteBudget::new(1024 * 1024));
            let error = timeout(Duration::from_millis(100), input.read_packet())
                .await
                .unwrap()
                .err()
                .unwrap();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        }
    }

    #[tokio::test]
    async fn invalid_config_is_rejected() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let metrics = Arc::new(Metrics::default());
        let config = Config {
            max_connections: 0,
            ..Config::default()
        };
        assert!(
            serve(listener, config, metrics, std::future::pending())
                .await
                .is_err()
        );
    }
}
