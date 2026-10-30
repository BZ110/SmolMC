# SmolMC

Rust Minecraft Java Edition **26.3 / protocol 777**, implemented through **M2B**:
isolated worlds sharing immutable stone/registries, sparse COW snapshots, cheap
clones/reset, profiles/budgets and event-driven idle lifecycle. See the
[M2B engineering report](docs/M2B_ENGINEERING_REPORT.md) for acceptance and measurements.

M2A was accepted after user-reported real stock-client verification. M2B retains
that protocol path and adds automated cloned-world isolation checks; no new stock
rendering or real-account capacity claim. Player block interactions remain
read-only; trusted Rust controls can edit buried Air/Stone blocks only.
Inventory actions (including F/off-hand swap), commands and most gameplay are
unsupported and may disconnect. No signed-player-chat verification is claimed.

**M2B is implemented and validated. M2C and all later milestones are unauthorized.**
See [AGENTS.md](AGENTS.md), [the roadmap](docs/13_ROADMAP.md),
[ADR-006](decisions/ADR-006-m2b-shared-cow-overlays-and-budgets.md) and the
[accepted M2A checkpoint/report](docs/M2A_ENGINEERING_REPORT.md).

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
dependency graph. The server never launches or requires a JVM.

Add **127.0.0.1:25565** in the stock 26.3 client's Multiplayer list. Status and
ping work. Joining requires the client's legitimate signed-in account. There
is no offline mode, proxy identity forwarding, authentication fallback, or
credential/token input on the server command line.

```sh
target/release/smolmc --bind 127.0.0.1:25565 --trace-connections
target/release/smolmc --instances 2 --world-profile read-only --bind 127.0.0.1:25565
target/release/smolmc --help
```

`--bind` accepts IP:port, including `[::1]:25565`; port `0` chooses a free port
printed in the ready log. Default binding is IPv4 loopback. Ctrl-C/SIGTERM closes
and joins connection tasks, waits for bounded RSA work, checks resource leases,
and prints counters. Opt-in connection tracing logs IDs and phase/packet metadata,
never keys, tokens or payloads. Keep tracing off for ordinary load measurements.

## Responsibilities and limits

| Crate | Responsibility |
|---|---|
| `protocol-777` | Pure bounded field codecs and packet serialization |
| `network` | Admission/recovery, persistent read/write halves, framing/transforms, byte budgets and supervision |
| `auth` | Provider-backed challenge cryptography and Mojang HTTPS session verification |
| `world` | Materials/geometry plus explicit template/instance identities, bounded edits and lifecycle |
| `chunks` | Shared stone chunk body and coordinate serialization |
| `sessions` | Protocol phases, identity/world leases, trusted lifecycle controls, shared terrain, per-world replication and switching |
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
physics/mob/persistence/Java capabilities fail explicitly. Parked worlds own no
watch channel; terrain watches are created only while occupied. No network admin endpoint or player commands are added.
Edits are ephemeral Air/Stone overrides only at `-64 <= y < 63`, at most 65,536 per
world, preserving the intact roof/heightmaps/lighting. Surface editing is unsupported.

| Resource | Current default/policy |
|---|---|
| Connections / authenticated session slots | 512 / 512; safety budgets, not measured gameplay capacity |
| Connection admission | 256/s global, 32/s/IP, fixed one-second windows; at most 1024 tracked IPs/window |
| Shared frame reservations | 16 MiB; leases follow packets/pending writes |
| Inbound frames | 1 KiB before authentication; 64 KiB control phase |
| Outbound packets | 256 KiB; one pending frame per writer |
| Inbound work | 128 KiB/s per connection before 8 MiB/s shared; wire bodies plus decoded output |
| Compression | Threshold 256; decoded-size/ratio (128)/exact-stream-length checks |
| Authentication | 8 concurrent challenges/verifications; 2 admitted RSA jobs |
| Runtime | One async thread; at most 4 blocking threads, retired after 2 seconds idle |
| Deadlines | Handshake/status 5s; Login 15s; Configuration 20s; Play writes 5s; teleport/batch ACK 10s |
| Persistent Play | Keepalive after 12s, response within 20s; at most 256 inbound packets/s/session |
| Terrain streaming | Radius 2 (25 slots); one batch in flight, at most 4 chunks/batch; shared 35,956-byte body, 35,965-byte scratch/Play session |
| Presence | At most 511 peers/viewer; latest snapshots/rosters, 50ms minimum dirty publish/flush |
| Chat | System relay only; 3-message sender burst, refill 1/s; 8192 deliveries/s aggregate; 32-message viewer queue |
| Directory | 2048 command slots; coalesced update notices, reserved generation-checked leave |
| Worlds | 4096 live worlds, 16 template descriptors; no per-world task/timer/thread |
| World profile | Up to 512 players, 65,536 buried overrides, 4096 modified chunks and 1 MiB logical overlay charges/world |
| Retained overlay snapshots | 64 MiB shared charge budget, including pinned old snapshots; charges are payload/header policy accounting, not RSS |
| Edits | At most 4096 operations per transactional batch; unchanged chunks/root snapshots shared by clones |
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
empty for this bootstrap; vanilla mechanics/tag semantics are not implemented.
No Mojang registry/tag JSON/NBT assets or implementation bodies are bundled.

## Verification and reports

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
