# Instructions for an AI engineering agent

**CURRENT TASK STATUS: M2C IMPLEMENTED AND VALIDATED; STOP BEFORE M3 (2026-10-29).** The user authorized M2C only after optimized `d881fe3`. Checkpoint `m2c-before` (`5d6f8d2`) preserves the optimized core and authorization. Read `docs/M2C_ENGINEERING_REPORT.md`, `docs/M2C_BRIDGE_CONTRACT.md` and ADR-008. The optional authenticated Unix IPC/Java 17+ SDK 0.1.0 exposes bounded lifecycle/COW edits/reads, session-safe player transfers/messages, literal commands and cold events. Bridge-off constructs no IPC/event resources and starts no JVM. 100 Rust test executions (94 distinct bodies; six shared bridge checks also compile into the private Play fixture), fmt, Clippy, release/examples, strict real Java integration/faults/encrypted Play/event flood and listener recovery pass on macOS and Alpine Linux. Two default-ignored probes are separately invoked; measurements and JVM/Rust costs remain distinct. Preserve all ADR-005–007 optimized replication/auth/network/COW invariants. No raw pointers/JNI/unsafe, blocking Java hot-path callbacks, per-world JVM/thread/tick or unbounded queue. Java-created worlds survive sidecar loss; reconnect reconciles snapshots; transfers are accepted/coalesced before commit events/client ACKs. Only built-in stone_flat and buried Air/Stone edits, fixed spawn transfers, observe/control OS-user trust domain; no Bukkit/Paper/plugin-loader/permission framework/survival. No M3 or later work is authorized. Stop; further implementation requires explicit authorization.

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
