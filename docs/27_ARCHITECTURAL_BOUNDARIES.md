# 27 — Isolation boundaries and the three scales of multi-world hosting

## Scale A — 100 worlds in ONE Rust process
**Primary project goal.** Multiple worlds share network runtime, cache, registries and executable code. Each has scoped game state, player subscriptions, task scheduling and overlay. Efficient but a Rust panic/OS kill/OOM can affect all worlds; process-wide memory allocation still matters.

## Scale B — 100 Rust processes in ONE host
Some users need hard process isolation or independent restarts. Then each process duplicates some core state and transport metadata. A future supervisor could run a pool of worker processes, sharing template files through OS page cache / mapped files and assigning arenas. This is a separate performance/security tradeoff, not the baseline.

## Scale C — Many physical machines
A coordinator/proxy routes players to Rust worker processes; template revisions and player identity are externally replicated. This involves coordination, state transfer, metrics, deployment and auth trust. It is NOT first-stage scope.

## Tenant boundaries
- **World isolation:** default game-state isolation, not hardened security boundary.
- **Plugin fault isolation:** Java sidecar separate process by default, but multiple plugins inside one sidecar share JVM trust.
- **Native Rust plugin:** trusted, in-process extension, capable of crashing entire server; not sandboxed just because written in Rust.
- **WASM:** potential isolation under a separately reviewed sandbox/fuel/memory policy, not automatic protection.
- **Player authentication:** independent of world/game instance; do not let world profile override server identity verification.

## API across boundaries
Within one Rust process, use stable IDs, ownership routing and immutable snapshots. Across Java IPC, use serialized commands/receipts and strongly typed errors. Across Rust worker processes, add authenticated transport and idempotent operation semantics only when multi-process requirements arise.

## Explicit 'thread vs process vs world' myth-busting
A thread isn't a world, a world isn't a process, and disabling game ticks doesn't eliminate network activity. Rust can have many worlds per process without one thread per world, but fairness and shared executor saturation still matter. 100 worlds with 0 players have a fundamentally different resource profile from 100 worlds with 10 players each.

## Practical value proposition
Optimize for **many independent lightweight matches without multiplying expensive generic survival machinery**. Do not trade away correctness, security or developer support simply to produce a lower idle RSS chart.
