# 13 — Practical roadmap: defer the ambitious parts

M4B is the latest accepted milestone. See the [public roadmap](../ROADMAP.md) and
[testing record](TESTED.md) for the current overview. Checkpoint labels refer to
the same source milestones in the public history. Historical hashes in reports
identify the original validation archive.

**Historical revised M3 authorization (2026-10-29): M1A–M2C and replication optimization are complete (`m2c-validated`, `08fef9a`). The user explicitly replaces the original M3 with M3 — PvP Gameplay Foundation. Checkpoint `m3-pvp-before` preserves M2C. Implement this one vertical slice and stop.**

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
This records historical M2A acceptance; current M2C authorization is above.

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
This is historical M2B completion; current M2C authorization is recorded above.

## M2C — Java SDK bridge (implemented and validated)
Concepts: sidecar launch/attach, API semantic version, world/player/command APIs, batched edits, event bus. **Gate:** developer writes a tiny Java minigame without Rust changes; bridge-off uses no JVM.

**Evidence:** [M2C report](M2C_ENGINEERING_REPORT.md), [bridge/SDK contract](M2C_BRIDGE_CONTRACT.md),
[ADR-008](../decisions/ADR-008-m2c-optional-java-ipc.md) and the compiling
[Java minigame](../java/examples/TinyArena.java). Optional authenticated local IPC,
launch/attach, semantic SDK version, typed lifecycle/player/command controls,
transactional edits, read-only events and bounded failure isolation. Rust-only,
real Java, encrypted Play and Linux evidence are recorded in the report. No M3
plugin loading/permissions framework or later gameplay capabilities. Stop here.

## M3 — PvP Gameplay Foundation (implemented and user-accepted)

Server-authoritative player combat for two authenticated unmodified 26.3 clients:
swings, supported inventory/hotbar/main/off-hand/equipment, swords, validated reach,
cooldowns, health/damage/hurt/knockback, invulnerability, death interface and respawn.
Configurable PvP/spawn and deterministic kit. Minimal nonblocking optional Java
snapshots/events/control extension. No extra 50ms combat buffering; no idle-world
simulation or per-player/world worker. Preserve all M1A–M2C guarantees.

**Gate:** two real vanilla clients equip, swing, fight, take damage/knockback, die,
respawn, continue, remain isolated across worlds and reconnect cleanly. Automated
unit/encrypted Play/failure/bridge regressions and short comparable movement/combat
resource/latency measurements supplement, not replace, that required real-client
acceptance. Document deliberate unsupported vanilla mechanics honestly.

**Evidence (2026-10-30):** [M3 engineering report](M3_ENGINEERING_REPORT.md),
[protocol/gameplay ledger](M3_PROTOCOL_EVIDENCE.md), [security review](M3_SECURITY_REVIEW.md)
and [ADR-009](../decisions/ADR-009-m3-authoritative-arena-pvp.md). The user verified
both stock 26.3 clients and accepted the final 1.19.4-based knockback profile,
including timed consecutive hits. Its deliberate flat-arena contact policy is
recorded explicitly; no Purpur or Paper compatibility framework was introduced.
Short matched movement/combat, full Rust/Java checks and Linux evidence accompany
the final checkpoint `m3-pvp-validated`. Stop here; later milestones remain deferred.

## Deferred original M3 — plugin/dev ecosystem

The old M3 (polished commands/permissions, plugin JAR lifecycle, config/scheduling,
debugger/telemetry and three independently written plugins) is deferred by explicit
user instruction until basic gameplay works. M2C literal commands/SDK remain.

## M4A — Real Worlds, Block Interactions & Terrain Physics (implemented and user-accepted)

2026-10-30 user authorization; checkpoint `m4a-before` preserves accepted M3
`3f83f7d`. Import a bounded, explicitly supported modern Java Anvil/NBT terrain
subset into immutable templates. Preserve sparse COW clone/reset/isolation. Extend
palette/heightmap/biome/light streaming, authoritative opt-in creative-style
placement/breaking and local full-block collision/step foundations. Retain M3's
accepted 1.19.4-based knockback/contact policy and existing flat compatibility path.
Java remains optional and outside movement/combat decisions.

