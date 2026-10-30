# 22 — AI coding agent handoff

## Read order before touching any code
`README.md` → `AGENTS.md` → `docs/04_MINIMUM_SERVER.md` → `docs/03_PROTOCOL_777.md` → `docs/20_TEMPLATE_FORMAT_AND_LIFECYCLE.md` → `docs/07_FEATURES_AND_ZERO_IDLE.md` → `docs/13_ROADMAP.md` → `docs/12_VALIDATION.md` → `docs/21_SECURITY_AND_TRUST.md`.

## Current authority

M4A imported arena terrain and interactions are implemented and user-accepted.
Checkpoint `m4a-before` preserves accepted M3 `3f83f7d`; final checkpoint
`m4a-validated` records implementation/evidence. Read the M4A engineering,
terrain-contract, protocol/security reports and ADR-010. No M4B, general survival,
plugin ecosystem, Bukkit/Paper compatibility or M5 is authorized. Stop after M4A.

Preserve immutable imported/virtual templates and encoded packets, sparse block/
light COW, transaction/generation checks, world isolation, accepted M3 classic PvP,
fair reliable replication and optional Java schema 1.0/1.1/1.2 subsets. No unsafe,
per-world/player ticking worker, full physics, fluid/redstone or persistence work.
The explicit removal import mode is lossy; never claim arbitrary map fidelity.
Held mining was specifically added by user clarification; drops and durability
remain deferred. User acceptance provenance and final source/binary hashes are
recorded separately; do not substitute fixtures for GUI evidence.

## Milestone issue contract (copy into task ticket)
- Title, rationale, and preconditions.
- Allowed files and affected layers; explicitly excluded adjacent work.
- Protocol/version ground truth and evidence URLs.
- State transition/invariants; error and security behavior.
- Test cases (positive, negative, adversarial, load).
- Performance evidence if claiming benefit; baseline and budget.
- Deliverables and acceptance checklist.
- Deferred work / exact TODOs without invented implementation.

## Recommended first implementation sequence WHEN AUTHORIZED
1. Status-only server with hardened framing; tested with vanilla 26.3 server list.
2. Online-mode login, encryption/compression, configuration registries: verified real join.
3. Virtual flat world chunk and light data with client movement/flight.
4. Multiplayer presence and simple system chat.
5. Two world instances, join/switch and isolation tests.
6. Immutable template sharing and COW overlay; measure memory and churn.
7. Feature profile resolver and task/resident telemetry.
8. Java sidecar SDK and bulk operations. All later systems remain opt-in.

## Agent honesty checklist
Do not make packet IDs up; do not claim tests passed without running them; do not claim FPS/TPS/memory numbers without reproducible experiment; do not confuse binary Bukkit plugin compatibility with dedicated Java SDK support; do not report "zero memory" for parked worlds; do not let feature switches violate client protocol. When archive files conflict with community wiki, record discrepancy and validate against a real vanilla client.

## Context-management approach
Split work by *bounded deliverable* rather than asking one model to implement the entire project. Preserve decisions in `decisions/` ADRs and open questions in `structured/open_questions.csv`. At each handoff: summary of completed artifacts, explicit state, relevant invariants, test outcome, next single ticket. Avoid loading thousands of decompiled source files into context indiscriminately; choose targeted symbols based on task.
