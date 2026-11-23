# Running SmolMC

## Build and run

Rust **1.99.1** is pinned in the repository. Install system OpenSSL development libraries and pkg-config.
On Debian or Ubuntu use `build-essential pkg-config libssl-dev`. On macOS
with Homebrew use `pkgconf openssl@3`.
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
target/release/smolmc --import-world '/path/to/world' --import-bounds '-16,-64,-80,48,320,-16' --world-profile build --pvp on --inventory-policy items-keep --bind 127.0.0.1:25568
target/release/smolmc --help
```

`--bind` accepts IP:port, including `[::1]:25565`; port `0` chooses a free port
printed in the ready log. Default binding is IPv4 loopback. Ctrl-C/SIGTERM closes
and joins connection tasks, waits for bounded RSA work, checks resource leases,
and prints counters. Opt-in connection tracing logs IDs and phase/packet metadata,
never keys, tokens or payloads. Keep tracing off for ordinary load measurements.

## Optional Java SDK

Java **17+**, SDK **0.4.0**, Unix socket schema **1.3**; old 0.1.0/1.0, 0.2.0/1.1
and 0.3.0/1.2 clients retain their operation/event subset. Extensions add immutable
inventory/count/durability/armor snapshots and combat observations,
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

[Typed SDK source](../java/sdk/src/main/java/dev/smolmc/MinecraftRustServer.java),
[tiny guessing-game example](../java/examples/TinyArena.java),
[wire/API/security/backpressure contract](../docs/M2C_BRIDGE_CONTRACT.md),
[ADR-008](../decisions/ADR-008-m2c-optional-java-ipc.md) and
[measurements/regressions/rollback](../docs/M2C_ENGINEERING_REPORT.md).
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
| Item gameplay | Explicit policy; 1024 entities/world, 8192 shared; configurable up to 4096/world and 32768 shared; 256 visible/viewer |
| Reliable gameplay payloads | 128 KiB logical queued charge/viewer and 8 MiB shared; existing 32-message queue; frame/write budgets separate |
| Interaction pipeline | 16 waiting inputs/player, eight finite delayed records/player, bounded lighting retries; shared scheduler sleeps when no work remains |
| Item reconnect cache | 1024 identities in process; no eviction/repair or disk persistence |

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
