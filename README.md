# SmolMC

Rust Minecraft Java Edition **26.3 / protocol 777**, implemented through **M4A imported arena terrain and interactions**:
isolated worlds sharing immutable stone/registries, sparse COW snapshots, cheap
clones/reset, profiles/budgets and event-driven idle lifecycle. See the
[M2B engineering report](docs/M2B_ENGINEERING_REPORT.md) for acceptance and measurements.

M2A was accepted after user-reported real stock-client verification. M2B retains
that protocol path and adds automated cloned-world isolation checks; no new stock
rendering or real-account capacity claim. Player block interactions remain
read-only on the legacy virtual-stone path; trusted controls retain buried
Air/Stone edits there. Imported maps support broader terrain edits through the
explicit `build` profile and typed controls.
With `--pvp on`, the authoritative arena kit supports inventory/hotbar/F swaps,
sword combat, health, death and respawn. Other survival actions and signed commands
remain unsupported; see the M3/M4A reports for exact limits. The optional bridge supports bounded unsigned
literal commands. No signed-player-chat verification is claimed.

**M4A is implemented and user-accepted following real 26.3 client tests.**
Bounded modern Anvil import, shared immutable terrain/COW instances, supported
collisions, placement and held mining are available with explicit imported-map
profiles. See the [M4A report](docs/M4A_ENGINEERING_REPORT.md),
[exact supported subset and limits](docs/M4A_TERRAIN_CONTRACT.md),
[protocol evidence](docs/M4A_PROTOCOL_EVIDENCE.md) and
[security review](docs/M4A_SECURITY_REVIEW.md). Full survival remains deferred.

Revised M3 was previously user-verified with two stock 26.3 clients.
The accepted knockback profile follows 1.19.4-based behavior with the documented
flat-arena contact policy; wire protocol remains 777.
See the [M3 engineering report](docs/M3_ENGINEERING_REPORT.md),
[protocol/gameplay evidence](docs/M3_PROTOCOL_EVIDENCE.md) and
[security review](docs/M3_SECURITY_REVIEW.md). M4A preserves this combat profile; stop after M4A.
See [AGENTS.md](AGENTS.md), [the roadmap](docs/13_ROADMAP.md),
[ADR-006](decisions/ADR-006-m2b-shared-cow-overlays-and-budgets.md) and the
[accepted M2A checkpoint/report](docs/M2A_ENGINEERING_REPORT.md).

A focused replication optimization followed M2B: bounded write batching and wake/
allocation reductions lower CPU while preserving update rates and isolation.
See [short before/after measurements and limits](docs/REPLICATION_CPU_REPORT.md)
and [ADR-007](decisions/ADR-007-bounded-replication-write-batches.md). These optimizations
remain intact through M4A.

## Build and run

Rust **1.99.1** is pinned. System OpenSSL development libraries are required
(M1D tested with Homebrew OpenSSL 4.0.3 on macOS and Alpine OpenSSL 3.5.9
on Linux).
On Alpine, prerequisites are `build-base pkgconf openssl-dev`; the tested musl
build uses `RUSTFLAGS='-C target-feature=-crt-static'` for system OpenSSL linking.

```sh
cargo build --release --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
target/release/smolmc
```

The existing project-local toolchain can be used with `scripts/cargo-local.sh`
in place of `cargo`; it does not modify shell profiles. `Cargo.lock` pins the
dependency graph. The Rust server never launches or requires a JVM. An explicitly started Java
application can launch/attach through the optional M2C local control bridge.

Add **127.0.0.1:25565** in the stock 26.3 client's Multiplayer list. Status and
ping work. Joining requires the client's legitimate signed-in account. There
is no offline mode, proxy identity forwarding, authentication fallback, or
credential/token input on the server command line.

```sh
target/release/smolmc --bind 127.0.0.1:25565 --trace-connections
target/release/smolmc --instances 2 --world-profile read-only --bind 127.0.0.1:25565
target/release/smolmc --pvp on --arena-spawn 8,64,8 --bind 127.0.0.1:25566
target/release/smolmc --import-world '/path/to/world' --import-bounds '-16,-64,-80,48,320,-16' --world-profile build --pvp on --bind 127.0.0.1:25567
target/release/smolmc --help
```

`--bind` accepts IP:port, including `[::1]:25565`; port `0` chooses a free port
printed in the ready log. Default binding is IPv4 loopback. Ctrl-C/SIGTERM closes
and joins connection tasks, waits for bounded RSA work, checks resource leases,
and prints counters. Opt-in connection tracing logs IDs and phase/packet metadata,
never keys, tokens or payloads. Keep tracing off for ordinary load measurements.

## Optional Java SDK

