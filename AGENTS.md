# Instructions for an AI engineering agent

**CURRENT TASK STATUS: FOCUSED REPLICATION CPU OPTIMIZATION VALIDATED; STOP BEFORE M2C (2026-10-29).** The user authorized targeted profiling/CPU optimizations only. Pre-change checkpoint `replication-perf-before` preserves `d046b37`. Read `docs/REPLICATION_CPU_REPORT.md` and ADR-007: bounded ordered write batches, unchanged-roster removal checks, dirty-viewer wake coalescing, complete-view fast path and pre-sized encoders reduce repeated work without changing recipients, wire fields, crypto, pacing or ownership. Three representative short final scenarios pass with matched movement rates; 78 focused tests, fmt, all-target Clippy and release build pass on macOS. The report distinguishes initial diagnostics/client-observer correction, final measured CPU/RSS/fanout/latency, and remaining AES/dense fanout cost. No new Linux/soak/stock-client result is claimed for this focus. Preserve bounded backpressure and cancellation safety; errors/cancellation close the owning viewer session. No M2C, Java/plugins/survival or unrelated work is authorized. Stop; further implementation requires explicit user authorization.

## Mandatory reading order
README → 00_PRODUCT_THESIS → 01_REFERENCE_ARCHIVE_AUDIT → 02_ARCHITECTURE → 03_PROTOCOL_777 → 04_MINIMUM_SERVER → 05_WORLD_INSTANCES → 07_FEATURES_AND_ZERO_IDLE → 09_JAVA_API_AND_BRIDGE → 12_VALIDATION → 13_ROADMAP → 16_CAPABILITY_CATALOG → 18_SOURCES → 21_SECURITY_AND_TRUST → 22_AI_BUILD_HANDOFF.

## Grounding rules
- Target protocol exactly 777 (MC 26.3) initially. No speculative packet IDs, fields, order, or NBT formats. Verify against maintained protocol documentation and, if authorized to inspect, the user-owned reference archive.
- Treat all API examples as **illustrative**, not compiling code or promised signatures.
- Distinguish **observed reference behavior**, **documented third-party behavior**, **design decision**, **hypothesis**, and **unverified question**.
- Never invent performance measurements. List target metrics separately from measured results.
- Do not copy proprietary Mojang source into deliverables. Do not distribute the supplied archive; citations and path-level inventories are sufficient for this phase.
- No unsafe Rust, global locks in hot paths, bespoke cryptography, or JNI shortcuts without a reviewed design decision.
- Do not claim 100% Bukkit compatibility; track API surface separately from binary behavior and plugin compatibility.

## When implementation is explicitly authorized in a future turn
Work strictly in stage order from `docs/13_ROADMAP.md`. Each stage must have: acceptance criteria, test evidence, resource profile, backward compatibility statement, and rollback path. Request clarification only on a genuine blocker; otherwise follow documented assumptions.

## Agent reporting format
For every proposed change: **Goal → invariants → source evidence → design trade-offs → dependency impact → measurable acceptance tests → open questions.**

## Anti-goals
- Do not write a full vanilla simulator for a first login milestone.
- Do not use 1 OS process / 1 OS thread / 1 ticking loop per world.
- Do not allocate complete copies of immutable template chunks per match.
- Do not directly expose Rust pointers to JVM plug-ins or rely on undocumented shared-memory layouts.
- Do not confuse a protocol feature flag sent to the client with our modular engine feature system.
