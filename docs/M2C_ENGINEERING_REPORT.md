# M2C engineering report — optional Java SDK and local bridge

Original capture filenames refer to the [local validation archive](../evidence/README.md).

> Protocol erratum, 2026-10-30: real M3 client checks exposed a command-advertisement
> packet ID defect in this checkpoint. Commands must use 0x10, not 0x0f. The M2C
> headless fixture mirrored the incorrect ID; its pass did not establish vanilla
> GUI command rendering. Revised M3 corrects it and adds an independent official
> client codec check. Historical results below remain historical; see
> [M3 protocol evidence](M3_PROTOCOL_EVIDENCE.md).


Date: **2026-10-29 America/Toronto** (evidence timestamps extend into October 30
UTC). Authority: explicit user M2C-only authorization after optimized commit
`d881fe3`. Pre-implementation checkpoint **`m2c-before` → `5d6f8d2`** updates
AGENTS.md and preserves the optimized code. Prior tags `replication-perf-validated`,
`replication-perf-before`, `m2b-validated` and `m2a-accepted` remain intact.

**Outcome: the roadmap M2C gate is satisfied.** Java can launch/attach, manage
isolated COW instances, batch-edit/read, transfer/message players, register literal
commands and receive bounded asynchronous events. A compiling, runnable Java
number-guessing arena game uses those controls without modifying Rust. Bridge-off
starts no JVM, listener or control event bus. Rust remains authoritative; no JNI,
unsafe code, JVM-owned Rust memory, per-world worker or simulation loop.
**Stop after M2C. M3 and later milestones remain unimplemented and unauthorized.**

## Goal → invariants → source evidence

Expose existing M2B controls through the optional out-of-process boundary proposed
in docs/09, with explicit ownership, failure behavior and actual Java integration.
Keep ADR-005–007 world/identity isolation, immutable template/registries, sparse
COW/budgets, authenticated networking and optimized replication intact. No blocking
Java callbacks on Rust networking/replication paths, per-packet Java serialization,
unbounded queue or one-JVM/thread/tick-per-world design.

Sources: roadmap M2C, docs/02, 05, 07, 08, 09, 20, 21, 27, M2B and replication
reports. [ADR-008](../decisions/ADR-008-m2c-optional-java-ipc.md) records the design;
[bridge contract](M2C_BRIDGE_CONTRACT.md) defines the implemented wire/API, grants,
limits, timing, errors and packet evidence. Broad illustrative future API sketches
were not treated as authorization for imports, survival or a plugin framework.

| Roadmap concept / gate | Implementation and verified evidence |
|---|---|
| Sidecar launch/attach | Real Java `launch()` owns one Rust child; `attach()` connects to an explicitly enabled private socket. Close/shutdown/restart/reconnect tested |
| Semantic API version | SDK 0.1.0, wire 1.0, explicit handshake/version rejection; no native ABI/layout exposure |
| World API | Built-in `stone_flat` descriptor, editable/read-only create, clone, reset, park, destroy, resources/list, batched reads/edits; 1000-edit and COW divergence tests |
| Player API | Verified session/UUID snapshots, accepted bounded system message and transfer to fixed world spawn, commit event, stale session protection; actual encrypted Play clients |
| Commands | Bounded connection-owned literals with greedy arguments; unsigned player slash command reaches Java; operator invoke and cleanup/re-registration tested |
| Event bus | Optional cold lifecycle/player/command snapshots; subscription/unsubscription, ordering, bounded ring, explicit gap disconnect; no movement event stream |
| Tiny Java minigame without Rust changes | [TinyArena](../java/examples/TinyArena.java) guesses a number, messages winner, transfers to separate lobby, resets empty round world; compiled and startup/setup/shutdown executed. Gameplay control operations separately exercised through real SDK/Play integration |
| Bridge-off uses no JVM | Default binary path has no process launch or IPC constructor; disabled-bus and lazy-player-resource tests, separate production PID idle measurement |

The example is application Java code, not a shipped plugin loader or Bukkit/Paper
adapter. Its random-number game does not implement combat/survival mechanics.
No graphical client verification was required or newly claimed; protocol behavior
is checked with private synthetic post-authentication TCP/AES/compression clients.
The production executable cannot enable their authentication bypass/fixture path.

