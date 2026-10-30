# Instructions for an AI engineering agent

**CURRENT TASK STATUS: REVISED M3 — PvP GAMEPLAY FOUNDATION IMPLEMENTED AND USER-ACCEPTED (2026-10-30).** Explicit user authorization replaced the original plugin-ecosystem M3. Checkpoint `m3-pvp-before` preserves accepted M2C `08fef9a`; final checkpoint is `m3-pvp-validated`. See `docs/M3_ENGINEERING_REPORT.md`, protocol/security reports and ADR-009 for automated/platform/resource evidence and rollback. Two unmodified 26.3 clients were tested by the user; final consecutive-hit knockback feedback was “IT'S SO GOOOOD ty”. At the user's request, gameplay uses a 1.19.4-based knockback profile with an explicit flat-roof landing consistency policy, while wire protocol stays exactly 777. This is not bitwise Paper or full survival simulation. No Purpur was used. Preserve M1A–M2C networking/auth/world COW/isolation and optimized replication. No per-world/player task/tick loop, blocking Java dependency, raw pointers/JNI/unsafe, broad survival/crafting/plugin host or compatibility framework. Original M3 ecosystem and M4/M5 remain deferred. **Stop after revised M3; no later milestone is authorized.**

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
