# 07 — Feature modules, profiles, dependencies and 'near-zero idle' engineering

**M4B implementation update (2026-10-30):** explicitly configured native item
gameplay now adds counted/component inventory, bounded world-local drops,
durability and armor. The existing directory owns item/interaction ordering;
one shared deadline mechanism handles actual active work, with no inactive-world
scan. Predictive clicks reconcile against Rust state; commit receipts preserve
inventory effects across disconnect. See [M4B report](M4B_ENGINEERING_REPORT.md),
[ADR-011](../decisions/ADR-011-m4b-authoritative-items.md) and
[security/budgets](M4B_SECURITY_REVIEW.md). User-accepted and validated at `m4b-validated`; see the report for evidence and limits.


**M4A implementation update (2026-10-30):** bounded imported templates, sparse
block/light COW, local collision and held mining now extend the historical scope
below. See [M4A contract](M4A_TERRAIN_CONTRACT.md), [M4A report](M4A_ENGINEERING_REPORT.md)
and [ADR-010](../decisions/ADR-010-m4a-imported-arena-terrain.md). One lazy shared
lighting worker; no world/player ticking task and no required Java hot path.
Root AGENTS.md and the roadmap govern current authorization.

**Current implementation:** M2B implements editable/read-only profiles with the fixed required dependency closure, rejected unavailable heavy capabilities, creation-time limits and resource metrics. Parked worlds have no worker/timer/watch; this is not a general live-toggle framework. See [M2B evidence](M2B_ENGINEERING_REPORT.md). Root AGENTS.md controls authorization. Revised M3 `--pvp on` adds state/critical
queues only for occupied PvP sessions, with no timer/task per player or world.
Cooldowns/iframes and the supported flat-roof motion subset advance lazily on
existing events. `--pvp off` preserves the creative-flight path and allocates no
combat state or critical-delivery channel. This startup option is not a general
live feature-toggle framework.

## Primary user requirement
Enable/disable **large** optional systems, especially those with nontrivial idle CPU or RAM. 'Off' must mean **no subsystem instances, no scheduled callbacks, no caches, no subscriptions, no per-world ticking allocations** to the greatest practical extent. Core runtime, sockets, authentication, session timers, player state and protocol-required data cannot simply disappear.

## Three forms of disablement
1. **Build-time `cfg` / Cargo feature:** excludes compilation and dependencies; can shrink binary, reduce attack surface and avoid initialization, but produces multiple distributions/testing combinations.
2. **Startup-time capability profile:** binary includes feature but its constructor, timers, stores, registries and workers are never started when disabled. Best default for most deployments.
3. **Runtime per-world capability:** safe transitions under a validated state machine, possibly requiring a quiescence barrier; not every feature can be dynamically toggled.

## Feature states
`Unavailable` (not compiled) → `Disabled` (not instantiated) → `Starting` → `Enabled` → `Stopping` → `Disabled`; `Failed` with explicit cleanup/rollback. Runtime switches only for features with declared safe transitions. Never permit 'Enabled' without dependencies or in the middle of half-applied state changes.

## Dependency graph examples
- `chat_relay` requires `sessions + play_protocol`, but `chat_signing` adds profile-key management and message-chain rules.
- `players` requires `sessions + movement + world_subscription`.
- `block_interaction` requires `world_mutation + packet_ack + outgoing_block_updates`.
- `block_physics` requires `world_mutation + scheduler`; enabling it in a preexisting mutable world needs a clear initialization policy.
- `mob_ai` requires `entities + navigation + scheduler + world_reads`, but movement handling for **players** never depends on mob AI.
- `redstone` requires `block_neighbor_updates + scheduler` and potentially block entities.
- `disk_persistence` requires storage backend and shutdown drain; disabling it means explicit `ephemeral` behavior.
- `java_bridge` requires IPC/event permissions, but no simulation subsystem should depend on Java being present.

## Four feature profiles to define
**ultra-pvp:** status, auth, players, chunk stream, chat, creative/fly, commands; no world mutation, no mobs, no redstone, no persistence, no weather ticks. **creative-edit:** plus block placement, event callbacks, optional persistence; no survival systems by default. **survival:** enables required physics, entities, inventory, gamerules, persistence etc only when implemented; not a promise of vanilla parity. **lobby:** nearly static world, low view distance, chat/commands and optional cosmetics.

## Resource accounting
Per feature publish: `compiled?`, `enabled?`, instance count, scheduled tasks, resident bytes, optional cache bytes, CPU ns/tick or events/sec, peak activation bytes, disable/cleanup bytes, outstanding tasks. A feature claiming 'off' while showing periodic ticks fails the acceptance test.

## Runtime feature toggle semantics
- **Immediate:** purely observational hooks, simple rule checks with no persisted state, provided no operation is in-flight.
- **Quiesce:** block interaction, scripts, entity AI, plugin listeners; first stop ingress, drain jobs, remove subscriptions, then drop allocations.
- **World reload:** dimension registry, generator, major persistence backing modes.
- **Server restart/build:** protocol version, transport codec, some security mechanisms, fully compiled-out modules.

## Idle behavior specification
When no players and no scheduled operations remain, an ephemeral instance is **parked**: no 20TPS simulation callback, no periodic timer owned by the world, no scanning all chunks, no eviction sweep per instance. Shared runtime may still have coarse timers and bookkeeping. **Near-zero incremental idle CPU**, not mathematically zero total CPU. Compare 0, 1, 10, 100, 1000 parked instances with perf counters.

## Danger: multiplying feature combinations
An unconstrained toggle graph explodes test combinations. Validate predeclared, supported profiles in CI, plus selected pairwise dependency combinations, property-based state transition tests, and a compatibility matrix. Disabled systems cannot make packet structures invalid; client protocol duties are mandatory irrespective of game features.

## Suggested feature descriptor contract (documentation only)
Each feature describes: `id; version; compileAvailability; scope(global/world/player); dependencies; conflicts; default; switchPolicy(immediate/quiesce/reload/restart); memoryBudget; workBudget; eventSubscriptions; allocatedResources; status/telemetry; rollback; persistenceImplications`.

## Initial feature list
See `docs/16_CAPABILITY_CATALOG.md` and `structured/capabilities.csv` for ownership/toggle priority. Make `core_protocol`, `sessions`, `world_registry`, `chunk_stream`, `player_sync` immutable required dependencies. Modules such as mob pathfinding, weather, crop random ticks, redstone, autosave and rich plugin hosting can be truly absent for PvP.

## Failure scenarios to test
Disable AI while it has pending pathfinding; unload world with active Java callback; toggle block physics while packet batches queued; re-enable persistent storage after ephemeral writes; enable chat with mismatched security policy; unload plugin that owns a scheduled callback. Define transaction and rollback semantics for each.
