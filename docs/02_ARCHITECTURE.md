# 02 — Proposed architecture and responsibilities

**Current implementation through M2B:** explicit instance lifecycle, immutable
shared templates/terrain/registries, sparse chunk COW snapshots, clone/reset/park,
validated profiles/budgets and retained-memory metrics. See
[ADR-006](../decisions/ADR-006-m2b-shared-cow-overlays-and-budgets.md) and
[the M2B report](M2B_ENGINEERING_REPORT.md). The diagram remains the long-term
proposal: no simulation workers, Java/plugin bridge, persistence or survival.
M2C and later remain unauthorized.

## High-level ownership tree

```text
Java Edition clients (26.3; protocol 777)
              |
     TCP socket + protocol codec (Rust)
              |
     Connection/session actor (authentication, states)
              |
        Server kernel / supervisor
              |
   +----------+--------------+-------------+
   |                         |             |
 Player/session registry   World manager  Capability manager
   |                         |             |
   |              Templates -> Instances   Subsystem factories
   |                         |             |
 Entity replication   Chunk stream + COW overlays   Plugin event bus
              |                      |             |
        Observability    Persistence interface   Java sidecar / native extensions
```

This is a **responsibility diagram, not module source code**.

## Critical boundary decisions
**Networking:** owns framing, compression, encryption, bounded input queues and protocol transitions. It should never directly mutate arbitrary world state.

**Sessions:** connection ID distinct from authenticated player UUID distinct from entity runtime ID; owns keepalive, control flags, outbound interest/visibility.

**Worlds:** a stable **WorldId** references a running instance; a stable **TemplateId** references immutable chunk data; a **StorageId** references a persistence backend. They are not interchangeable.

**Simulation:** owner-based message passing. A chunk/region/instance has one authoritative writer at a time; readers may observe read-only snapshots. Never hold a global lock during serialization or plugin callback.

**Plugin events:** payloads are immutable snapshots or validated commands, not borrowed Rust internals. Synchronous cancellable events only when the deadline, ownership context, and performance contract permit.

**Java:** **out-of-process JVM sidecar recommended by default**; typed messages over IPC. In-process JVM/JNI/Panama could improve individual call latency but creates memory overhead, crash isolation concerns, unsafe lifetime complexity and potentially poor tail latency. Treat as a later measured optimization, not default.

## Suggested conceptual crates (future names; DO NOT BUILD yet)
- `core`: lifecycle, supervision, identities, metrics; minimum policy.
- `protocol-777`: bounded codecs and state transitions; independent of worlds.
- `auth`: online profile verification and transport crypto.
- `players`: session state and player visibility.
- `world`: templates, instances, overlays, chunk/state contracts.
- `chunks`: palettes, lighting data, chunk packet encoding/cache.
- `runtime`: task/owner routing, schedulers, timers.
- `capabilities`: feature profile resolution, enable/disable and dependency validation.
- `extension-api`: versioned commands/events and permissions.
- `bridge-ipc`: Java sidecar transport, batching, backpressure.
- `storage`: optional backends.
- `server`: thin assembly/composition root.

## Key invariants
1. No entity can be live in two independent worlds simultaneously.
2. Each mutable chunk overlay has one writer at a time.
3. Shared immutable templates cannot be changed by instance mutations.
4. Packet processing cannot trust a client-supplied position, UUID, command, NBT length, compression ratio or chunk subscription.
5. A disabled world feature cannot silently schedule its worker jobs.
6. Java plugins cannot access raw Rust memory or stall a hot networking thread.
7. Clean shutdown waits for or explicitly aborts persistence work with clear outcome reporting.
8. Every resource cache has an owner, budget, and eviction policy.

## Isolation levels
**World isolation:** distinct entities, block modifications, ticks, weather, gamerules, plugin data, visibility (default). **Extension isolation:** separate JVM process optionally per plugin group, capability-based operations. **Failure isolation:** separate processes for truly independent failure domains; a single Rust process with 100 worlds does **not** mean the worlds survive a kernel crash. **Security isolation:** plugins run untrusted only behind an actual sandbox/capability boundary.

## Architecture decision ordering
Protocol correctness → single stone world → second world (isolation) → immutable shared template with mutable overlay → scheduling and metrics → feature profiles → Java bridge → ecosystem breadth.

## Alternative rejected for MVP: one thread per world
Simple initially but creates thread stacks, idle wakeups, oversubscription and poor scaling toward 100+ instances. Use bounded worker pools/owner scheduling instead; no fixed thread per world.

## Alternative deferred: ECS everywhere
An ECS may help large entity simulation but forces complexity on a first milestone with only players. Use simple owner-managed player tables first; benchmark entity-heavy scenarios before migrating.
