# 00 — Product thesis, scope, and differentiation

## Problem
Many conventional Java Minecraft server deployments are expensive to operate at high instance counts. Startup, process-per-match overhead, long-lived state, world persistence, plugin assumptions, and global tick workflows complicate free-tier hosting and minigame fleets. But 'Minecraft servers are slow' is **not universally true**: Paper and Folia have substantial optimizations, and existing Rust servers such as Pumpkin already emphasize startup/memory benefits. Benchmark actual workloads before claiming superiority.

## Proposed thesis
A standalone Rust **kernel**, optimized for many independent, often idle, *small worlds* in one process, can achieve a more favorable **memory-per-empty-instance**, **time-to-spawn**, and **CPU-per-active-player** profile than separate full Java processes, while preserving an ergonomic Java SDK for developers.

## Who is it for?
1. PvP/minigame networks creating and destroying dozens or hundreds of arena copies.
2. Low-cost hosts running many isolated worlds for small groups.
3. Creative sandboxes and lobbies with custom entity/game logic managed by plugins.
4. Later: survival, but only after a defined mechanics-completeness plan.

## What makes this different from Pumpkin / FerrumC / Valence / Minestom?
- Pumpkin: Rust, modular crates, multithreaded, rich world formats, plugin support; **major overlapping goals**. Do not misrepresent it as monolithic or idle-heavy.
- FerrumC: experimental Rust creative/server approach; the project's current rework status and 1.21.8 target mean maturity must be verified before comparisons.
- Valence: Rust modular protocol/game framework, useful design precedent.
- Minestom: JVM server library with lightweight instances and shared instance concepts; **already proves the market wants this**, and its API is a strong DX reference.
- Paper/Folia: API ecosystem and region-ownership scheduler concepts, strong baseline performance competitors.

**Differentiation to validate**: truly cheap independent mutable worlds from shared templates; event-driven idle instances; immutable encoded chunk reuse; explicit feature ownership/profiles; a clean Java **bridge** rather than a JVM inside the hot path; straightforward per-instance API and observability.

## Deliverables by stage
**M0 research (this archive):** documented decisions, unknowns, plans, risk register. **M1 runnable** (not yet requested): protocol 777 joins, stone superflat, creative flight, two players, chat and multiple worlds. **M2:** overlays, templates, resource profiling, Java SDK skeleton. **M3:** permissions/events/commands and plugin framework. **Later:** persistence, survival simulation and selected Bukkit adapters.

## Not in first running milestone
Vanilla world generation, inventory correctness, crafting, combat fidelity, fluids, redstone, AI mobs, pathfinding, datapack execution, Bedrock support, Bukkit JAR compatibility, multiple client versions, mod loader compatibility, distributed world simulation, automatic live migration, and hard process sandboxing.

## Product principles
- Features off by default; minimum protocol features always on.
- Per-instance state ownership and lifecycle management.
- No work per tick for an idle inactive instance unless an explicitly enabled feature schedules work.
- Prefer clear small mechanisms first; optimize only with observable metrics.
- No loss of security, identity, or protocol correctness for benchmark wins.
- A Java API should feel as polished as Bukkit's, **not be falsely advertised as Bukkit-compatible**.

## Explicit unanswered questions
Must the server support 26.3 only? Are Mojang account-authenticated connections mandatory at M1? Can players place stone blocks in M1 or only fly? How many concurrent instances and players are target profiles? These are parameter choices, not reasons to block the initial research.