## Architectural decisions → trade-offs → dependency impact

A new `smolmc-bridge` crate owns the optional Unix listener, framing, authentication,
request dispatch, work limits, subscriptions and peer cleanup. The composition root
adds `--bridge-dir NEW_PATH` and `--bridge-access observe|control` (default observe).
0700 directory / 0600 socket and bootstrap token, OpenSSL-generated 32-byte secret,
constant-time comparison, strict schema/correlation/frame validation. Credentials
are not command-line/environment inputs and are not included in evidence.

The transport is a small fixed bounded binary schema, not an unmeasured claim of
superiority over Protobuf/FlatBuffers/Cap'n Proto. It avoids a Java codec dependency
and a generated build/runtime for this narrow control surface. Measurements are
end-to-end, not a codec bake-off. Windows named pipes/remote IPC are deferred.

Existing cold catalog operations serialize lifecycle mutations; no lock covers
socket awaits, encoding, ordinary movement or Java execution. Events use immutable
snapshots in a 128-entry bounded broadcast ring. Player identity/message endpoints
and the shared command-tree watcher are created only when startup enables control.
Late enabling after Play membership is rejected. Control-bus lifetime is Service
lifetime; this is not a general live-toggle mechanism. Peer ownership IDs do not
reuse across bridge owners. Unregistering an owner with no commands does not
republish a command tree or wake all players.

World handles include server incarnation and reset generation. Player handles
include incarnation, monotonic entity session ID and verified UUID. The existing
same-dimension switch path owns transfer, old-peer/chunk cleanup, destination
membership, teleport and client ACK handling. Receipt means accepted/coalesced;
`PLAYER_WORLD_CHANGED` means membership committed, not client-render completion.
Reset/destroy still reject occupied worlds. Java-created worlds survive sidecar
loss; reconnecting applications must reconcile IDs/state rather than blindly retry.

The SDK has typed records/enums, futures, explicit lifecycle and machine-readable
errors. One reader and writer use Unix SocketChannel directly, with bounded
outbound/pending/completion/callback work. Java callbacks never execute in Rust.
Standard future continuations may run on completion workers or the attaching caller;
application blocking work needs its own executor. Slow/uncooperative Java code can
stall its own bounded workers, but cannot hold a Rust world lock. Queue overflow
closes that sidecar visibly; no silent event-gap acceptance. Cancellation/timeout
cannot undo a committed mutation. There is no durable command replay or exactly-once
transaction protocol across disconnects.

Maximums: 4 peers/listener; 128 KiB frame; 2048 accepted requests/1s shared window;
16384 edit inputs/window; existing 4096 edits/batch; 64 global command names; 8
queued messages/enabled player. Java has 64 end-to-end outstanding slots (including
completion work), 64 outbound frames, 128 event tasks, 2 completion workers and
5s request deadlines. Handshake/partial-frame/write deadlines are 2s. See the
contract for exact bounds, permissions, error mapping and callback semantics.
The permissions are observe/control for an operator-approved OS-user trust domain,
not per-plugin/per-world tenant isolation or an untrusted JVM sandbox.

No new external Cargo package/version or Java runtime dependency. OpenSSL, Tokio
and zeroize already existed; the new workspace crate reuses them. Auth, network,
chunks, pure world implementation, session player state machine, replication,
profiling helpers and registry inventories are preserved; source/hash evidence
records this. The only new Minecraft layouts are the bounded unsigned command
and literal command tree, verified against the private user-owned reference
inventory and an independent fixture decoder (contract lists paths/fields).
No proprietary implementation or game asset is delivered.

## Measurements — short, separate processes, no scaling sweep

Host: macOS 27.0.1 arm64 / Apple M5 Pro; pinned Rust 1.99.1, system OpenSSL 4.0.3,
OpenJDK 25.0.5, Java compiled `--release 17`. CPU means **percent of one core**.
RSS is resident memory, not allocator-live bytes or proportional shared-library
memory. Java and Rust are sampled independently; adding RSS double-counts any
shared resident pages. Short shared-host runs have no confidence interval or
capacity guarantee. `ps` CPU resolution is 0.01s; “0.00%” means no measured delta,
not mathematical zero CPU (about 0.33 percentage-point resolution over 3s).

