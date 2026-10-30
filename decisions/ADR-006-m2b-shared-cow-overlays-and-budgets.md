# ADR-006: M2B shared COW snapshots, profiles and resource ownership

Date: 2026-10-29. Status: implemented; measurements/acceptance recorded in the
[M2B engineering report](../docs/M2B_ENGINEERING_REPORT.md). Scope: M2B only.
M2A was accepted by the user; Git tag `m2a-accepted`, commit
`7a1ac2e3e46cbe17a3097065966f94a14339ab0b`, precedes implementation.

## Goal → invariants

Host many independent idle/edited arenas without copying template terrain,
registries or complete overlay maps on every edit. Make clone/reset/reuse cheap,
account retained snapshots, and reject resource/feature requests explicitly.
Preserve accepted M1A–M2A protocol/auth/ownership behavior and ingress coalescing.
No per-world task/thread/timer, mutable shared template, global hot-path lock,
Java/IPC/plugin/survival/persistence subsystem or new wire layout.

## Source evidence

`docs/13_ROADMAP.md` M2B requires immutable chunks + sparse overlays, clone/reset,
parked idle state, world budgets, feature resolver and metrics. Its gate is tracked
incremental RAM/timer work and correctness under high churn, with real measurements.
Ownership and test requirements come from `05_WORLD_INSTANCES.md`,
`06_CHUNKS_AND_STREAMING.md`, `07_FEATURES_AND_ZERO_IDLE.md`,
`20_TEMPLATE_FORMAT_AND_LIFECYCLE.md`, `21_SECURITY_AND_TRUST.md` and ADR-005.

