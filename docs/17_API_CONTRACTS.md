# 17 — API design contracts for Rust and Java SDK

## Public API principles
A rich API is not one giant list of getters. It is reliable semantics (who owns the value, freshness, thread safety, errors, event ordering, permissions) with excellent discoverability, documentation and testing tools.

### Requested Java ergonomics: illustrative, NOT IMPLEMENTATION
The user's original aspiration:

```java
MinecraftRustServer server = new MinecraftRustServer();
server.loadTemplate(someTemplateServerFile);
```

Possible developer-facing design (pseudocode signatures; deliberately not guaranteed compilable):

```java
MinecraftRustServer server = MinecraftRustServer.launch(ServerOptions.defaults());
TemplateHandle template = server.templates().importTemplate(templatePath);
WorldHandle arena = server.worlds().create(WorldSpec.from(template)
    .withProfile("minimal-creative")
    .ephemeral(true));
server.events().on(PlayerJoinEvent.class, e ->
    e.player().teleport(arena.spawn()).whenComplete(...));
server.commands().register("arena", ...);
server.close();
```

The Java handle should represent a **remote Rust process/control socket by default**. `new MinecraftRustServer()` could be a launcher facade, not the whole server running inside a JVM. APIs must make lifecycle and asynchronous semantics explicit.

## Object hierarchy (conceptual)
`ServerHandle` → `Capabilities`, `TemplateCatalog`, `WorldManager`, `PlayerManager`, `EventBus`, `CommandManager`, `Scheduler`, `Metrics`, `PluginManager`, `StorageManager`.

`TemplateHandle` → id, manifest version, dimensions, chunk snapshot access, registration/unregistration, estimated immutable size, template revision.

`WorldHandle` → generation-bound id, template, profile, lifecycle state, player list, spawn, time/weather static settings, edits API (when enabled), park, reset, destroy, statistics.

`PlayerHandle` → stable verified UUID + connection ID + generation; world membership, pose snapshot, game mode/abilities, teleport, message, permissions, kick; no raw `Socket` or mutable pointers.

`ChunkSnapshot` → immutable versioned view. `BlockBatch` → transaction or best-effort semantics explicitly selected; atomicity scope stated. `ChangeReceipt` → success/failure, authoritative version, changed chunk list.

## Error taxonomy
`InvalidState`, `NotReady`, `UnsupportedFeature`, `MissingDependency`, `Unauthorized`, `CapacityExceeded`, `WorldNotFound`, `StaleWorldHandle`, `StalePlayerHandle`, `InvalidPosition`, `OutOfBounds`, `VersionConflict`, `Timeout`, `Backpressure`, `TransportError`, `InternalError`, `ShutdownInProgress`.

## Threading + blocking guarantees
- Async `CompletionStage`/future by default for bridge operations and writes.
- No Java synchronous blocking getters for arbitrary block coordinates in a remote world. Offer `ChunkSnapshot`, `readBlocksBatch` and subscription-based cache.
- Java plugin callbacks dispatched on SDK-controlled worker(s) with bounded queues; exceptions isolated to handler.
- World mutation commands are authoritative only once accepted by the world owner; ordering within one world is defined.
- Synchronous cancellation is optional, rare and costs a bounded response deadline; no unbounded per-player `MoveEvent` round-trip.

## Event semantics specification per event
Each event documents: origin phase; payload fields; mutable?; cancellable?; ordering scope; delivery guarantee (at-most-once/at-least-once); replay behavior; deadline; failure policy; thread executor; tenant/world scope; maximum frequency; batching.

Core v1 candidates: `ServerStarting`, `ServerReady`, `ServerStopping`, `PlayerJoin`, `PlayerQuit`, `PlayerChat`, `PlayerMoveSampled`, `PlayerTeleport`, `WorldCreated`, `WorldParked`, `WorldReset`, `WorldDestroyed`, `TemplateImported`, `CommandInvoked`. **Only PlayerJoin/quit/chat+world lifecycle needed at initial Java bridge milestone.**

## Plugin developer experience checklist
Javadoc for every public type; complete Java examples and Gradle/Maven publishing; CLI doctor to diagnose compatibility; meaningful exception messages; lifecycle simulator; sample arena plugin; strong command API and permissions; stable semantic versioning; migration guides; telemetry per plugin; interactive troubleshooting; clear code-of-conduct and contribution guide.

## Compatibility contract and truth in advertising
"Java API" means Java SDK JARs built **against this project's API** can work with the Rust server. It does **not** mean existing Bukkit, Paper, Spigot, NMS, Fabric or Forge jars run unchanged. Older projects that expect sync `org.bukkit.World#getBlockAt` semantics cannot be emulated cheaply over IPC. Compatibility could be added selectively, with explicit unsupported calls and test plugins.

## Security boundaries
- SDK permissions not inherited implicitly from OS; a developer can supply an authenticated local token.
- Plugins are at minimum fault-isolated from the core if running in sidecar, but not necessarily isolated from each other when sharing a JVM.
- World edit and player-kick capabilities are independently scoped; read-only plugins get read-only credentials.
- Audit asynchronous mutation requests and preserve correlation ID; reject stale generation after arena reset.
