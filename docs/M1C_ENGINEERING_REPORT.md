# M1C engineering report — rendered stone and persistent flight

Original capture filenames refer to the [local validation archive](../evidence/README.md).

Date: 2026-10-29. Scope: **M1C only**, following AGENTS and roadmap order.
M1A/M1B were accepted before this work. M1C is implemented and its documented
gate passed. **Stop here: M1D, M2A, Java integration and later systems remain
unauthorized.** No mutable world, multiplayer replication or chat was added.

## Outcome and acceptance evidence

An unmodified, normally signed-in Java Edition **26.3 / protocol 777** client
rendered stone, double-jumped into creative flight, traveled **50+ blocks**
across chunk boundaries and remained connected for the requested minute.
The user's observation was: “It works ... 50+ blocks. flew. still connected.”
They also opened the inventory screen; no inventory item manipulation is
claimed. Visual evidence is user-reported, without a captured screenshot.

The independent server trace (`evidence/m1c-client-attempt3-server.log`) records
one legitimate authentication, Configuration and Play entry, one confirmed
teleport, **65 chunk sends / 40 unloads / 23 batch replies / 8 center changes /
10 keepalive replies / 576 movements / 6 flight-state changes**. Final counters:
zero protocol errors, timeouts, task failures, active connections, active sessions
and retained frame reservations. SIGTERM stopped the test server cleanly.
Ten keepalive replies also corroborate sustained connection beyond a minute.

Client record (`evidence/m1c-vanilla-client-check.json`) pins the successful
binary SHA-256 to
`5ea1081f273b0b9ff2aea4e2c0a54f426d6793b0a29a3507d217d2c05d4c258b`.
The final release rebuild has the same hash. Only tests, sampling tooling and
documentation changed after the successful client check.

| Roadmap concept / gate | Implementation and evidence |
|---|---|
| Virtual stone world | Pure material function: stone -64 <= y < 64, air otherwise; no dense chunks |
| Light/heightmap/biome | 24 sections, 26 light sections including sentinels, three typed heightmaps, plains; independent decode and exact mask fixture |
| Chunk+light and batches | Shared body, coordinate header, fixed view, ACK-paced batches; real-client and encrypted/compressed loopback tests |
| Spawn and teleport confirmation | Creative spawn (8,65,8), full protocol-777 destination ACK verified before movement changes interest |
| Creative abilities and flying | Client-reported flight works; six independently observed flight changes |
| Keepalive | 12-second issue / 20-second response deadline; real client and automated response test |
| Boundary and no permanent void/hang | User flew 50+ blocks successfully; eight observed center changes, streamed arrivals/unloads |
| Regression/resource/rollback | 53 tests, formatting, Clippy, release; listener fault recovery; measurements below; pre-M1C archive/binary retained |

## Goal, invariants and architecture

The goal was the smallest persistent rendered world that satisfies M1C.
[ADR-003](../decisions/ADR-003-m1c-immutable-stone-stream.md) records invariants,
trade-offs, dependency impact, tests and open questions. ADR-002 transport/auth
invariants remain in force. No global hot-path lock, unbounded queue, world
worker, simulation tick, automatic world timer or new third-party runtime
dependency was introduced. Workspace unsafe-code prohibition remains enabled.

| Layer | Responsibility after M1C |
|---|---|
| `world` (new) | Dependency-free immutable geometry/materials and fixed-capacity chunk-view mathematics |
| `chunks` (new) | Specialized immutable stone-to-protocol conversion; no player state, sockets or resident per-coordinate chunks |
| `protocol-777` | Pure Play field codecs and serializers; exact-version layouts |
| `sessions::player` (new private module) | Connection-local position/rotation/flight, sent subscriptions, ACK/deadline state and movement work budget |
| `sessions` | Authenticated session lifecycle, shared registry/template ownership and bounded Play I/O orchestration |
| `network`, `auth`, `server` | Existing persistent transforms/budgets, legitimate identity verification, supervision/runtime/CLI assembly |

The single immutable content function and wire dimension key are not an instance
manager or WorldId API. Future world ownership stays a separate layer; no session
holds mutable terrain or makes a socket await under a world lock. Multiple
connections may use the same immutable geometry, but players cannot see or message
one another. No later multiplayer feature is implied by allowing connections.

### Representation and scheduling decisions

One shared **35,956-byte** boxed chunk suffix contains identical sections,
heightmaps and light data. The session reuses one **35,965-byte** Vec to append
the packet ID/coordinates and body for any subscribed coordinate. Shared registry
wire metadata is **33,513 bytes**. Neither figure includes collection/object
overhead or TLS/compression/runtime allocations.

