# RustMC — pre-implementation research and architecture dossier

**Status: RESEARCH ONLY / NO CODE IMPLEMENTED.** Prepared 2026-10-29. Initial target Minecraft Java Edition **26.3 / protocol 777 / world data 5023**; verified against supplied `unz.zip` and independent protocol records. Documentation remains a proposal, not a finished design review or runnable Minecraft server.

## Product thesis
A Rust-native Java Edition server for **cheap independent PvP/minigame/world instances** with stable, developer-friendly Java APIs. Start with one correct minimal creative stone world, two authenticated players, movement, flight, messages, and two distinct worlds. Then build immutable templates, isolated COW overlays, transparent heavy-feature toggles and an optional Java plugin sidecar.

### The crucial distinctions
- **Not a full vanilla/Paper rewrite in M1.** Survival, redstone, mob AI and terrain generation are future optional systems.
- **Rich Java SDK ≠ arbitrary Bukkit/Paper JAR compatibility.** A Java plugin authored for this SDK can be JAR-packaged, but existing plugin binaries depend on Bukkit lifecycle, threading and internals.
- **Shared immutable template ≠ shared mutable world.** Each running arena must have independent entities, subscriptions, overlays and resets.
- **Near-zero idle work ≠ literally zero memory/CPU.** No per-world 20Hz work when parked, but core buffers/network/timers are real.
- **No JVM unless opted in.** Default Rust deployment should not require a JVM; the optional Java bridge has measurable overhead.
- **Process isolation ≠ logical world isolation.** Multiple worlds in one process share crash risk. Hard multi-tenancy needs separate processes.

## Where to start reading
| Need | Read |
|---|---|
| Vision, realistic competitive position | `docs/00_PRODUCT_THESIS.md`, `docs/15_COMPETITIVE_LANDSCAPE.md` |
| Exact upload metadata and legal scope | `docs/01_REFERENCE_ARCHIVE_AUDIT.md`, `reference/archive_audit.json`, `docs/21_SECURITY_AND_TRUST.md` |
| Core architecture and ownership | `docs/02_ARCHITECTURE.md`, `docs/08_SCHEDULING_AND_OWNERSHIP.md`, `docs/27_ARCHITECTURAL_BOUNDARIES.md` |
| How a real 26.3 client connects | `docs/03_PROTOCOL_777.md`, `docs/04_MINIMUM_SERVER.md`, `docs/06_CHUNKS_AND_STREAMING.md` |
| Template world architecture | `docs/05_WORLD_INSTANCES.md`, `docs/20_TEMPLATE_FORMAT_AND_LIFECYCLE.md` |
| Toggle off heavy features and cap resources | `docs/07_FEATURES_AND_ZERO_IDLE.md`, `docs/16_CAPABILITY_CATALOG.md`, `docs/26_CONFIG_OPERATIONS_AND_RESOURCE_BUDGETS.md` |
| Java API and plugins | `docs/09_JAVA_API_AND_BRIDGE.md`, `docs/10_PLUGIN_ECOSYSTEM.md`, `docs/17_API_CONTRACTS.md`, `docs/25_JAVA_SDK_SURFACE_ROADMAP.md` |
| Optimization research and benchmarking | `docs/11_BENCHMARKS.md`, `docs/19_CONCRETE_PERFORMANCE_OPPORTUNITIES.md` |
| Security, QA, risks | `docs/12_VALIDATION.md`, `docs/14_RISK_REGISTER.md`, `docs/21_SECURITY_AND_TRUST.md`, `docs/23_ACCEPTANCE_SCENARIOS.md` |
| Build stages **only when authorized** | `docs/13_ROADMAP.md`, `docs/22_AI_BUILD_HANDOFF.md`, `AGENTS.md` |
| Original sources and decisions | `docs/18_SOURCES.md`, `docs/24_OPEN_DECISIONS_AND_ADRS.md` |

## Pack organization
- `docs/00`–`27`: 28 thematic Markdown research chapters.
- `structured/`: capabilities CSV, feature profiles JSON, benchmarks CSV, milestones CSV, questions CSV for AI review and planning.
- `templates/`: ADR, future implementation-ticket and benchmark-report *documentation* templates.
- `reference/`: metadata and selected **file paths** from supplied archive; **no Mojang code or assets**.
- `AGENTS.md`: future AI contributor instructions, explicitly forbidding implementation until user says to begin.

## The minimal milestone (do not expand casually)
An unmodified Java Edition 26.3 client connects securely and renders a valid flat stone dimension; creative flight, movement and messages work, two clients see each other, and the server supports two distinct world identities. That is already a meaningful achievement. Complex survival mechanics, Bukkit binary compatibility, world editing and runtime plugin hosting are *not prerequisites*.

## Benchmarks and honesty
The project has **no benchmarks** and contains **no implementation**. COW, shared compression, bounded schedulers and feature-off residency are hypotheses that may help. The dossier defines experiments and failure cases to validate them; no performance improvement is guaranteed.

## Provenance
The original user-provided `unz.zip` is external to this pack. Its `version.json` reports Java Edition 26.3, protocol 777, Java 25. Research inventories only metadata, class paths and high-level observations; file contents are not redistributed. Public references and licensing caveats: `docs/18_SOURCES.md`.
