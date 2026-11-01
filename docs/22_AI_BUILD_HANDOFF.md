# 22 — AI coding agent handoff

## Read order before touching any code
`README.md` → `AGENTS.md` → `docs/04_MINIMUM_SERVER.md` → `docs/03_PROTOCOL_777.md` → `docs/20_TEMPLATE_FORMAT_AND_LIFECYCLE.md` → `docs/07_FEATURES_AND_ZERO_IDLE.md` → `docs/13_ROADMAP.md` → `docs/12_VALIDATION.md` → `docs/21_SECURITY_AND_TRUST.md`.

## Current authority

M4B is complete, user-accepted and validated at `m4b-validated`. Accepted M4A
`d6371cc` / `m4a-validated` is retained by rollback checkpoint `m4b-before`.
Native counted items, bounded dropped entities/loot, durability and armor are
implemented. The user confirmed grass/Netherite corrections, then reported rapid
mining/placement and armor-menu rejection. The final reconciliation fixes passed
focused regressions and were accepted after restart: “Seems good.” Read ADR-011
and the M4B engineering/protocol/security reports for exact scope, raw platform
logs, independent reference checks and short resource measurements.

Preserve immutable templates/encoded packets, sparse COW, world generations,
auth/crypto, bounded reliable payloads, optional Java and accepted unarmored M3
knockback/contact equations. Explicit 26.3 Netherite resistance is authorized.
SDK 0.4.0/schema 1.3 adds read-only item snapshots; old 1.0/1.1/1.2 subsets remain.
No full survival/crafting/enchanting/mobs/persistence/general physics/plugin host,
Bukkit/Paper or later milestone. STOP: no next milestone is authorized.
The user authorizes restarting only M4B on port 25568 when a new client check is
ready; do not ask permission again. Source saves remain read-only.

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