Before idle (`evidence/m2c-idle-before.json`) uses the preserved optimized release
binary, 2.5s warmup and ~3s observation. Final overhead evidence (`evidence/m2c-resources-final.json`)
uses the final production binary, 2.5s warmup/~3s windows. The initial implementation
measurement is also retained in initial resources (`evidence/m2c-resources.json`);
final cold startup/ownership guards and SDK bounds were then validated. This is
not another increasing-world/player campaign.

| Mode | Rust RSS KiB / MiB | Rust CPU | Rust threads | Additional JVM RSS / CPU / threads |
|---|---:|---:|---:|---|
| Before, optimized Rust only | 9872 / 9.64 | 0.00% | 1 | none |
| After, bridge off | 10016 / 9.78 | 0.00% | 1 | none |
| Enabled listener, no Java | 10176 / 9.94 | 0.00% | 1 | none |
| Java attached, idle after control workload | 10272 / 10.03 | 0.00% | 1 | 86416 KiB / **84.39 MiB**, 0.00%, **31 threads** |

Bridge-off incremental RSS in this run is **144 KiB**. Enabling the listener adds
160 KiB over after/off; attached Rust is 256 KiB over after/off. JVM overhead is
substantial and explicit: combined Rust+JVM RSS is **94.42 MiB**, with 32 total native
threads in this warmed workload. JVM infrastructure/JIT/GC contributes beyond the
SDK's at most six owned workers; none of this multiplies by world count. JVM heap,
GC and thread ergonomics were default, with no forced GC or hidden heap tuning.

Final loopback Java measurements (100 warmup reads, 200 timed resource requests,
100 park/notification cycles):

| End-to-end metric | p50 | p95 | p99 |
|---|---:|---:|---:|
| Java submit → completed resource future | 59.54 μs | 95.38 μs | 179.88 μs |
| Java mutation submit → event callback | 47.83 μs | 105.08 μs | 288.04 μs |

Event measurement includes IPC/control commit/callback dispatch; it is not an
unsynchronized comparison of Java and Rust clocks, nor client rendering latency.
A burst of 1024 reads in batches of 32 completes in **10.15ms**, zero rejected
(~100,903 successful requests/s **during that short burst**). This is explicitly
**not sustainable configured throughput**: admission is capped at 2048 requests/
fixed second and 16384 edit inputs/second. The separate 2500-call saturation check
gets explicit Backpressure responses (365 in the saved final macOS run); deterministic
quota tests verify exact request/edit limits and refill. Java's 64-slot cap is
separately verified while the test-owned Rust child is briefly SIGSTOPped/resumed.

### Active Minecraft regression

Only **one representative workload** was compared twice: 128 players / one world,
1s warmup and ~8s observation. Same existing 20Hz movement/yaw, 1Hz crouch, system
chat every two seconds, actual TCP/AES-CFB8/compression and 50ms replication pacing.
Separate Rust, client-generator and Java processes. Before is the saved optimized
release probe (`replication-perf-validated`), after enables the bridge and one Java
observer subscribing to cold events. No movement IPC is introduced. Native samples
and raw logs are saved with before (`evidence/m2c-play-before.json`) and
Java-enabled (`evidence/m2c-play-java.json`) evidence. Later changes add cold
startup/peer guards, test assertions and bounded SDK input validation, not the
replication/network/chunk/player hot paths; measured binary hashes are retained.

| Metric | Optimized before | M2C + Java observer |
|---|---:|---:|
| Rust CPU, one core | 32.00% | 32.91% |
| Rust RSS | 26736 KiB | 26864 KiB |
| JVM separately | none | 57008 KiB (55.67 MiB), 28 threads, no measured CPU delta |
| Incoming movements/s | 2584.0 | 2581.5 |
| Published snapshots/s | 2521.7 | 2521.6 |
| Position packets/s | 308141 | 306951 |
| Directed visible edges | 16256 | 16256 |
| Directory p95 upper bucket | 4 μs | 4 μs |
| Flush p95 upper bucket | 8.192ms | 8.192ms |
| Publication-to-write p95 upper bucket | 65.536ms | 65.536ms |

