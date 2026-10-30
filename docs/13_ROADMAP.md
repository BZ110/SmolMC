# 13 — Practical roadmap: defer the ambitious parts

**Authorization (2026-10-29): M2A is accepted following user-reported successful real Minecraft 26.3 client verification. M2B ONLY is explicitly authorized, including implementation, performance/resource measurements, regression and feasible Linux validation, documentation and rollback. M2B is now implemented and validated below. M2C and all later stages remain unauthorized.** Milestone gates are unchanged.

## R0 — Research, architecture, constraints (current deliverable)
Artifacts: source inventory, protocol guide, decisions, feature model, Java SDK contract, benchmarks, risk register. **Gate:** agree on what defines M1 connectivity and version target.

## M1A — Skeleton and status endpoint (Stage 1, implemented and validated)
Concepts: program lifecycle, bounded TCP framing, VarInt/length discipline, protocol state handler, status ping. **Gate:** stock 26.3 client server-list status works and malformed packets do not panic. No worlds.

**Evidence:** user confirmed the stock-client status/ping display and readable login rejection; 19 automated tests pass. See [Stage 1 report](STAGE1_REPORT.md). The historical M1A implementation stops at status; M1B/M1C completion is recorded below.

## M1B — Legitimate login and registry configuration (implemented and validated)
Concepts: online mode session, encryption/compression, configuration registry & tags, known packs, play transition. **Gate:** real client joins play (even if empty) without invalid registry errors. This often takes longer than expected; don't skip it.

**Evidence:** signed-in stock 26.3 client completed auth/configuration and sent valid Play traffic; 42 tests, Clippy and release builds pass on macOS and Alpine Linux. See [M1B engineering report](M1B_ENGINEERING_REPORT.md). The historical M1B empty probe closes after 20 seconds; M1C below replaces it with persistent terrain/flight.

## M1C — Minimal stone world and movement (implemented and validated)
Concepts: virtual stone world, light/heightmap/biome, chunk+light, chunk batches, spawn, teleport confirmation, creative abilities, keepalive, flying and chunk boundary. **Gate:** client can fly over stone without void or hanging.

**Evidence:** signed-in stock 26.3 client rendered stone, flew 50+ blocks across chunks and remained connected; server recorded eight center changes and ten keepalive replies with zero protocol errors/timeouts. 53 automated tests, formatting, Clippy and release build pass on macOS. See [M1C engineering report](M1C_ENGINEERING_REPORT.md) and [protocol ledger](M1C_PROTOCOL_EVIDENCE.md). M1D was subsequently authorized and is recorded below; this remains historical M1C evidence.

## M1D — Multiplayer and messaging (implemented and validated)
Concepts: second player entity, visibility, pose/rotation/movement replication, readable system chat, join/leave cleanup. **Gate:** two clients can see each other and converse, no false secure chat indication.

**Evidence:** two legitimate signed-in stock 26.3 clients confirmed visibility, movement/rotation/crouch, labeled system relay, world/Tab cleanup and reconnect. Final clean trace: four messages/eight deliveries, 17 keepalive replies, zero protocol errors/timeouts. 65 tests, formatting, Clippy, release and listener fault recovery pass on macOS and Alpine Linux. Active post-auth fixture loads and their limits are documented in [M1D engineering report](M1D_ENGINEERING_REPORT.md), with [M1C review](M1D_M1C_REVIEW.md), [protocol ledger](M1D_PROTOCOL_EVIDENCE.md) and ADR-004. This is historical M1D acceptance evidence; current M2A authorization is recorded above.

## M2A — Explicit worlds and isolated instances (accepted following stock-client verification)
Concepts: WorldId and TemplateId, load/reset/destroy, world changes, viewer subscriptions, no cross-instance replication. **Gate:** two identical arenas diverge safely with edits.


**Evidence/status:** [M2A engineering report](M2A_ENGINEERING_REPORT.md),
[protocol evidence](M2A_PROTOCOL_EVIDENCE.md) and
[ADR-005](../decisions/ADR-005-m2a-explicit-instance-ownership.md). All 76 regression
tests, formatting, Clippy and macOS release builds pass; the opt-in resource probe
was run explicitly. Four post-authentication protocol clients verify scoped
visibility/chat/edits and switching. Empty-instance measurements through 4096,
30,000 lifecycle cycles and M1D active-load regression are recorded. Edits are
limited to buried Air/Stone overrides; no surface or arbitrary-template editing.
The user reports all requested real 26.3 client tests passed and explicitly accepts
M2A; [user evidence](../evidence/m2a-stock-client-user-verification.json) records
the specific two-world isolation/relog observations and their provenance.
M2B only is now authorized; stop before M2C.

## M2B — Shared templates and resource profiles (implemented and validated)
Concepts: immutable chunks + sparse overlays, clone/reset, parked idle state, world budgets, feature resolver and metrics. **Gate:** tracked incremental RAM/timer work and correctness under high churn; publish real measurements.


**Evidence:** [M2B engineering report](M2B_ENGINEERING_REPORT.md),
[protocol regression evidence](M2B_PROTOCOL_EVIDENCE.md) and
[ADR-006](../decisions/ADR-006-m2b-shared-cow-overlays-and-budgets.md). All 84 tests,
fmt, Clippy, release/examples and listener recovery pass on macOS and fresh Alpine
Linux with matching source hashes. Empty/modified instances through 4096/1000,
COW clone sharing/divergence, creation/reset latency, retained charges, RSS/CPU/
threads and 34,000 lifecycle cycles are measured. Four protocol clients verify
cloned-world edit isolation; the opt-in dense active/churn probe passes separately.
No world-owned worker/timer, Java, plugin, survival or persistence system is added.
**Stop after M2B; M2C and later require new explicit authorization.**

## M2C — Java SDK bridge (future)
Concepts: sidecar launch/attach, API semantic version, world/player/command APIs, batched edits, event bus. **Gate:** developer writes a tiny Java minigame without Rust changes; bridge-off uses no JVM.

## M3 — Polished plugin/dev ecosystem (future)
Commands, permissions, Javadocs, plugin lifecycle, structured config, errors, scheduling, debugger/telemetry, examples; compatibility matrix. **Gate:** at least three independently written small plugins (chat/staff, arena manager, creative block editor) work without touching core.

## M4 — Optional heavy subsystems (future)
Inventory, actual block interactions, entity physics, redstone, fluid/block ticks, weather, persistence, import/export, survival; each independently budgeted and feature-gated, with correctness regressions checked.

## M5 — Scaling and compatibility (future)
Regionization/parallel heavy world logic based on profiles; additional client versions and possible Bukkit wrappers only when real adoption requires them. Optimize based on measured bottlenecks.

## Why stage order matters
A flawless COW arena system is worthless if 26.3 client registry negotiation fails. A rich Java API with wrong threading guarantees locks in painful incompatibilities. First prove the vertical slice; then optimize and expose stable contracts.
