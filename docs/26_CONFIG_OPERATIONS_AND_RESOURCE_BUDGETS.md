# 26 — Deployment ergonomics, capacity policy and explicit feature toggles

## Operator mental model
The server runs as one Rust executable hosting many independent world instances. An operator chooses a global profile and per-template/instance overrides. Optional plugin hosts are separate processes, starting only when configured. A world can be read-only without block mutation/physics workers.

### Illustrative, NON-FUNCTIONAL configuration sketch
This shows the **desired configuration experience**, not parser implementation or committed syntax.

```toml
# proposal only; not a real server config
[server]
protocol = 777
authentication = "online"
max_players = 100

[profiles.minimal_creative]
world_template = "stone_flat"
creative = true
flight = true
block_editing = false
mob_ai = false
redstone = false
random_ticks = false
persistence = "none"
java_bridge = false

[resources]
max_loaded_instances = 1000
world_overlay_bytes_limit = "configure_after_benchmark"
max_connection_queue_bytes = "configure_after_benchmark"

[java]
enabled = false
```

A configuration containing future unimplemented capabilities must fail clearly when enabled, not quietly claim they are working. Illustrative numbers are **example inputs**, not achieved capacities or safe operational defaults.

## Resource governance
**Global:** maximum sessions, aggregate send queue bytes, connection acceptance rate, background worker concurrency, template cache bytes, total active worlds, Java sidecar count. **Per world:** active sessions, loaded chunks, retained overlay bytes, entity count, queued mutation bytes, scheduled jobs, event callback budget, storage quota. **Per connection:** decompressed packet size, outbound queue, inbound packet rate, chunk subscriptions, login deadline, keepalive health. **Per Java plugin:** event subscriptions, request queue, CPU time if measured, message bytes, granted operations, outstanding futures.

## Behavior at limits
- Resource budget violations return typed errors; don't silently allocate.
- Player join capacity: explain/full status rather than entering half-joined state.
- Chunk queue grows: throttle optional cosmetic or distance updates first, preserve essentials or disconnect pathological client.
- World overlay reaches cap: reject further edits, optionally request controlled snapshot/persist if supported; do not mutate shared template.
- A plugin floods the bridge: throttle/revoke plugin; gameplay should remain responsive.
- Idle instances can remain logically registered; loaded immutable template cache is governed separately.

## Shutdown and crash recovery
Rust core shutdown should stop new ingress, notify/relocate players per policy, terminate Java event intake, finish/abort world work by declared deadline, flush durable storage only if enabled, and report abandoned work. A crash loses ephemeral world overlays: this must be documented. A sidecar crash does not kill Rust gameplay unless an operator has deliberately made plugin-dependent functionality critical.

## Packaging plans (not implemented)
- No-Java native distribution by default (single platform-appropriate Rust binary **after coding authorized**).
- Java SDK distribution as JAR + optional plugin host (separate download or bundle, only run if enabled).
- Registry/protocol version assets with license checks; do not ship Mojang archive.
- Docker/container, systemd, Windows service, macOS developer launch guides **later**; first demonstrate on Linux and macOS.
- Management API local-only by default, authenticated and not accessible from public game port.

## Operator UX
Feature inspector should explain: "feature disabled by profile", "compiled out", "unavailable because dependency", "requires restart", "loaded memory", "active worker count". 'World idle' should report parked/timers remaining, not simply TPS=0. Observability must be low overhead by default with deeper optional profiling.
