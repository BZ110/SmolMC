# 15 — Competitive audit: why another Rust server? (29 Oct 2026)

This project is **not** the first attempt at a Rust Minecraft server. Differentiation cannot be "uses Rust" or "has async networking." Reproduce and verify competitors' claims in a pinned test environment rather than assuming any implementation is slow.

| Project | Model / emphasis | Useful precedent | Unsolved opportunity for our design | Evidence / caution |
|---|---|---|---|---|
| Pumpkin | Rust server emphasizing vanilla mechanics, performance and plugin extensions | modular systems; native + WASM plugin architecture | isolated cheap world instances and optional JVM SDK can be first-class product goals rather than add-ons | active evolving project; check version/commit before relying on claims |
| FerrumC | Rust performance-oriented server; currently early creative/minigame work with rewrite plans | custom network / NBT / Anvil, ECS, world import, performance-first philosophy | better specified SDK contracts, lazy world lifecycle and feature residency measurement | project itself warns pre-production, version support differs |
| Valence | Rust/Bevy-ECS framework for building custom Minecraft servers | modular core, packet/schema abstractions, world/entity separation | simpler out-of-box standalone server, fewer mandatory systems, template+overlay isolation, Java sidecar API | early framework; development version support may lag |
| Minestom | Java custom server framework with `InstanceContainer` + `SharedInstance` | explicit world instances; sharing chunks across separate entity sets | copying **immutable templates plus isolated overlays**, not merely sharing mutable chunks | SharedInstance's chunk container remains shared; verify mutation expectations |
| Paper | Highly optimized Java vanilla-derived server with massive plugin API | mature developer experience, production correctness, compatibility | simpler minigame kernel and lean world instancing | unfair to call all Java servers 'horrible'; measure actual bottlenecks |
| Folia | Paper regionized multithreading | clear ownership and multiple schedulers | world-instance ownership and idle pausing without full vanilla simulation | compatibility costs from threading ownership are real |

## Strategic differentiators to defend with evidence
1. **First-class instance lifecycle** `template → create → active → parked → reset/destroy`, with explicit scoped identity and COW isolation, not a static world copy.
2. **'OFF means unallocated' capability graph** with measured, auditable resident bytes and wakeups when a heavyweight feature is disabled.
3. **No forced JVM dependency** for Rust-only servers, *optional* Java SDK/sidecar with bulk operations and clean contract.
4. **MVP vertically complete**: one current vanilla Java client joins, creative flight, visible stone world, chat, two-player presence and worlds; not an ECS/survival engine project at first.
5. **Operational API for 100 instances in one process**; no one-process-per-arena assumption, but honest about shared-process failure domains.

## What NOT to claim without measurements
- "Faster than Paper / Folia / Pumpkin"; "100x instances"; "zero idle RAM/CPU"; "Bukkit-compatible"; "vanilla compliant"; "supports all Java plugins"; "survival parity".
- More threads do not guarantee speed; a cache, palette or serialization algorithm may dominate.
- Memory mapped templates, immutable slabs and pooled buffers are workload-specific, not universal wins.

## Future competitor investigation checklist
Pin exact commit hash, build flags, client version, test machine, server settings, view/simulation distance, auth mode, player movement pattern, world data, and packet rate. For each run measure p50/p95/p99 latency, allocations, resident memory, CPU, GC (where applicable), connections, throughput, errors, plus shutdown cleanup. Separate architectural *claim* from measured outcome.

## Direct sources
- Pumpkin: https://github.com/Pumpkin-MC/Pumpkin
- Pumpkin plugin architecture: https://github.com/Pumpkin-MC/Pumpkin-Docs/blob/master/docs/en/developer/plugins/index.md
- FerrumC: https://github.com/ferrumc-rs/ferrumc
- Valence: https://github.com/valence-rs/valence
- Minestom SharedInstance: https://github.com/Minestom/Minestom/blob/master/src/main/java/net/minestom/server/instance/SharedInstance.java
- Folia overview: https://docs.papermc.io/folia/reference/overview/
