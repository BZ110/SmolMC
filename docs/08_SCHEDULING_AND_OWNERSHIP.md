# 08 — Concurrency, schedulers, ticks and ownership

## Decision: don't make a global tick the architecture
Minecraft traditionally specifies many game mechanics in ticks, often targeting 20 TPS. But a static PvP lobby with no simulated blocks needs **network events and minimal player synchronization**, not a 20Hz full-world pass. Keep protocol deadlines and active-player updates; park idle world simulation.

## Tiered scheduling proposal
**I/O:** async nonblocking network sockets, buffering, compression/encryption, backpressure. **World owner executor:** bounded worker pool to process serialized mutation commands for a given owner/region/instance. **Simulation:** periodic 20TPS tasks *only for worlds/features that require it*. **Background:** chunk import, compression, storage, Java bridge. None should monopolize a global queue.

## Ownership model
- A world/region has a single authoritative writer during its mutation turn.
- Operations across worlds are asynchronous transactions with clear success/failure/timeout outcomes.
- Players may change world but retain stable identity/session; movement authority moves with entity.
- Do not hold ownership locks across `await`, plugin callbacks, disk I/O, or network sends.
- Reads can use snapshots or versioned immutable state; observable staleness must be documented.

## Interaction with Folia
Folia uses independent region ownership and region/entity schedulers; useful precedent but not a direct blueprint. It also shows why legacy synchronous plugin APIs collide with parallel world updates. We should expose native `runOnWorld` / `runOnEntity` / async snapshot operations rather than promising unrestricted world mutation from arbitrary Java threads.

## Active vs idle work
- 0 players, no timers, static template: world is parked.
- 1 flying player: process inbound movement, nearby observers, keepalive; no redstone, weather or mob tick.
- 100 independent games: schedule active worlds fairly with bounded queues, not 100 threads or 100 unconditional full-world loops.
- Long mutation jobs: split/batch over time or reject; never block a network executor.

## Backpressure
Every queue has maximum length and byte budget, including connection send queues, world commands, Java sidecar callbacks, chunk send, storage writeback and metrics. On overflow: prioritize essential events, shed optional telemetry, slow/reject writes, or disconnect abusive peers. No unlimited Tokio channels.

## Time semantics
Distinguish real monotonic time (timeouts, keepalive, metrics) from simulation tick count (game rules, plugin `runLaterTicks`). Paused worlds do not silently advance simulated timers unless profile contract explicitly says they do. A per-world time policy must be explicit for arena countdowns.

## Performance traps
Global concurrent hash map in every packet path; mutex around player registry; lock-held compression; per-player alloc/realloc every movement; unbounded actor-mailbox growth; per-world Tokio runtime; plugin callbacks inside world owner without timeout; overzealous async tasks spawning and wakeups.

## Scheduler tests
Deterministic virtual-time tests; 1000 idle instances do not create 1000 periodic wakes; active instance does not starve others; concurrent teleport/destroy completes cleanly; slow Java sidecar cannot stall network; async storage failure doesn't corrupt world overlay.

## Sources
- https://docs.papermc.io/folia/reference/overview/
- https://docs.papermc.io/folia/reference/region-logic/
- https://docs.papermc.io/paper/dev/folia-support/

## Revised M3 implemented subset

One existing directory serializes combat and inventory; accepted poses remain
session-owned and are captured with generation-checked actions. No Java callback,
network write or await occurs under player mutation locks. Critical delivery uses
a bounded 32-message queue per enabled session and existing connection write
batching/deadlines. Important state is delivered or the slow connection is closed.

A common monotonic epoch supplies lazy 50ms tick indices. Cooldown/iframes do not
require decrement loops. Flat-stone motion is bounded scalar catch-up on existing
movement/combat events; the vanilla listener's post-player-tick position restoration
is part of that subset. Corpse visibility adds a one-second deadline to the existing
session select, with no new task. Empty/parked worlds still own no simulation work.
Optional control-event sequencing uses its own short publication mutex; movement
and bridge-off combat do not touch it. See ADR-009 for scope and trade-offs.