CPU is +0.91 percentage points (~2.8% relative) in these two short runs; Rust RSS
+128 KiB and position throughput -0.39%, with matched input/snapshot rates and
identical p95 buckets. Both pass with zero lost clients, protocol errors, timeouts,
task/I/O failures and final connections/replicas/frame leases zero. No statistically
bounded “zero regression” or speedup claim. The earlier report's 27.69% at 128 and
82.48% at dense 256 were separate historical runs; they are not mixed into this
comparison. New 256-player sweeps were not necessary. Dense required fanout and
per-client AES-CFB8 remain the previously identified bottleneck, unchanged here.

## Regression, failure isolation and cleanup

**100 Rust test executions pass**, zero failures, two opt-in tests ignored by the
default suite and invoked separately as appropriate. Groups: auth 5, bridge 6,
chunks 3, network unit 13 + TCP 11, protocol 18, sessions 30, world 14. Six bridge
checks are intentionally also compiled into the private sessions fixture against
cfg(test) Service, so these are 100 executions / 94 distinct test bodies, not 100
independent scenarios. This keeps private Play test entry points out of production.

macOS evidence: full suite (`evidence/m2c-tests.txt`), fmt (`evidence/m2c-fmt.txt`),
Clippy (`evidence/m2c-clippy.txt`), release (`evidence/m2c-release.txt`),
examples (`evidence/m2c-examples.txt`), strict Java compile/integration (`evidence/m2c-java-tests.txt`),
Java encrypted Play (`evidence/m2c-java-play.txt`),
fault checks (`evidence/m2c-faults.json`),
listener recovery (`evidence/m2c-listener-recovery.json`),
example startup/shutdown (`evidence/m2c-example-smoke.txt`).

Tests cover real launch/attach; 1000-edit isolation and clone divergence; reset,
park/destroy/stale world/server/player handles; unsupported/invalid/read-only/
occupied requests; concurrent calls; unsubscribe ordering; disconnected registration
cleanup and reconnection; unique ownership even across two bridge listeners;
frame size/UTF-8/count/trailing-field checks; authenticated version/grant rejection;
nonreading peer write timeout; partial-handshake deadline; explicit event-ring gap
disconnect; bounded pending calls/callbacks; actual killed JVM and healthy core status;
shutdown file/thread cleanup; no-op unregister avoiding player command-tree wakes.
Existing auth/encryption, bounded read/write/cancellation, coalescing, chunk interest,
reserved leave, world isolation/switch, pose/chat/rejoin and COW/resource tests pass.

The real Java/Play test additionally streams **2000 park events over about 2.7s**
while two encrypted 20Hz clients continue moving, checks every event reaches Java,
and asserts movement progress before client chunk/entity/message assertions.
This is a short fairness/failure check, not a high-load soak. A separate synchronous
256-event burst deliberately overruns the 128-slot ring and closes only that peer;
reattach and world state remain healthy. Slow Java callback queue overflow reports
Backpressure. Terminating Java releases its command registrations, not worlds or
Minecraft sessions. OS-killed Rust may leave private bootstrap paths; explicit
operator cleanup after confirming ownership is documented rather than unsafe socket
takeover. No promised graceful cleanup after SIGKILL or host power loss.

Fresh **Alpine Linux arm64/musl** validation uses Linux 6.18.58, pinned Rust 1.99.1,
OpenSSL 3.5.9 and OpenJDK 25.0.4, Java `--release 17`. Full tests/fmt/Clippy,
release/examples, actual Java integration, Java/Play event flood, bridge faults and
listener recovery pass. Source hashes match the host code snapshot. Evidence:
Linux metadata (`evidence/m2c-linux-meta.json`),
tests (`evidence/m2c-linux-tests.txt`), Clippy (`evidence/m2c-linux-clippy.txt`),
build (`evidence/m2c-linux-build.txt`), Java (`evidence/m2c-linux-java.txt`),
Java/Play (`evidence/m2c-linux-java-play.txt`),
faults (`evidence/m2c-linux-faults.json`),
listener recovery (`evidence/m2c-linux-recovery.json`).
A validation-cache issue was caught by comparing test names/counts, not only
source hashes: extracting a changed archive with preserved timestamps let Cargo
reuse an older test binary (98 executions). Final evidence comes from a fresh VM
and clean build, and explicitly includes both peer-ownership test executions.
Linux performance, Windows, x86_64 and graphical client tests were not run.

