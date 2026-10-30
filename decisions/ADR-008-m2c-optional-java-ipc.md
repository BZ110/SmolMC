# ADR-008: optional bounded Java control sidecar

Date: 2026-10-29. Status: implemented and validated; M2C authorization only.
See [engineering report](../docs/M2C_ENGINEERING_REPORT.md) for measured results,
regressions, limitations and rollback.

## Goal → invariants → evidence

Meet roadmap M2C's tiny Java minigame gate over existing authoritative Rust world
controls. Preserve ADR-005–007 ownership, immutable COW terrain, bounded transport
and optimized replication. No Java per-packet callbacks, per-world workers, raw
pointers, JNI, simulation or M3 plugin host. Bridge-off constructs no IPC/event bus
and starts no JVM. Sources: docs/09, 08, 21, 27, M2B/replication reports.

## Design trade-offs → dependencies

Optional local Unix socket; Java launches the Rust child or attaches to an
operator-enabled server. One private 0700 directory, 0600 socket/bootstrap secret,
32 random bytes from OpenSSL, constant-time credential comparison. Same OS user
is the trust boundary; explicit observe/control grant, no remote transport or
untrusted plugins in one JVM. Up to four peers; fail closed on invalid framing,
authentication, event lag or blocked writes. Disconnect never rolls back committed
mutations or destroys Java-created worlds. IDs include server incarnation;
world generations advance on reset and entity IDs never reuse in an incarnation.

Small fixed binary schema with network byte order, correlation IDs, explicit
status and bounded collections. No third-party SDK dependencies or generated
runtime. Protobuf offers evolution/codegen but adds a build/runtime dependency;
FlatBuffers/Cap'n Proto zero-copy benefits do not apply to these small control
snapshots. JSON is useful for management but needs a Java dependency/parser and
expands 4096-coordinate batches. This decision favors the smallest auditable
fixed M2C surface; no assertion of codec superiority. Short end-to-end measurements
will determine actual IPC cost, not a synthetic codec contest.

Read-only noncancellable events are emitted at cold lifecycle/command points;
no movement stream. Bounded broadcast ring disconnects lagging subscribers rather
than silently accepting gaps. Java uses bounded pending calls, outbound and
callback queues. Request deadlines close an ambiguous connection; callers must
reconcile snapshots rather than blindly replay. Futures/callbacks complete away
from the socket reader so arbitrary Java code cannot stop IPC progress.

## Measurable acceptance → open questions

Real Java launch/attach/minigame, isolated 1000-edit batches, resets/stale IDs,
player world transfer/teleport/message, command events, concurrency, overload,
slow/crashed/disconnected sidecars, restart and cleanup. Full Rust checks and Java
compile/tests; short separate-process RSS/CPU/thread, request/event latency,
throughput/saturation and active-player comparison. Feasible Linux validation.
Windows named pipes, arbitrary templates/materials, cancellation deadlines,
plugin loading/permissions framework and Bukkit compatibility remain deferred.

## Final acceptance and rollback

The SDK/bridge and Java guessing-game example compile and run without new Rust
application code. Full Rust tests/fmt/Clippy/release/examples and real Java
launch/attach/lifecycle/isolation/concurrency/fault/Play checks pass on macOS and
Alpine Linux. The full suite executes the shared six bridge checks twice: 100
executions, 94 distinct bodies. Two opt-in fixture tests are separately invoked.
The report distinguishes short measured CPU/RSS/request/event costs, JVM overhead,
burst throughput versus configured sustained quota, event flood and untested soak.
No broad feature/plugin compatibility is claimed.

Cold startup rejects late enabling instead of silently omitting existing players.
Peer ownership IDs never reuse across bridge owners, and no-op unregister does
not wake player command trees. Engine mutation limits, accepted-versus-committed
transfer semantics and failed/ambiguous receipts are explicit in the contract.

Rollback: gracefully stop Java/Rust; use a separate worktree at `m2c-before`
(`5d6f8d2`), a separate Cargo target, and rebuild/verify before replacement. Remove
bridge CLI/SDK deployment calls. No persistent-state migration; current ephemeral
worlds disappear on restart. `replication-perf-validated` remains preserved.
Stop at M2C; M3 and later are not implemented or authorized.