Each player has a fixed `[Option<ChunkPos>; 25]` subscription array, tested to
keep the complete `ChunkView` within 512 bytes. Radius two covers a 5x5 square;
near-first missing chunks are computed from desired center and sent slots.
Crossing one horizontal chunk edge preserves 20 overlapping chunks and replaces
five. Rapid travel cannot accumulate a stale pending-work mailbox. There are no
stored chunks for coordinates outside views and no per-coordinate terrain cache.
For scale context only, 512 active Play scratch buffers would occupy about
17.56 MiB in addition to other connection costs; this is arithmetic, not a
measurement of 512 authenticated players.

One batch may be in flight; at most four chunks are serialized/sent per batch.
Positive finite client rate hints select one to four chunks and 50–500ms pacing.
Batch and teleport ACKs expire after ten seconds. Each bounded send group or
unload reconciliation has a five-second write deadline. Pending bytes continue
to belong to the transport's existing writer/lease owner. A stalled peer fails
its own session; no global output mailbox grows. The task yields after each
chunk. A fully populated view has no chunk timer; connection keepalive/ACK
deadlines and inbound packets drive the loop. Frozen noon needs no time ticker.

Compression remains on the current I/O executor for this small uniform body.
Measured encode+zlib CPU cost is about 10.4–10.5 microseconds/chunk below; that
does not establish safety for arbitrary dense terrain or simultaneous bursts
from hundreds of authenticated players. No speculative compression pool/cache
was added. Framing still allocates compression/encoded workspaces and control
packets; reusable chunk assembly is not a whole-server zero-allocation claim.

### Resource isolation and input authority

M1B policies remain: default 512 connections/session slots, 16 MiB shared frame
leases, 1 KiB pre-auth / 64 KiB control inbound limits, 256 KiB outbound limit,
4 KiB raw read-ahead per connection, aggregate inbound 8 MiB/s work accounting,
compression threshold 256/ratio limit 128, eight authentication permits and two
RSA permits. One async thread and a maximum of four blocking workers (two-second
idle retirement) remain. Native TLS may create platform-owned threads separately;
the runtime cap is not a total process-thread promise. Duplicate identity's cold
admission mutex is unchanged; movement has no global lock.

M1C adds 256 inbound Play packets/second/session; finite movement/rotation and
ACK matching; x/z bounds +/-100,000; y bounds 63.99–512; pitch +/-90.01;
32-block travel burst refilling at 64 blocks/second. These are explicit resource
policies, not physics, collision simulation or production anti-cheat. Movement
before initial teleport confirmation cannot change interest. On-ground/input
flags do not authorize terrain or cross-player state. Read-only block-action
ACK/correction is allowed only within a sent subscription and eight-block reach;
it restores the immutable material and never applies edits.

## Protocol evidence and failures resolved

[M1C protocol ledger](M1C_PROTOCOL_EVIDENCE.md) contains packet IDs, complete
field ordering, nested codecs, policy distinctions and remaining uncertainties.
57-path reference inventory (`evidence/m1c-protocol-sources.json`) pins the
locally inspected exact-version reference. Public packet-wiki access was denied
by robots, so no public field-level cross-check is claimed. No game source bodies
or JSON/NBT assets are redistributed; identifier inventories from M1B remain.

Two real-client failures are retained as evidence:

1. Light BIT_SET was initially encoded as length-prefixed Longs. The client
   reported `ByteArray with size 16255 is bigger than allowed 2048`. Tracing the
   nested codec established trimmed little-endian byte arrays. A literal fixture
   now checks their exact wire representation; the first test decoder had repeated
   the encoder's incorrect assumption.
2. Accept Teleportation initially expected ID only. The client's 34-byte packet
   also includes three Double destination values and two Float angles. All fields
   are now decoded and matched to the issued destination. Tests reject every
   truncated prefix, wrong ID/destination and nonfinite values.

The third client attempt passed. These fixes emphasize why independent client
validation is required even when encoder/decoder unit tests pass. Additional
777-specific facts include the section fluid-count Short, singleton palettes
without storage length, typed heightmap maps rather than NBT, and registry clock
time maps. No fallback packet layout or authentication bypass was introduced.

## Build and regression tests

Final raw output: m1c-checks.txt (`evidence/m1c-checks.txt`).
**All pass on macOS arm64:** `fmt --check`, `test --workspace --locked` (53 tests),
`clippy --workspace --all-targets --locked -- -D warnings`, release build and CLI
help. The package's pinned compiler is Rust 1.99.1. The successful M1C binary links
Homebrew **OpenSSL 4.0.3** and macOS platform TLS; older M1B OpenSSL measurements
are historical. See environment/linkage record (`evidence/m1c-validation-environment.json`).

