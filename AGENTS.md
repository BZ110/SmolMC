# Instructions for an AI engineering agent

**CURRENT TASK STATUS: M2B IMPLEMENTED AND VALIDATED; STOP BEFORE M2C (2026-10-29).** M2A was explicitly accepted after user-reported successful stock Minecraft 26.3 verification; exact provenance is in `evidence/m2a-stock-client-user-verification.json`. The accepted M2A Git checkpoint is tag `m2a-accepted`, commit `7a1ac2e3e46cbe17a3097065966f94a14339ab0b`, created before M2B implementation. M2B's gate is verified: shared immutable templates/terrain/registries, sparse chunk COW snapshots, clone/reset/reuse/park, supported profiles/limits, retained-snapshot budgets and measured incremental RAM/idle work/churn. Read `docs/M2B_ENGINEERING_REPORT.md`, `docs/M2B_PROTOCOL_EVIDENCE.md`, ADR-006 and prior M2A/M1D reports. All 84 tests, formatting, Clippy, release/examples and listener recovery pass on macOS and fresh Alpine Linux with matching sources; the opt-in active-resource probe was run explicitly on macOS. Preserve bounded networking/auth, ingress coalescing, generation-safe reserved leave, player/world isolation and no per-idle-world task/thread/tick loop. Do not confuse charged snapshot bytes with RSS or claim leak freedom from short runs. Surface edits/import/persistence remain unsupported. No M2C, Java API/bridge, plugins, survival or later milestone is authorized. Stop after M2B; further implementation requires explicit user authorization.

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