Java **17+**, SDK **0.3.0**, Unix socket schema **1.2**; old 0.1.0/1.0 and 0.2.0/1.1
clients retain their operation/event subset. Extensions add combat observations,
configured-template discovery and typed supported terrain reads/atomic edits. Bridge-off starts no JVM,
listener, event bus or per-player Java queue. Enabling the bridge adds no Rust
thread or world tick. Java remains a separate process; its memory is reported
separately (not hidden in Rust RSS). No Bukkit/Paper plugin compatibility or plugin
loader is implemented.

```sh
# The directory must be new; default access is observe. Enable control explicitly.
target/release/smolmc --bridge-dir /tmp/smolmc-ipc-new --bridge-access control
scripts/java-check.sh
java -cp target/java/classes TinyArena target/release/smolmc
```

[Typed SDK source](java/sdk/src/main/java/dev/smolmc/MinecraftRustServer.java),
[tiny guessing-game example](java/examples/TinyArena.java),
[wire/API/security/backpressure contract](docs/M2C_BRIDGE_CONTRACT.md),
[ADR-008](decisions/ADR-008-m2c-optional-java-ipc.md) and
[measurements/regressions/rollback](docs/M2C_ENGINEERING_REPORT.md).
The example launches its own Rust child on a free loopback port; read its private
log directory for the game address. `/arena guess N` wins a round by guessing 1..10
and moves the winner to a separate lobby. `/arena a` rejoins. Worlds reset only
when empty; buried edits and fixed spawn transfers retain existing M2B limits.

## Responsibilities and limits

| Crate | Responsibility |
|---|---|
| `protocol-777` | Pure bounded field codecs and packet serialization |
| `network` | Admission/recovery, persistent read/write halves, framing/transforms, byte budgets and supervision |
| `auth` | Provider-backed challenge cryptography and Mojang HTTPS session verification |
| `world` | Materials/geometry plus explicit template/instance identities, bounded edits and lifecycle |
| `chunks` | Shared stone/imported chunk packets, palettes, heightmaps and light updates |
| `sessions` | Protocol phases, identity/world leases, trusted lifecycle controls, shared terrain, per-world replication and switching |
| `bridge` | Optional versioned authenticated Unix IPC, bounded control requests and events |
| `server` | Thin assembly root, runtime limits, CLI and shutdown |

A lifecycle manager owns distinct worlds over the shared immutable stone template.
The bootstrap dimension key is wire metadata, separate from WorldId. Sessions own
connection controls/subscriptions and RAII world membership; the directory scopes
visibility/chat by world. No per-world task, timer, thread or simulation loop.
`--instances N` (1–4096, default 1) routes joins round-robin across identical arenas.
Trusted Rust Service controls create/clone/batch-edit/switch/park/reset/destroy worlds;
occupied reset/destroy are rejected. Reset reuses identity with a new generation.
`--world-profile editable|read-only` defaults to editable, preserving M2A's trusted
buried edits. Required protocol/presence/chat capabilities remain on; unavailable
general physics/mob/persistence capabilities fail explicitly. PvP is a separate
startup opt-in; the optional local Java control bridge remains independent. Parked worlds own no
terrain watch channel; terrain watches are created only while occupied. An authenticated local control listener and literal commands are available only
with the explicitly enabled bridge.
Legacy virtual-stone edits remain ephemeral buried Air/Stone overrides only.
Imported edits cover their selected volume, with supported states and atomic light
updates. Both retain sparse COW and per-world/shared budgets. No persistence.

| Resource | Current default/policy |
|---|---|
| Connections / authenticated session slots | 512 / 512; safety budgets, not measured gameplay capacity |
| Connection admission | 256/s global, 32/s/IP, fixed one-second windows; at most 1024 tracked IPs/window |
| Shared frame reservations | 16 MiB; leases follow packets/pending writes |
| Inbound frames | 1 KiB before authentication; 64 KiB control phase |
| Outbound packets | 256 KiB; one pending transformed write per writer; replication batches ≤64 packets / 16 KiB plaintext payload |
| Inbound work | 128 KiB/s per connection before 8 MiB/s shared; wire bodies plus decoded output |
| Compression | Threshold 256; decoded-size/ratio (128)/exact-stream-length checks |
| Authentication | 8 concurrent challenges/verifications; 2 admitted RSA jobs |
| Runtime | One async thread; at most 4 blocking threads, retired after 2 seconds idle |
| Deadlines | Handshake/status 5s; Login 15s; Configuration 20s; Play writes 5s; teleport/batch ACK 10s |
| Persistent Play | Keepalive after 12s, response within 20s; at most 256 inbound packets/s/session |
| Terrain streaming | Radius 2 (25 slots), one batch in flight, ≤4 chunks/batch; shared encoded imported packets; temporary scratch released after imported view completes |
| Presence | At most 511 peers/viewer; latest snapshots/rosters, 50ms minimum dirty publish/flush |
| Chat | System relay only; 3-message sender burst, refill 1/s; 8192 deliveries/s aggregate; 32-message viewer queue |
| Directory | 2048 command slots; coalesced update notices, reserved generation-checked leave |
| Worlds | 4096 live worlds, 16 template descriptors; no per-world task/timer/thread |
| World profile | Up to 512 players, 65,536 overrides, 4096 modified chunks and 1 MiB logical block+light overlay charges/world |
| Retained overlay snapshots | 64 MiB shared charge budget, including pinned old snapshots; charges are payload/header policy accounting, not RSS |
| Edits | ≤4096 operations/transaction; unchanged block/light snapshots shared by clones; imported lighting uses one bounded shared worker |
| Session service | Fixed HTTPS origin; connect 3s/request 8s; response 64 KiB; no redirects/proxy routing |

