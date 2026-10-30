# 09 — Java API and runtime bridge: powerful without contaminating the Rust hot path

## Desired Java experience
The user proposed `MinecraftRustServer server = new MinecraftRustServer(); server.loadTemplate(someTemplateServerFile);` — **good usability target**, but actual startup, native process management and asynchronous operations must be modeled precisely. For instance: is the Java object a *server launcher*, *client control handle*, or *in-process JVM wrapper*? Those are distinct contracts.

## Recommended model: Rust core + Java SDK + optional Java sidecar
- Rust owns networking, world state, event timing, player simulation, chunk encoding and security.
- A versioned Java SDK controls the Rust server via typed IPC (local domain sockets / named pipes; remote authenticated transport optional later).
- Java objects are **proxies/handles**, not Rust objects. Stable IDs + version stamps prevent stale handles after world reset.
- A separate Java **plugin host** may load plugins and register commands/listeners with the Rust core. Running it costs JVM memory **only when Java integration is enabled**.
- Operators running no Java plugins never launch a JVM. This is central to the feature-toggle promise.

## Why sidecar before JNI / Panama?
| Approach | Benefits | Costs / risks | Recommendation |
|---|---|---|---|
| Java SDK → local IPC → Rust core | process isolation; no JVM for Rust-only deployment; clear versioned protocol; simple recovery | serialization/latency; sidecar RAM; cross-process calls | **Default design** |
| Java + JNI → Rust native library | lower call overhead; synchronous façade possible | JNI safety/lifetime issues, core crash on native faults, JVM always running, blocking callbacks | Study later |
| Java FFM/Panama → native Rust ABI | modern native access, less JNI glue in some cases | ownership, Java version, raw ABI contracts, safety and deployment | Study later |
| Embed JVM from Rust | maybe ergonomic existing Java plugin loading | increases idle footprint and crash coupling; difficult runtime ownership | Not first choice |
| Generic HTTP REST for high-frequency events | easy integration | encoding overhead and latency; unsuitable per-block sync calls | Management only |

**Key caveat:** if users demand a *single JVM with sub-millisecond synchronous Bukkit method calls for each block*, IPC is a poor fit. The API must **batch** and expose snapshots or explicit asynchronous operations. A future in-process adapter may serve exceptional requirements after benchmarking.

## Illustrative **non-executable** API sketch
```
MinecraftRustServer server = MinecraftRustServer.launch(config);
TemplateId stone = server.templates().importFrom("arena-template");
WorldHandle arena = server.worlds().create(
    WorldSpec.builder().template(stone).profile("ultra-pvp").ephemeral().build()
);
server.players().onJoin(join -> arena.addPlayer(join.player()));
server.start();
```
These calls describe **developer experience only**. Names, return types, ownership and blocking semantics are open design decisions, not working Java code or guaranteed API signatures. The user's `new MinecraftRustServer()` form may still be offered via a builder or launcher abstraction.

## Java SDK major surfaces (Bukkit-like breadth of developer support)
**Server:** start/stop, status, version/capabilities, operator commands, profiles. **World:** templates, create/reset/destroy, block snapshots/edits, chunk subscriptions, teleport, spawn, borders/time. **Players:** UUID, identity, connection properties, game mode, creative/flying, messaging, teleport. **Events:** join/quit/chat/movement/block events; cancellable only with well-defined sync deadlines. **Commands:** parsing, completions, permissions, help. **Scheduling:** real-time, world-tick, entity owner, async I/O. **Permissions:** pluggable node/subject checks. **Components:** rich text, translation keys, colors and hover/click events. **Registries:** IDs, block-state data, profiles. **Metrics:** resource usage, instance-level stats. **Storage:** plugin-scoped key-value or per-world persistent container. **Test APIs:** spawn simulator, virtual clock, world reset.

## Object semantics and error contracts
- A proxy handle is `serverId/worldId/playerId + generation` and may expire.
- `getBlock` should not be a blocking cross-process call per coordinate. Provide bulk `getSectionSnapshot`, `readRegion`, `setBlocksBatch` and local SDK snapshot cache with explicit version.
- `setBlock` replies with receipt or future; no claim of immediate global visibility unless the contract defines a barrier.
- The SDK emits explicit `UnsupportedFeature`, `StaleHandle`, `WorldNotReady`, `PermissionDenied`, `Backpressure`, `Timeout`, `ConflictedVersion` errors.
- All callbacks have owner, deadline, cancellation, scope and order semantics.

## Bridge message contract proposal
A binary schema with explicit `schemaMajor/minor`, correlation IDs, request ID, world ID, bounded list length, message kind, deadlines, status code and tracing fields. Compare Protobuf, FlatBuffers, Cap'n Proto or custom simple framing **with measured benchmarks**, not subjective preference. Control/event channels should be separated from large chunk/mesh bulk paths; never send raw Rust pointers.

## Events and plugins
**Fast read-only events** can be streamed asynchronously with snapshot payloads; **cancellable events** require the server to wait for a response within a strict budget, which can stall gameplay. Use a small budget and opt-in; fail-open/closed per event policy. Offer command-based mutation as default, and batching for repetitive edits. Never block all worlds because one Java plugin is unresponsive.

## Java plugin loading / packaging
A Java plugin host could support `plugin.yml`-inspired metadata or a new `plugin.json`/manifest. Plugins declare API compatibility and capabilities; Java SDK semantic versioning plus stable IPC version. Each plugin has lifecycle callbacks and scoped resources. **A plugin written against this SDK can be JAR-packaged** without being compatible with unmodified Bukkit/Paper JARs.

## Security
Authenticated local sockets, least privilege, plugin capability permissions, resource/time quotas, exception isolation, and audit logging; sidecar restarts do not kill Rust gameplay if plugins are optional. Do not expose an unauthenticated localhost control API by default.

## Hard acceptance tests
- With Java bridge OFF, no JVM process is started; no bridge listener, per-world callback or SDK cache is initialized.
- Java can launch/connect, create two template worlds, teleport player, batch-edit 1000 blocks and receive events without stale state.
- Kill sidecar mid-task: Rust remains responsive; cancellation and pending request outcomes are defined.
- Sustained event flood cannot exhaust Rust heap or starve player movement.
- Java API error messages and docs are good enough to implement a simple minigame without editing Rust.
