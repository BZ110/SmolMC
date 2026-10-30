# ADR-009: revised M3 authoritative arena PvP

Date: 2026-10-30. Status: accepted; user-verified revised M3. Authority:
explicit user roadmap adjustment; original M3 plugin ecosystem deferred.
Checkpoint `m3-pvp-before` → accepted M2C `08fef9a`.

## Goal → invariants → source evidence

Two authenticated vanilla 26.3 clients equip swords, swing, fight, take damage/
knockback, die, respawn and continue. Preserve shared immutable terrain/COW,
world membership/generation isolation, bounded networking/auth and ADR-007
replication optimizations. Java is optional and never gates combat resolution.
No world/player task or idle scanning loop. Only necessary player PvP mechanics,
not a full survival simulator or plugin host. Grounding paths and scalar/wire
semantics are recorded in docs/M3_PROTOCOL_EVIDENCE.md; no proprietary bodies.

## Design trade-offs → dependencies

Opt-in `--pvp on` arena uses adventure mode (read-only blocks), vanilla client
movement/gravity and the existing validated travel envelope; default creative
flight remains compatible. Deterministic iron sword in hotbar slot 0 and spare
diamond sword in main inventory. Immutable default-component stacks only; no
client-authoritative creative inventory or arbitrary component/NBT ingestion.
Supported pickup/shift/hotbar/offhand swaps use authoritative state IDs; predicted
client stack hashes never create or edit items. Unsupported operations resync.
Arena kit respawn/no-drops and disabled durability are deliberate local policies;
armor/enchantments/shields/crafting/general survival remain deferred.

The existing single shared event-driven directory owns validated gameplay actions
in each instance's HashMap membership. Per-session Arc-owned state follows an
explicit world transfer; locks are per player, short and never cover IO. Identity/
world/generation checks reject stale queued actions. Attacks read fresh validated
source/target snapshots, not a 50ms delayed movement publication. Recipient sets
reuse the directory's scoped visibility index. No scan of unrelated players.

Combat-critical reliable transitions have a separate bounded per-enabled-player
queue. Sessions flush these promptly, establishing current entity visibility before
entity-addressed packets; no additional 50ms combat buffer. Slow consumers close
and release their membership rather than silently dropping death/health/respawn.
Movement keeps existing coalescing and bounded batches. Empty arenas create no
simulation work; cooldown/iframes derive lazily from 50ms monotonic tick counts.

Costs: per-active-PvP-player inventory/state/critical queue and per-action recipient
work. Per-client encryption/fanout remains. This is not a full server-side collision/
gravity simulation, lag-compensation engine or anti-cheat guarantee. Timing,
resources, event/write latency and remaining deviations must be measured/reported.

## Measurable acceptance → open questions

Unit codec/inventory/timing/reach/knockback tests; encrypted Play kit/swing/damage/
death/respawn/isolation/reconnect; adversarial/concurrent actions and cleanup;
optional Java health/equipment snapshots and nonblocking damage/death/respawn
notifications; old bridge contract compatibility. Short baseline and matched
128/1, 256/16 movement plus active combat measurements; full fmt/Clippy/test/
release/Java and feasible Linux. Mandatory actual two-client acceptance was supplied by the user, including final
classic consecutive-hit feel. The evidence records its provenance and limits;
synthetic fixtures and compilation are supplementary.

Rollback: stop server/sidecars, preserve evidence; isolated worktree at
`m3-pvp-before`, separate target directory, rebuild and rerun M2C checks. Ephemeral
state does not survive restart; no persistence migration. Stop after revised M3.

## Reference-driven refinements

Classify weak/strong/critical/sweep/sprint knockback attacks separately; send named
attack audio to the source as well as scoped observers. Keep vanilla-generated
hurt/death audio in event handling. Sweeps use indexed nearby candidates, exact
AABBs/range, unenchanted secondary damage and both knockback stages. Sprint input
is key intent; actual start/stop commands govern state and successful knockback
cancels sprint. Holding the key does not silently restart authoritative sprinting.

Keep known client movement separate from server motion. The supported stone roof
uses lazy bounded scalar motion advancement (gravity/drag/ground collision/jump)
on existing events, not a new scheduler. Knockback and attack ground predicates use
advanced server support state. Rising departures compare with the previous accepted
client height, preventing a descending packet from spuriously jumping. Independent
original adapters execute actual vanilla Player.aiStep/LivingEntity.knockback/
Player.causeExtraKnockback against a flat test world; vectors include landing and
client reanchors. This verifies the exercised subset, not full survival physics.

Optional notifications share a short publication mutex for sequence+broadcast only;
it never covers world mutation, IO, awaiting or Java. This replaces the insufficient
atomic-number-then-send scheme, whose sequence could reorder under trusted concurrent
Rust embedding. Movement/Rust-only combat do not acquire it. The concurrent event
regression preserves M2C stream ordering; no Java round trip controls a hit.

## Accepted target refinement: classic knockback

User feedback rejected consecutive-hit feel despite earlier primitive checks.
The user explicitly selected 1.19.4/Paper-Spigot behavior and prohibited Purpur.
Protocol remains exactly 777. Use the classic base/sprint equations, historical
float-indexed yaw table arithmetic, primary velocity restoration and 1/8000
truncation (then modern LP encoding). Paper build 550 is independently checked
for yaw/packet quantization; private vanilla/Paper source inventories pin formulas.

Flat-roof contact settling is an explicit local policy: a confirmed landing cannot
carry a stale downward server vector into a same-phase hit. Settle to ordinary
post-contact gravity/drag before resolving that hit; preserve airborne Y. This
eliminates the captured .249 versus .361 grounded height dip without arbitrary
per-hit vertical constants or a new tick task. It is not bitwise Paper simulation.
The user accepted the resulting profile in the real two-client test. No broad
survival/plugin compatibility follows from this gameplay choice.

## Reliable-delivery scheduling correction

A synchronized 32-player sweep workload exposed critical queue saturation when
32 directory commands ran before outbound writers got a turn; 16 clients closed.
Keep the existing capacity of 32 and fail-closed slow-consumer behavior. End a
directory batch after each gameplay action and yield the existing executor, so
writers can drain between actions. Movement retains its 32-command/1ms batching.
No new task, timer, queue expansion or dropped important state. The repeated short
combat case validates connected clients, death/respawn and complete cleanup.
Debug numeric tracing also releases the player-state lock before stderr IO.