Admission can be configured with `--max-connections N` (up to 4096),
`--connection-rate N` and `--per-ip-rate N` (up to 10000). Other limits remain
the bounded transport profile described in the M1D report; M2B world limits are
listed above and exposed by trusted Rust profile/resource controls. Excess work fails within the connection. Recoverable
accept errors back off without evicting existing peers.

Only the acknowledged **minecraft:core:26.3** known-pack path is supported.
Declining it produces a readable Configuration rejection. The client supplies
its own built-in registry data. The repository contains identifier inventories
only: **32 registries, 432 entries, 773 named tags**. Tag memberships are explicitly
empty for the legacy bootstrap; imported clients receive four supported mining-tag
subsets. Narrow PvP rules explicitly identify supported swords/tools. General vanilla tag/survival semantics are not implemented.
No Mojang registry/tag JSON/NBT assets or implementation bodies are bundled.

## Verification and reports

- [M4A: imported maps, collisions, held mining, performance and rollback](docs/M4A_ENGINEERING_REPORT.md)
- [M4A supported subset and bounds](docs/M4A_TERRAIN_CONTRACT.md)

- [Revised M3: authoritative arena combat, client acceptance, resources, limits and rollback](docs/M3_ENGINEERING_REPORT.md)

- [M2B report: 84 tests on macOS/Linux, empty/modified/COW resource measurements, 34,000 lifecycle cycles, limits and Git rollback](docs/M2B_ENGINEERING_REPORT.md)
- [M2B protocol evidence](docs/M2B_PROTOCOL_EVIDENCE.md) and [ADR-006](decisions/ADR-006-m2b-shared-cow-overlays-and-budgets.md)

- [Accepted M2A: 76 tests, user client verification, four-client isolation/switch and historical measurements](docs/M2A_ENGINEERING_REPORT.md)
- [M2A protocol evidence](docs/M2A_PROTOCOL_EVIDENCE.md) and [ADR-005](decisions/ADR-005-m2a-explicit-instance-ownership.md)

- [M1D engineering report: client checks, 65 tests on macOS/Linux, active workloads, limits and rollback](docs/M1D_ENGINEERING_REPORT.md)
- [M1C safety review](docs/M1D_M1C_REVIEW.md), [M1D protocol evidence](docs/M1D_PROTOCOL_EVIDENCE.md) and [ADR-004](decisions/ADR-004-m1d-presence-and-system-relay.md)
- [M1C engineering report: client verification, tests, measurements, limits and rollback](docs/M1C_ENGINEERING_REPORT.md)
- [M1C packet evidence](docs/M1C_PROTOCOL_EVIDENCE.md) and [immutable stream decision](decisions/ADR-003-m1c-immutable-stone-stream.md)
- [M1B engineering report: acceptance, measurements, limits and rollback](docs/M1B_ENGINEERING_REPORT.md)
- [Packet evidence and policy differences](docs/M1B_PROTOCOL_EVIDENCE.md)
- [Architectural decision](decisions/ADR-002-m1b-transport-authentication.md)
- [Historical M1A report](docs/STAGE1_REPORT.md)

```sh
python3 scripts/check_listener_recovery.py --output evidence/m1d-listener-recovery.json
python3 scripts/measure_m1b.py --output evidence/m1d-connection-resources.json
cargo run --release --example profile -p smolmc-chunks --locked
```

The listener regression uses a child descriptor limit on the tested Unix hosts.
The existing M1B-named resource sampler also works against M1D and uses macOS
`ps`. It explicitly raises admission rates for
a single-IP generator, exercises 512 pending connections, 1000 status reconnects
and failed authentication, and never simulates successful account authentication.
The real-account client check is recorded separately. Active Play and repeated
active churn use private release test fixtures in separate client/server processes;
see the M1D report for commands and precise workload definitions. The production
binary cannot enable those fixture paths. At 256 densely colocated moving fixtures,
the single core is nearly saturated; the 512 admission limit is not a gameplay
capacity promise. Raw evidence is in `evidence/`.

Private `unz/`, development tools in `.tools/`, and client logs are excluded by
`.gitignore`. Research-pack documents are historical proposals; root authorization
and milestone reports describe implemented scope.