The [Rust 1.99.1 Arc documentation](https://doc.rust-lang.org/std/sync/struct.Arc.html)
substantiates shared ownership and last-reference destruction. The implementation
uses immutable Arc-owned nodes with explicit transactional copying of touched
chunks; there is no unsafe pointer/layout manipulation. Header charges below are
a deterministic quota policy, not a promise about allocator/ABI or exact RSS.

## Design trade-offs

**Shared base:** one Service retains one existing encoded stone body and registry
set. World instances now share Arc-owned immutable template descriptors. No
per-coordinate base chunks, prewarming, alternate encodings or extra compressed
cache are created: the accepted uniform encoding already measured cheaply.

**Sparse COW:** an empty overlay is `None`, with no heap allocation. An edited
snapshot is one immutable sorted chunk index pointing to immutable sorted sparse
chunk arrays. A clone shares its root in O(1); an edit transaction copies the
index and only touched chunks. Unchanged chunks keep their original Arcs.
Chunk replay uses a chunk-index lookup, rather than scanning every world edit.
The representation stores local coordinates compactly, not a full dense world.

A trusted batch of at most 4096 edits validates all positions first, applies
input-order last-write-wins semantics, removes base-valued overrides, checks
budgets and then publishes one snapshot. Rejection leaves world content and
edit counters unchanged. Single-edit callers retain their existing behavior.
Sparse arrays deliberately remain sparse at high edit counts; no unmeasured
palette-promotion threshold or general terrain encoder is introduced. Root-index
copying is O(modified chunks), touched-chunk copying is O(local edits); these costs
and concentrated/spread edit workloads are measured, not claimed constant.

**Lifecycle:** clone copies identity/profile metadata and shares immutable state,
never players or subscriptions. Reset reuses WorldId, advances generation and
drops the overlay; destroy retires WorldId. Occupied reset/destroy remain rejected.
Explicit park and last-player leave schedule no work. Ready worlds are also idle
until an event. Terrain watch channels are allocated only for occupied worlds and
removed on last leave; parked worlds retain no watch/channel per instance.
A global cold control lock serializes lifecycle/admin edits; network I/O,
serialization and ordinary movement do not acquire it.

**Budgets:** defaults remain 4096 live worlds, 16 template descriptors and 512
aggregate Play memberships. Per-world validated limits cap players (512), edits
(65,536), modified chunks (4096), and logical snapshot charges (1 MiB).
A shared 64 MiB retained-overlay budget charges every unique immutable root/chunk
until its final Arc drops, including obsolete snapshots held by readers or clones.
Cloning shares charges globally; each world's logical limit still counts its full
referenced content. Failed reservations roll back all tentative node ownership.
Reset/destroy can release data without reserving another overlay allocation.

Charges use `size_of` for node structs, a two-usize policy charge per Arc header,
and boxed array lengths. They exclude allocator rounding/metadata, temporary
bounded mutation buffers, manager/routing metadata, watch channels, sockets,
registry/terrain bodies and runtime memory. They are **not allocator live bytes,
RSS or a hard OS memory limit**. Committed per-world logical quotas and global
retained snapshot quotas are separate from bounded transient mutation work.

**Profiles/resolver:** `editable` preserves accepted trusted buried Air/Stone edits;
`read-only` disables that capability. Required protocol/world/chunk/player/
presence/system-chat dependencies are resolved before instance allocation.
Physics, mobs, persistence and Java capabilities are declared unavailable and
rejected without constructing workers/caches. The resolver does not add those
systems. Client Feature Flags remain untouched. CLI `--world-profile` chooses
these supported profiles; richer per-world limits are trusted Rust controls.

**Metrics:** per-world state, profile, player count, edit/chunk count, logical
charges and zero world-owned task/timer construction counts; global current/peak
retained charges. These counters do not introspect or attribute shared session,
directory, TLS or Tokio tasks as world-owned timers. RSS/CPU/native threads are
measured independently in a separate process with its async executor active.

## Dependency impact

No new third-party package, workspace crate, Cargo feature or lockfile change.
Auth, network, protocol codecs, registry inventories and chunk encoder remain
unchanged from accepted M2A. No new authentication fixture is enabled in production.
The resource driver is a separate example, with one fixed controller thread;
it does not add threads to production world creation.

## Measurable acceptance tests

- Pointer/content checks: template and clone sharing, unchanged chunk sharing,
  independent edits, input-order duplicates/revert and negative chunk boundaries.
- Transaction checks: stale handles, invalid positions, edit/chunk/player/byte
  limits, unavailable capabilities, no partial publication or lost budget leases.
- Retention checks: pinned snapshots remain charged after reset/destroy; all
  charges return to zero after final holder cleanup; idle worlds have no watchers.
- Lifecycle checks: concurrent clone/edit/reset/park/destroy, generation-safe reuse,
  full retained M1A–M2A suite and four-client cloned-world wire divergence.
- Resource checks: increasing empty/edited worlds, concentrated/spread edits,
  cheap clones then COW divergence, reset/destroy and repeated lifecycle churn;
  report latency distributions, charged bytes, RSS, CPU and threads separately.
- Regression checks: fmt, Clippy, release, listener failure recovery, dense active
  Play/churn probe; fresh Linux conformance/build where feasible.

The report records actual outcomes, commands, hashes and untested areas; none of
the acceptance items is satisfied merely by a successful compilation.

## Compatibility → rollback → open questions

Accepted buried edit bounds/lighting assumptions, read-only player interactions,
online authentication, chunk batching, system chat and same-dimension switching
remain. No surface editing, arbitrary template import, persistence or stable SDK.
The default profile is editable, preserving existing trusted M2A calls.

Stop the process and restore `m2a-accepted` into a separate Git worktree, rebuild
with the pinned compiler/native OpenSSL and rerun checks; no saved-state migration.
All edited state is ephemeral. Keep current authorization separate from old docs.

Long soak, Windows/glibc/x86_64, allocator-precise live/fragmentation profiling,
maximum edited-world/slow-reader combinations, general block types/lighting and
future cache/palette promotion remain open. There is no Java/plugin work in M2B.
Stop after M2B; M2C requires new explicit authorization.
