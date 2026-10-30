# 25 — Bukkit-grade developer support without misleading compatibility claims

## Define "lots of support" correctly
A successful ecosystem is more than a massive API method count. It needs stable behavior, examples, debugging, dependency/version management, event contracts, extensible permissions, tools, tutorials, migration guides, and release discipline. The *long-term surface* below is ambitious; only minimal control APIs and player events are authorized for the future first Java milestone.

**Status key:** `M1` needed for Rust kernel, `J1` candidate first Java SDK milestone, `J2+` later SDK, `Future` mechanics not yet implemented. These labels are **design targets**, not existing functionality.

| API family | Conceptual operations | Earliest relevant stage | Correctness/compatibility warning |
|---|---|---|---|
| Server lifecycle | launch/attach, stop, version, capability negotiation, health | J1 | Java launcher spawns Rust; doesn't mean Rust runs on JVM |
| Worlds | create, list, get, reset, destroy, park, move player, set spawn | J1 | future/handle semantics and generation stamps |
| Templates | import virtual/compiled, resolve revision, list, apply manifest | J1 | reference consistency; user file validation |
| Players | UUID, name, world, game mode, flight, teleport, messaging, kick | J1 | verified ID, world-scoped session ownership |
| Chat/Audience | plain text, Adventure-style rich components (future), broadcast | J1/J2 | secure signed chat semantics cannot be faked |
| Events | subscribe/unsubscribe join/leave/chat/world; later block, inventory, entity, movement | J1/J2 | ordering, batching, loss and deadlines per type |
| Commands | named registration, aliases, parsing, suggestions, execution, permission | J1 | ambiguity resolution and safe namespace conflicts |
| Permissions | subject/world/command scoped nodes, inherited groups, audit decisions | J2 | no implicit superuser from Java process |
| Schedulers | once, delayed, repeating, world owner, player owner, async | J2 | world may be parked, entity can change owner |
| Chunks | immutable snapshots, prefetch, unload observer, load-state | J2 | no sync per-block IPC getter |
| Blocks | batch read/write, changeset, conflict policy, undo log | J2 | no cross-instance leaks or main-thread assumptions |
| Metadata | plugin-keyed persistent world/player data | J2 | explicit storage backend and serialization rules |
| Registries | blocks, biomes, dimensions, item IDs, effects | J2 | versioned; update protocol carefully |
| Maps/regions | area/point queries, spatial subscriptions, bounding checks | J2 | world coordinate floor division and overflow |
| Player UI | scoreboards, boss bars, titles, actionbar, tab list, sounds | Future | client packets and capability flags required |
| Inventory/items | stacks, containers, click callbacks, equipment | Future | requires inventory state machine and validation |
| Entity components | spawn, despawn, position, effects, custom data | Future | requires entity system; no fake vanilla parity |
| Combat/PvP | hit events, damage, knockback, cooldown | Future | security and client prediction |
| Physics/redstone | neighbor events, block update scheduler | Future | high complexity, high idle cost if enabled |
| Custom payloads | namespaced plugin channels, selected interceptors | Future | proxy spoofing and packet trust risks |
| Persistence | snapshot, journal, migration, restore, backup | Future | transaction and crash-consistency contract |
| Metrics | server/world/player/plugin memory and CPU, event queues | J1/J2 | observability overhead scoped |
| Admin/control | create/remove arenas, user bans, permission administration | J2 | least privilege and audit |
| Testing | ephemeral virtual test world, simulated join, clock control | J2 | simulator isn't proof vanilla client works |

## Detailed Java event contract examples
- `PlayerJoin`: emitted after verified auth and world join readiness; `PlayerHandle` valid; optional warmup event must be separate. Subscriber exception cannot abort login unless specifically declared.
- `PlayerQuit`: terminal for this connection; session handle becomes invalid after generation boundary.
- `PlayerMoveSampled`: explicitly sampled/coalesced and non-cancellable by default. "Exactly Bukkit's every movement event" is neither cheap nor required for first SDK.
- `BlockChangeRequest`: can be cancelable **only if** a same-owner native hook decides before mutation or bridge deadline semantics are explicitly implemented. Do not promise synchronous cancellation through unreliable IPC.
- `WorldReset`: remove stale callbacks; all pre-reset handles expire. Semantics must be observable and migration path documented.

## Plugin packaging and lifecycle
Java SDK plugin = new JAR intended for this server; load using a Java plugin-host process. Metadata: plugin ID, version, SDK API range, main entry class, declared dependencies, requested capability permissions, events/commands, configuration version, startup order and shutdown policy. Distinguish `loaded`, `enabled`, `disabled`, `faulted`; plugin unload must revoke listeners/tasks/tokens. Kotlin/Scala/Groovy possible by compiling against JVM SDK but not automatically compatible without testing.

## Experience promises worth enforcing
- Small runnable Java starter example **when coding authorized** (not included in this documentation ZIP).
- Generated Javadoc pages, Gradle/Maven coordinates, supported Java versions, API compatibility matrix.
- Migration/adaptor recipes from common Bukkit concepts (`World`, `Player`, `BukkitScheduler`, events, commands) to explicit asynchronous semantics, not magic binaries.
- Release notes for behavioral changes, deprecation windows, semantic versioning, examples CI.
- Tooling for introspecting active feature profile and why API calls are unsupported in that world.
- Error codes with docs, latency/retry guidance, cancellation/deadline controls, logged correlation IDs.

## Why not just expose Bukkit over IPC?
Existing Bukkit plugins often make enormous numbers of synchronous `getBlockAt`, inventory, event and entity calls. A cross-process call per read would be catastrophic; forcing IPC to emulate everything would be slower and ambiguous. Instead, support *bulk snapshots* and *world-owner commands*, and identify a **small, curated Bukkit compatibility tier** if the product eventually proves demand. Java API breadth is a long-run target; vanilla/Paper semantics are not promised.
