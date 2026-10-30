# 24 — Architecture decisions, alternatives, and uncertainties

The table below contains historical **proposals**. M1A decisions are recorded in [ADR-001](../decisions/ADR-001-stage1-status-boundary.md). The explicitly authorized M1B implementation is recorded in [ADR-002](../decisions/ADR-002-m1b-transport-authentication.md). M1C implementation is recorded in [ADR-003](../decisions/ADR-003-m1c-immutable-stone-stream.md). M1D is recorded in [ADR-004](../decisions/ADR-004-m1d-presence-and-system-relay.md). M2A and later proposals remain unauthorized.

| Decision | Leading proposal | Competing choice | Why it matters | Trigger to reconsider |
|---|---|---|---|---|
| Scope | static creative stone world first | survival parity first | avoids years of mechanics before login works | real users demand survival MVP |
| Client | Java Edition 26.3 protocol 777 | multi-version/Bedrock | network/registry scope explosion | stable live 26.3 verified |
| World data | virtual template + COW overlay | full duplicated Anvil worlds | cheap arenas / fast reset | dense write workloads defeat COW |
| Scheduler | event-driven owner executor | thread per world / one global 20TPS loop | idle cost and scaling | measured unfairness or CPU contention |
| Java | sidecar with typed IPC | embedded JVM/JNI/FFM | Rust-only deployments avoid JVM, clear isolation | tight sync bridge requirements are validated |
| API compatibility | rich original Java SDK | binary Bukkit emulation initially | ambitious compatibility impedes lean engine | SDK ecosystem proven, demand measurable |
| Plugins | in-process trusted Rust / optional Java sidecar, WASM later | Bukkit loader in Rust | performance vs security | developer demand + threat model |
| Persistence | ephemeral first, optional snapshots later | autosave always on | no idle I/O for minigames | durable worlds become default product |
| Registries | pinned versioned canonical data pipeline | hard-coded IDs from memory | protocol correctness | tested data source is established |
| Visibility | world-scoped chunk interest index | broadcast to all | avoids O(P²) and cross-arena leaks | low player counts make simplicity attractive |
| Feature switches | startup disable + a few runtime policies | universal live toggling | avoids invalid midgame state | clean quiescence semantics proven |

## Open questions to answer before first code milestone
1. Should this be called an engine/framework or a deployable standalone server with an SDK? (Proposed: standalone server + SDK.)
2. Is **online-mode directly** required for M1? (Proposed: yes. A dev-only offline harness should never be public.)
3. Do world instances need crash durability initially? (Proposed: no, explicitly ephemeral.)
4. Do we need player block placement in M1? (Proposed: no; read-only creative fly for first connection. Add edit immediately after.)
5. Does "Java support" mean SDK plugin JARs or unmodified Bukkit JARs? (Proposed: SDK first; binary compatibility deferred.)
6. Which host OS is first production target? (Proposed: Linux x86_64, also support macOS development.)
7. Can default world be infinite stone or finite arena? (Proposed: procedural infinite flat world with bounded chunk subscription.)
8. Max `view-distance` or concurrent player target? (Set after measuring, not guessing.)
9. Should Java SDK start a child Rust process or attach to an existing instance? (Proposed: support both through explicit launcher/connector.)
10. How to package redistributable block registries and avoid proprietary data? (Requires license/technical review.)

## Rejected premature bets
Custom transport stack; aggressive unsafe codec rewrites; mandatory Wasmtime at launch; universal ECS; exact vanilla survival; Bukkit binary compatibility; per-world dedicated OS thread; one process per minigame; automatic runtime hot-toggle of registry/protocol-critical components.
