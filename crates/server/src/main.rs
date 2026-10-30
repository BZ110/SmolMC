//! Thin composition root: bounded networking, sessions and explicit worlds; no JVM.
use smolmc_network::{Config, Metrics, serve_with};
use std::{io, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        tokio::select! {
            result = tokio::signal::ctrl_c() => result.expect("install Ctrl-C handler"),
            _ = term.recv() => {},
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c()
        .await
        .expect("install Ctrl-C handler");
}

fn main() -> io::Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(4)
        .thread_keep_alive(std::time::Duration::from_secs(2))
        .build()?
        .block_on(run())
}
async fn run() -> io::Result<()> {
    let mut bind: SocketAddr = "127.0.0.1:25565".parse().unwrap();
    let mut config = Config {
        max_connections: 512,
        ..Config::default()
    };
    let mut instances = 1usize;
    let mut import_path = None;
    let mut convert = false;
    let mut import_bounds = None;
    let mut explicit_spawn = false;
    let mut pvp = smolmc_sessions::gameplay::Config::default();
    let mut bridge_dir = None;
    let mut bridge_access = smolmc_bridge::Access::Observe;
    let mut world_profile = smolmc_sessions::WorldProfile::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pvp" => {
                pvp.enabled = match args.next().as_deref() {
                    Some("on") => true,
                    Some("off") => false,
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "--pvp needs on or off",
                        ));
                    }
                };
            }
            "--import-remove-unsupported" => convert = true,
            "--import-world" => {
                import_path = Some(std::path::PathBuf::from(args.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--import-world needs a world folder",
                    )
                })?));
            }
            "--import-bounds" => {
                let values = args.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--import-bounds needs minx,miny,minz,maxx,maxy,maxz",
                    )
                })?;
                let n = values
                    .split(',')
                    .map(str::parse::<i32>)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidInput, "invalid import bounds")
                    })?;
                if n.len() != 6 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "import bounds need six integers",
                    ));
                }
                import_bounds = Some(
                    smolmc_sessions::ImportBounds {
                        min: [n[0], n[1], n[2]],
                        max: [n[3], n[4], n[5]],
                    }
                    .validate()?,
                );
            }
            "--arena-spawn" => {
                explicit_spawn = true;
                let text = args.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--arena-spawn needs x,y,z")
                })?;
                let xyz: Vec<f64> = text
                    .split(',')
                    .map(str::parse)
                    .collect::<Result<_, _>>()
                    .map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidInput, "invalid arena spawn")
                    })?;
                pvp.spawn = xyz.try_into().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "spawn needs three coordinates")
                })?;
            }
            "--bridge-dir" => {
                bridge_dir = Some(std::path::PathBuf::from(args.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--bridge-dir needs a new private directory path",
                    )
                })?));
            }
            "--bridge-access" => {
                bridge_access = match args.next().as_deref() {
                    Some("observe") => smolmc_bridge::Access::Observe,
                    Some("control") => smolmc_bridge::Access::Control,
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "--bridge-access needs observe or control",
                        ));
                    }
                };
            }
            "--bind" => {
                bind = args
                    .next()
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "--bind needs an IP:port")
                    })?
                    .parse()
                    .map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidInput, "invalid --bind IP:port")
                    })?;
            }
            "--instances" => {
                instances = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .filter(|n| (1..=4096).contains(n))
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "--instances needs 1..4096")
                    })?;
            }
            "--world-profile" => {
                world_profile = match args.next().as_deref() {
                    Some("editable") => smolmc_sessions::WorldProfile::editable(),
                    Some("build") => smolmc_sessions::WorldProfile::builder(),
                    Some("read-only") => smolmc_sessions::WorldProfile::read_only(),
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "--world-profile needs editable, build or read-only",
                        ));
                    }
                };
            }
            "--trace-connections" => config.trace_connections = true,
            "--max-connections" | "--connection-rate" | "--per-ip-rate" => {
                let value: usize = args
                    .next()
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "resource option needs a positive integer",
                        )
                    })?
                    .parse()
                    .map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidInput, "invalid resource limit")
                    })?;
                if value == 0 || value > 10000 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "resource limit must be 1..10000",
                    ));
                }
                match arg.as_str() {
                    "--max-connections" => {
                        if value > 4096 {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "max connections must be <=4096",
                            ));
                        }
                        config.max_connections = value;
                    }
                    "--connection-rate" => config.accepts_per_second = value,
                    _ => config.accepts_per_ip_per_second = value,
                }
            }
            "--help" | "-h" => {
                println!(
                    "smolmc [--pvp on|off] [--arena-spawn X,Y,Z] [--import-world PATH --import-bounds MIN_X,MIN_Y,MIN_Z,MAX_X,MAX_Y,MAX_Z] [--import-remove-unsupported] [--bridge-dir NEW_PATH] [--bridge-access observe|control] [--bind IP:PORT] [--instances N] [--world-profile build|editable|read-only] [--trace-connections] [--max-connections N] [--connection-rate N] [--per-ip-rate N]\nM4A: Minecraft 26.3 protocol 777, online login, isolated arenas, optional PvP, bounded imported terrain and build interactions. Optional Java control bridge.\nJoins route round-robin across N isolated arenas (default 1). Imports are strict unless unsupported-state removal is explicitly enabled. Mining drops and durability are deferred.\nDefaults: loopback 127.0.0.1:25565, 512 connections, 256 accepts/s, 32 accepts/IP/s. Ctrl-C or SIGTERM stops all connections."
                );
                return Ok(());
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unknown argument; use --help",
                ));
            }
        }
    }
    let service = if let Some(path) = import_path {
        let bounds = import_bounds.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "--import-world requires explicit --import-bounds",
            )
        })?;
        let spawn = explicit_spawn.then_some(pvp.spawn);
        // The listener is not bound yet. Bounded import/serialization runs outside the reactor.
        Arc::new(
            tokio::task::spawn_blocking(move || {
                smolmc_sessions::Service::import_instances(
                    instances,
                    world_profile,
                    &path,
                    bounds,
                    spawn,
                    convert,
                )
            })
            .await
            .map_err(io::Error::other)??,
        )
    } else {
        if import_bounds.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "--import-bounds requires --import-world",
            ));
        }
        Arc::new(smolmc_sessions::Service::with_profiled_instances(
            instances,
            world_profile,
        )?)
    };
    if !explicit_spawn {
        pvp.spawn = service.arena_spawn();
    }
    if bridge_dir.is_none() && bridge_access == smolmc_bridge::Access::Control {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--bridge-access requires --bridge-dir",
        ));
    }
    service.configure_pvp(pvp)?;
    let listener = TcpListener::bind(bind).await?;
    let bridge = bridge_dir
        .as_ref()
        .map(|directory| {
            smolmc_bridge::Bridge::start(Arc::clone(&service), directory, bridge_access)
        })
        .transpose()?;
    let metrics = Arc::new(Metrics::default());
    eprintln!(
        "ready address={} minecraft=26.3 protocol=777 stage=M4A-Terrain online=true max_connections={} registry_bytes={} chunk_template_bytes={}",
        listener.local_addr()?,
        config.max_connections,
        service.registry_bytes(),
        service.chunk_template_bytes()
    );
    let sessions = Arc::clone(&service);
    let result = serve_with(
        listener,
        config,
        Arc::clone(&metrics),
        shutdown_signal(),
        move |connection, context| {
            let sessions = Arc::clone(&sessions);
            async move { sessions.handle(connection, context).await }
        },
    )
    .await;
    let bridge_cleanup = match bridge {
        Some(bridge) => bridge.shutdown().await,
        None => Ok(()),
    };
    let cleanup = service.shutdown().await;
    eprintln!("stopped metrics={:?}", metrics.snapshot());
    eprintln!("stopped sessions={:?}", service.metrics.snapshot());
    result.and(bridge_cleanup).and(cleanup)
}