## Compatibility → known limitations → open questions

M1A–M2B and optimized replication remain supported. Default Rust-only use needs
no Java installation. IPC is explicitly enabled, local, startup-scoped and optional;
there is no new unauthenticated public admin listener. New wire command support is
narrow and asynchronous. Existing signed chat verification, inventory and unsupported
packet limitations remain; unknown/signed commands can still reject/disconnect.
World edits remain buried Air/Stone only; the Java API cannot bypass those engine
quotas/bounds. The per-world `Feature::JavaBridge` resolver bit remains unavailable:
this milestone adds a global optional control bridge, not per-world plugin hosting.

Deferred: arbitrary templates/imports, coordinate teleport, game modes/inventory,
permissions/plugin-loading framework, cancellable events, movement subscriptions,
region snapshots/cache, durable IPC replay, persistence, scheduling API and Bukkit/
Paper compatibility. No M3/M4/M5 implementation. Per-OS-user credentials do not
sandbox multiple hostile plugins sharing one JVM. Event subscriptions are one per
SDK connection and snapshots are not atomic with subscription/replay; applications
reconcile identities after reconnect. Uncooperative Java code may retain its own
application objects or workers; Rust memory/queues remain bounded. No long soak,
allocator-precise leak proof, maximum-player mutation-delivery guarantee or dense
256-player Java workload is claimed. Logical overlay charges are not total RSS.

## Reproduction → rollback

```sh
scripts/cargo-local.sh fmt --all -- --check
scripts/cargo-local.sh test --workspace --locked
scripts/cargo-local.sh clippy --workspace --all-targets --locked -- -D warnings
scripts/cargo-local.sh build --release --locked
scripts/cargo-local.sh build --release --workspace --examples --locked
scripts/java-check.sh
scripts/cargo-local.sh test -p smolmc-sessions java_sidecar_controls_real_encrypted_play --locked -- --ignored --nocapture
python3 scripts/check_m2c_faults.py
python3 scripts/check_listener_recovery.py --output evidence/m2c-recovery-repeat.json
python3 scripts/measure_m2c.py --output evidence/m2c-resources-repeat.json
scripts/cargo-local.sh test --release -p smolmc-sessions --no-run --locked
python3 scripts/measure_replication_short.py --executable TEST_EXECUTABLE --small-only --bridge --seconds 8 --output evidence/m2c-play-repeat.json
java -cp target/java/classes TinyArena target/release/smolmc
```

Build Java classes before Java/Play/active-with-bridge checks. Select the release
test executable printed by Cargo; the authentication fixture is test-only. Linux
uses the same checks with `RUSTFLAGS='-C target-feature=-crt-static'`, system OpenSSL
development headers, Rust 1.99.1 and Java 17+ JDK. macOS resource samplers explicitly
use macOS ps units; do not pass them off as Linux performance measurements.
Environment, hashes and preservation (`evidence/m2c-environment.json`) records
which binaries produced each short measurement and the unchanged hot-path sources.

Rollback was not performed. Stop Java owners and Rust gracefully; preserve logs
and reconcile ephemeral state (there is no on-disk world migration). Make an
isolated checkout:

```sh
git worktree add --detach ../SmolMC-pre-M2C m2c-before
```

Use pinned Rust/OpenSSL, a **separate target directory**, and run the old full checks
before replacing a binary. Remove `--bridge-dir`/`--bridge-access` and Java SDK
calls from that deployment. Current ephemeral worlds/connections disappear on
restart, as before. Retain the optimized `replication-perf-validated` checkpoint;
rolling back M2C does not require undoing the replication optimization. The final
implementation/report is tagged **`m2c-validated`**. Stop before M3.
