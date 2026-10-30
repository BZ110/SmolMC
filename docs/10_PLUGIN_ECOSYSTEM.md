# 10 — Rich plugin support versus Bukkit binary compatibility

## Four compatibility layers: never collapse them
1. **Developer ergonomics:** familiar methods/events/commands, accessible Java docs, quality examples. Target this early.
2. **Source API compatibility:** a selected subset of Bukkit interfaces may be adapted with Java wrappers. Requires licenses, behavioral contracts and careful scheduling semantics.
3. **Binary/class compatibility:** unmodified compiled JARs expect `org.bukkit.*`, `JavaPlugin`, specific method/field signatures and class loader behavior. Harder.
4. **Behavioral/NMS/Paper compatibility:** plugins may use server internals, CraftBukkit, reflection, packet injections, unsafe assumptions and main-thread state. No general guarantee. A 100% faithful implementation conflicts with some optimization goals.

## Recommendation
**Build a dedicated Java SDK with Bukkit-like discoverability**. Add an **optional compatibility extension** for carefully selected APIs only after significant codebase maturity. Publicly describe exact supported interfaces, tested plugins, limitations and safety, rather than 'runs Bukkit JARs'.

## Broad SDK capability checklist
- Lifecycles: load/enable/disable, dependency ordering, clean errors, unloading policies.
- Events: player, world, chunks, blocks, chat, game mode, move/teleport, inventory (future), entities (future).
- Rich commands: argument parsing, completions, registered namespaces, aliases, permission nodes.
- Scheduling: real time vs world ticks vs entity ownership; async support with explicit threading.
- Data: plugin storage, configs, persistent player/world namespaced metadata.
- Text: Adventure-style components and audience messaging.
- World API: template import, clone, reset, chunk snapshot, batched edits, maps, dimensions.
- Players: permission checks, UUID/profile, visibility, cosmetics, scoreboards (future).
- Networking: safe plugin channels and permissioned packet observers (optional; packet interception carries high risk).
- Tooling: Javadoc, IDE completion, sample minigames, plugin SDK version matrix, local simulator.

## Events across an IPC boundary
Bukkit-style synchronous `PlayerMoveEvent` on **every packet** through a JVM will be expensive. Provide optional lightweight native filters, coarse-grained subscriptions (world/region/player), movement sampling, and high-frequency event batching. For truly cancellable low-latency events, benchmark synchronous bridges and define hard timeouts.

## Native, WASM and Java plugin modes
- **Rust extensions**: high performance, weak isolation if loaded into process; ABI strategy critical.
- **WASM modules**: better sandbox model with fuel/time/memory limits; overhead and host API effort; useful for user-provided logic. Pumpkin already documents WASM work—learn from it.
- **Java plugin host**: best Java ecosystem ergonomics, larger fixed memory, IPC semantics; leave off when not needed.
- **Generic network control**: bots/dashboards, not thousands of per-tick operations.

## Compatibility feasibility matrix
| Plugin category | Potential initial support | Risk |
|---|---|---|
| Commands/chat/staff utilities | New SDK: strong | Low–moderate |
| Basic minigames / arenas | New SDK: strong once world/events work | Moderate |
| WorldEdit-like mass edits | New batch API: plausible | Moderate |
| Protocol packet listeners | selected safe hooks | High |
| Economy/permissions bridge | adapter service | Moderate |
| NMS/reflection-dependent plugins | generally not supported | Very high |
| Full Paper event semantics | gradual, selective | High |
| Complex survival / anticheat | major engineering, tuning | Very high |

## Source-based caution
Paper's own docs show how plugins rely on lifecycle/event/scheduler semantics, and Folia needs special schedulers because naive legacy plugin assumptions do not hold with independent region threads. This is a fundamental compatibility constraint, not just missing method names.

Sources: https://docs.papermc.io/paper/dev/how-do-plugins-work/ ; https://docs.papermc.io/paper/dev/api/ ; https://docs.papermc.io/paper/dev/folia-support/
