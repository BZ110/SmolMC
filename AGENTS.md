# Instructions for an AI engineering agent

**CURRENT TASK STATUS: M4A COMPLETE — USER-ACCEPTED AND VALIDATED (2026-10-30).** Two unmodified 26.3 clients accepted corrected imported terrain/mining/connection stability/combat: “Yes! Everything works. Continue on.” Checkpoint `m4a-before` preserves accepted M3 `3f83f7d` (`m3-pvp-validated`); final milestone tag `m4a-validated`. Bounded modern Anvil/NBT import, shared immutable templates and sparse block/light COW, palette-aware streaming, explicit building/mining timing, local terrain collisions and typed optional Java controls are implemented. The user-generated `USE THIS` save was read without modification; full-height test import uses explicit unsupported-state removal. Full macOS/Linux checks, Java compatibility, actual codec/mechanics validation and short resource comparisons are in `docs/M4A_ENGINEERING_REPORT.md` and `evidence/m4a-*`. Accepted M3 classic knockback, authentication, isolation and optional JVM operation are preserved. No Purpur, per-world/player ticking task, full survival, mobs, redstone/fluid simulation, persistence, plugin host, Bukkit/Paper compatibility, M4B or M5. **STOP: do not begin another milestone without explicit user authorization.**

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