| Test group | Count | Evidence covered |
|---|---:|---|
| Auth | 5 | Provider crypto/challenge failures, signed hash, profile validation, RSA permit/cancellation lifecycle |
| Network unit | 10 | Persistent framing/transitions, cancellation, encryption/compression limits and work budgets |
| Network TCP integration | 11 | Fragmentation/coalescing, malformed input, rate/capacity/deadlines, task fault containment and shutdown |
| Protocol | 14 | Existing login/status fields, bounded input corpus, Play movement/control/truncation/nonfinite fixtures |
| Sessions | 7 | Shared registries/identity cleanup, Configuration, ACK state/deadlines, Play streaming/keepalive cleanup |
| Chunks | 3 | Full geometry/heightmap/light decode, literal bitset bytes, reused scratch capacity over 2,000 coordinates |
| World | 3 | Negative-coordinate boundaries/materials, five-chunk edge replacement, 200-center fixed-capacity churn |

The encrypted/compressed loopback Play test confirms initial 25 chunks, one batch
in flight, five arrivals/departures on one edge, ten on a two-center reversal,
full teleport ACK and an actual twelve-second keepalive/reply. It ends with
40 sends/15 unloads, two boundaries, zero byte leases on either peer and no
authenticated-account claim. Only private `cfg(test)` code enters the internal
Play handler directly; no production authentication bypass exists. A harness
assertion initially retained its received keepalive packet lease until test end;
explicitly dropping that packet before the final accounting assertion fixed the
test lifetime, without a production change.

Listener recovery (`evidence/m1c-listener-recovery.json`) passes with child
descriptor limit 32: three accept failures observed, established ping survives,
fresh status succeeds after recovery, normal exit and no retained leases. This
preserves the M1B fix for the reproduced listener-wide failure.

M1C has **not** been validated on Linux or Windows. A temporary Alpine VM
validation attempt was interrupted before results were exported; it provides no
pass evidence. M1B's existing Linux logs do not validate newly changed M1C code.
No long soak or coverage-guided fuzzer was run. The existing bounded arbitrary
input corpus covers status/handshake, not comprehensive Play fuzzing.

## Measured resources

Host: Apple M5 Pro, 18 logical CPUs, macOS 27.0.1 arm64, release binary
**3,133,872 bytes**. These are single-host observations, not capacity targets or
cross-version performance comparisons. RSS is process resident memory, not
allocator live-byte accounting; libraries, stacks, TLS, runtime and kernel socket
costs are not all represented by frame counters. MiB uses 1,048,576 bytes.

### Idle, connection load and recovery

Raw resource JSON (`evidence/m1c-connection-resources.json`) and
server counters (`evidence/m1c-connection-resources.log`) were generated using
the existing M1B-named transport harness against the final M1C release. It raises
global/per-IP admissions to 4096/s to exercise one loopback generator; defaults
are unchanged. No successful accounts are fabricated or session-service load
generated. Pending sockets/status requests do not allocate a Play chunk scratch.

| Observation | RSS KiB (MiB) | Cumulative CPU seconds (`ps`) | Threads |
|---|---:|---:|---:|
| Ready / five-second idle | 9,840 (9.61) | 0.01 | 1 |
| 128 partial connections | 11,184 (10.92) | 0.02 | 1 |
| 512 partial connections | 15,024 (14.67) | 0.03 | 1 |
| After 512 close | 15,024 (14.67) | 0.04 | 1 |
| After 1,000 reconnects | 15,136 (14.78) | 0.12 | 1 |
| After invalid RSA work | 15,360 (15.00) | 0.12 | 3 |
| Final idle after worker retirement | 15,344 (14.98) | 0.12 | 1 |

Five batches of 200 status reconnects all pass. RSS rises to 15,136 KiB after the
first batch and remains at that value across the other four. It does not return
to cold idle; this experiment cannot distinguish retained allocator/library
capacity from every possible leak. Active connection/session/frame counters all
return to zero. Repeated churn shows no further RSS growth over these batches.
Eight pending authentication slots are held, a ninth is rejected, invalid RSA
work recovers, idle partial input expires, and shutdown cancels four open sockets.
Final log: 1,657 accepted / peak 512 active / zero task failures or protocol
errors / one expected timeout / nine expected auth-path I/O failures. Frame
reservation peak is 532,480 bytes, retained zero.

`wait4` reports OS peak RSS **15,392 KiB (15.03 MiB)** and total process CPU
**0.117896 seconds** (user 0.051795, system 0.066101). `ps` CPU snapshots have
0.01-second resolution. Startup to ready was 10.67ms; shutdown 2.85ms. Local
status latency median 0.305ms / p95 0.461ms / p99 0.546ms / maximum 1.366ms.
These are loopback transport results, not 512 authenticated-player world results.

### Real client

