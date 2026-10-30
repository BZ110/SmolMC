# ADR-004: M1D player presence and bounded system relay

Status: Implemented within explicit M1D authorization  
Date: 2026-10-29  
Scope: M1D only, following the provisional M1C acceptance/review. No M2A or later.

## Goal

Two legitimate 26.3 clients see each other's position/orientation/pose, exchange
readable system messages, and cleanly remove/recreate presence on disconnect/
rejoin. Preserve immutable stone, bounded transport/auth and explicit session
ownership; measure active Play rather than claiming capacity from idle RSS.

## Invariants

- Real account identity still comes only from the existing HTTPS session check.
  No production auth mode, fixture endpoint, token input or offline fallback.
- Sessions own controls and socket transforms. The directory owns replication
  membership/indexes only; it never waits for a socket or mutates terrain.
- One shared chunk body/registry inventory remains. No world instance manager,
  simulation worker, timer per world, persistence, inventory, Java or survival.
- Memory is bounded by session capacity, sent chunks, directed visible edges,
  latest snapshots and bounded chat/command storage. No motion history queue.
- No global hot-path lock; per-player pending-copy locks cover neither awaits
  nor compression/serialization. Shared watch channels also have scoped locks.
- Leave is generation checked and has a reserved queue permit, including when
  registration/session futures are cancelled or normal ingress is full.
- All relayed text is labeled System Chat; no signature/trust impersonation.

## Source evidence and review

[M1C review](../docs/M1D_M1C_REVIEW.md) reproduced a single peer spending the
shared ingress allowance. A connection-local 128 KiB/s gate now precedes the
global 8 MiB/s gate; a regression fails on archived M1C and passes with the fix.
Exact packet/nested-codec facts are pinned in
[the M1D ledger](../docs/M1D_PROTOCOL_EVIDENCE.md). Reference behavior, chosen
policies and observed client results are distinguished there. No Mojang source
bodies or game assets are copied into deliverables.

## Design and trade-offs

An event-driven server-scoped directory starts lazily for the first Play session.
It owns a member table, occupied-chunk index and sent-chunk subscriber index.
Visibility is directional: an acknowledged/ready target must occupy a chunk
actually sent to the viewer. A pending initial teleport cannot expose that
target. Joins and view changes reconcile candidates through chunk indexes;
crossing a target chunk updates affected pairs, avoiding a full server scan.
The directory is one necessary coordination task, not a world simulation worker.
With no commands it waits without timers, and after the last leave it discards
member/index high-water capacity. The small global command channel remains owned
by the service until shutdown.

Each peer has one immutable verified-profile packet and one current watch
snapshot. Each viewer has a watch roster of shared peer references and a local
last-sent table. Membership changes publish a new bounded roster; moves overwrite
the existing state. Notify coalesces viewer wakes. The session emits only changed
position/head/pose fields, pacing dirty flushes at 50ms minimum. Full absolute
position paths simplify correctness and avoid relative-base drift at a bandwidth/
CPU cost. Dense scenes inherently have P*(P-1) directed edges and are measured
as such; the design does not promise linear dense fanout.

The initial directory queued every published snapshot and yielded after each
command. A dense 256-player test filled ingress and disconnected healthy peers.
The fix coalesces ingress too: one short player-scoped pending-copy mutex, one
latest snapshot and at most one queued notice per player. Normal notifications
contain only ID/generation, not complete queued snapshots. One command plus up to 31 more are serviced before a cooperative yield, with a
1ms check on the additional batch work; individual commands are not preempted.
This is bounded batching, not a new scheduler/worker pool. Failed and passing
load results are preserved. The final dense 256-player run must be assessed
against its near-one-core CPU usage, not presented as spare production capacity.

Reserved leave permits plus monotonic generation tokens protect cancellation and
stale messages. Runtime entity IDs remain distinct from connection IDs and
authenticated UUIDs; production IDs are not reused. A rejected duplicate join's
cleanup cannot remove the prior member. No public extension API is promised by
these internal types. Future explicit world ownership must scope indexes and
chat membership separately; this single bootstrap realm is not an M2A API.

Chat is bounded plain text, one shared generated System packet per accepted
message, with a three-message sender burst/one-second refill before ingress,
independent directory admission/fanout limits and a 32-message per-viewer queue.
Overflow marks the slow viewer closed and wakes its session; the directory never
waits for it. Verified name plus `[System relay]` establishes transport/account
provenance without claiming signed-chat verification. Commands and inventory
remain unsupported. An attack packet is safely discarded without combat.

## Dependency impact

No new workspace crate or third-party runtime dependency; the M1C lock graph is
unchanged. New production modules are pure protocol presence codecs and internal
session replication. World geometry gains fixed-view copying/iteration/hash keys;
networking gains the local ingress gate. Private test-only probes cannot be
enabled in the production binary. No JVM, ECS, NBT framework or worker pool.

## Measurable acceptance

The final two-account stock client passed visibility, movement, rotation/crouch,
system chat, leave/rejoin and sustained connection. The final clean trace records
four authenticated sessions, 17 keepalive replies, four messages/eight deliveries,
peak two visible edges and zero protocol errors/timeouts/task failures/retained
leases. All 65 tests, formatting, Clippy and release builds pass on macOS arm64
and Alpine arm64 with matching implementation source hashes. Descriptor recovery
is exercised. Detailed load definitions, measurements and limitations are in
[the engineering report](../docs/M1D_ENGINEERING_REPORT.md).

## Compatibility, rollback and open questions

Status/login/Configuration/stone/flight remain. M1D adds visibility-scoped remote
Tab entries and unsigned system relay; inventory swap/drop/edit and most gameplay
remain unsupported. No saved state or external Java/API migration exists.
Stop and restore the preserved M1C workspace in a new directory or compatible
binary; the report supplies hashes and instructions.

Many real-account authentication/load capacity, 8–24-hour soak, Windows/glibc
x86_64, all legal control combinations, latency percentiles and thousands of
isolated worlds remain unverified. Near-saturated dense fanout, full-path movement
bandwidth, roster-copy cost during crowded membership churn and status-only
control wakeups remain profiling topics. Any M2A world ownership or later system
requires explicit authorization; stop after M1D.