**Gate:** two actual unmodified 26.3 clients render an imported non-flat world,
walk/jump/collide/cross chunks, place/break and observe supported blocks, reset and
isolate COW instances, fight/die/respawn with accepted combat. Automated adversarial
import/protocol/movement/world/Java tests supplement mandatory user acceptance.
Short M3-matched workloads and import/clone/edit/stream/collision measurements,
strict full checks, feasible Linux, source/security/engineering reports and final
commit/tag/rollback required. No unsupported feature may silently corrupt a map.

**Evidence:** [M4A report](M4A_ENGINEERING_REPORT.md),
[terrain/collision contract](M4A_TERRAIN_CONTRACT.md),
[protocol evidence](M4A_PROTOCOL_EVIDENCE.md), [security review](M4A_SECURITY_REVIEW.md)
and [ADR-010](../decisions/ADR-010-m4a-imported-arena-terrain.md). The user accepted
the corrected full-height imported map and held mining; actual runtime oracles,
Rust/Java checks, short measurements and clean Linux evidence accompany the final
checkpoint `m4a-validated`. Subsequent user clarification added narrow sword/tool
mining duration; drops/durability remain deferred. Stop after M4A.

## M4B — Items, Drops, Durability & Armor (complete, user-accepted and validated)

2026-10-30 explicit user authorization, after accepted M4A was committed/tagged.
Checkpoint `m4b-before` preserves `d6371cc` (`m4a-validated`), with a clean tree.
Rust owns bounded world-local dropped items, authoritative counted/component
stacks, supported deterministic block loot, tool/weapon durability, armor slots
and mitigation. Preserve the accepted M3 knockback equations and landing policy,
M4A terrain/mining reconciliation, COW/isolation, bounded queues and optional Java.
Keep the default arena kit/reset policy; item gameplay is explicitly configured.

**Gate:** two actual unmodified 26.3 clients mine/see/collect/drop/merge items,
experience durability/breakage, equip visible armor and fight with mitigation;
isolation/reset/death policies and accepted knockback remain correct. Automated
adversarial/lifecycle/combat/terrain tests, independent actual vanilla codecs and
mechanics, Rust/Java/Linux checks, short M4A-matched resource measurements and an
engineering/security report supplement mandatory real-client acceptance.
These gates passed; the user accepted the final interaction fixes with “Seems good.”
See the [engineering report](M4B_ENGINEERING_REPORT.md), platform/source manifests
and user-reported acceptance evidence. Final checkpoint: `m4b-validated`.

**Deferred:** enchanting/Fortune/Silk Touch/Unbreaking/Mending, crafting/furnaces,
containers, shields, effects, custom attributes, mobs, hunger/regen, persistence,
general physics/fluids/redstone, full survival, Bukkit/Paper and M5. Deterministic
supported loot and static-terrain item physics form a documented bounded subset.
No new external dependency or lockfile change. Roll back in an isolated checkout at
`m4b-before`; ephemeral state needs no persistence migration. Stop after M4B.

## Deferred broader M4 — optional heavy subsystems

Only the narrow inventory/player-combat/death dependencies above move into revised
M3. Full survival inventory/crafting/containers/enchanting, general block interactions beyond M4A,
entities/mobs, general physics, redstone/fluid/random ticks, weather, persistence
and arbitrary import/export beyond the bounded M4A subset remain deferred.

## Deferred M5 — scaling and compatibility

Regionization/parallel heavy simulation, additional client versions and Bukkit/
Paper wrappers remain future scope. M4B is complete; no subsequent milestone is authorized.

## Why stage order matters

The revised order proves playable PvP before expanding the plugin ecosystem.
Protocol correctness, authoritative ownership and measured resource bounds remain
prerequisites; illustrative API sketches are not extra scope.