Client sampling JSON (`evidence/m1c-client-resources.json`) contains 393 samples
over 84.793 seconds, approximately 0.2 seconds apart. The sampler attached after
login/session startup, so **it does not measure the authentication peak**.
Observed RSS was 14,768–14,928 KiB (**14.42–14.58 MiB**), observed threads two,
and cumulative CPU increased 0.12 to 0.22 seconds. The approximately 0.10 CPU
seconds / 84.793 wall seconds is roughly 0.12% of one core, with coarse `ps`
resolution and no isolation from shared-host effects. No thread-ownership
attribution or long-duration idle-zero-CPU claim is made. Final trace confirms
zero retained connection/session/frame leases after shutdown.

### Chunk microbenchmark

Three release runs (`evidence/m1c-chunk-profile.json`) use the `chunks` example:
100,000 coordinate encodes followed by 1,000 encodes+zlib operations per run.
Shared-body construction: 16.8–33.8 microseconds. Assembly into reused scratch:
**315–330ns/chunk**. Assembly+zlib: **10.415–10.503 microseconds/chunk**. Average
compressed body: 348 bytes, excluding outer protocol framing/encryption.
Scratch capacity remains 35,965 bytes. This synthetic benchmark uses biome ID
42; live plains ID is 41 (same one-byte VarInt width). It measures CPU encoding
only, not socket latency, authenticated-player throughput or all heap allocations.
Compression intentionally has its own allocated workspace. A uniform stone/light
payload is exceptionally compressible; do not generalize to heterogeneous worlds.

## Reproduction

```sh
scripts/cargo-local.sh fmt --all -- --check
scripts/cargo-local.sh test --workspace --locked
scripts/cargo-local.sh clippy --workspace --all-targets --locked -- -D warnings
scripts/cargo-local.sh build --release --locked
python3 scripts/check_listener_recovery.py --output evidence/m1c-listener-recovery.json
python3 scripts/measure_m1b.py --output evidence/m1c-connection-resources.json
scripts/cargo-local.sh run --release --example profile -p smolmc-chunks --locked
target/release/smolmc --bind 127.0.0.1:25565 --trace-connections
```

For visual acceptance, join using a normally signed-in stock 26.3 client,
observe stone, double-jump, fly horizontally 50+ blocks and stay a minute or more.
Do not manipulate inventory items/place blocks/use chat: those systems are
outside M1C. In another terminal attach
`python3 scripts/sample_m1c_client.py --pid SERVER_PID` for the same `ps` sampling
method used here. SIGTERM/Ctrl-C ends the server and lets the sampler save.
Keep connection tracing off for synthetic load. Reruns may overwrite evidence;
select different `--output` paths when preserving this historical run.

## Backward compatibility and limitations

M1A status/ping remains; description now identifies M1C. M1B online authentication,
encrypted/compressed Configuration and identity leasing remain. Play changes
from an adventure-mode empty 20-second probe to creative read-only terrain and
connection-driven keepalives. There is no forced successful-session probe close.
No persisted state, public Java API, database or migration format exists.

Only exact `minecraft/core/26.3` known-pack acknowledgement is supported; fallback
registry NBT remains unavailable and 773 named tag bindings retain empty members.
This is sufficient for the exercised bootstrap, not faithful gameplay tags.
No inventory item management, placement, entities/physics, health/food, multiplayer
visibility, messaging, edits, persistence, world instances, Java or survival.
Unsupported gameplay packets terminate that connection with a readable rejection.
The radius/bounds/pacing are deliberately small; fast/high-latency extremes and
all legal packet combinations are not exhaustively tested. Neither hundreds of
authenticated players nor thousands of worlds has been benchmarked. Those claims
require separate scope and evidence, not extrapolation from transport or stone.

## Rollback

No rollback is performed now. A pre-M1C snapshot is available locally at
`.tools/m1b-rollback.tar.gz` (source, manifests, documentation and prior evidence)
and `.tools/smolmc-m1b` (M1B binary). Their hashes are recorded in the environment
JSON. These are local development recovery artifacts, excluded from distribution.

To roll back, first stop this server with SIGTERM/Ctrl-C. Extract the archive into
a **new directory** and build it with the pinned toolchain/system OpenSSL, or run
the preserved M1B binary on its compatible native libraries. Verify status and
the M1B Play probe before replacing a deployment. A fresh directory avoids
retaining new `world`, `chunks`, `play.rs` or `player.rs` files accidentally; do
not blindly overlay an archive into an active tree. Preserve this M1C report and
user authorization separately: the archived AGENTS describes historical M1B
scope and grants no later milestone permission. M1B rollback returns to the
20-second empty probe and removes terrain/flight/Play keepalives. There is no
saved world data to migrate or undo. Temporary test listeners and validation
helpers are stopped; M1D and all later work remain untouched.
