# Instructions for an AI engineering agent

**CURRENT TASK STATUS: M4B COMPLETE — USER-ACCEPTED AND VALIDATED (2026-10-30).** M4A was user-accepted, committed as `d6371cc`, and tagged `m4a-validated` before M4B began on a clean tree. Rollback tag `m4b-before` preserves it; final milestone tag `m4b-validated` identifies M4B. Native counted/component inventories, bounded world-local dropped items, deterministic supported loot, durability, armor and ordinary inventory interactions are implemented. User testing prompted grass replacement, exact 26.3 Netherite resistance and rapid placement/mining/menu reconciliation fixes; after the corrected server restart, the user confirmed “Seems good.” See `docs/M4B_ENGINEERING_REPORT.md` and `evidence/m4b-*` for actual user-reported acceptance scope, macOS/Linux/reference/Java checks and short resource results. Accepted unarmored M3 equations/landing behavior remain unchanged. Source saves remain read-only; Java is optional. No crafting, mobs, enchanting, full survival, persistence, general redstone/fluids, plugin host, Bukkit/Paper or M5. **STOP: do not begin another milestone without explicit user authorization.**

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
