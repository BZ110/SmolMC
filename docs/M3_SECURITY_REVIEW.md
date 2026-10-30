# Revised M3 security and failure review

2026-10-30. Goal: authoritative bounded PvP without weakening M1A–M2C trust,
authentication, world isolation, COW storage or replication backpressure.

## Authority and isolation

Authentication/encryption/session verification are unchanged. No offline mode,
credential CLI, proxy identity bypass or account-token handling is added. The
private encrypted test fixture remains test-only. Optional codec checks read the
installed version JAR/libraries/metadata, never account files or launch credentials.

The existing directory serializes actions. Entity lookup is instance-local and
HashMap based; visibility, membership and generation/token checks precede mutation.
Source pose comes from accepted session state; target pose uses the latest validated
slot, independently of 50ms publication. Self/cross-world/dead/unready/out-of-reach
attacks fail. Sweep candidates use chunk occupants plus fresh exact geometry,
without scanning unrelated instances. World transfer retires the old replication
token and clears/rebuilds client interest; queued stale actions cannot cross owners.
Reset/destroy continue to reject occupied worlds. COW terrain/registries are shared.

Inventory is server-owned: 46 bounded slots, fixed kit, supported immutable stacks,
state revision modulo 32768. Changed-slot hashes are parsed with count limits then
discarded, never used as item authority. Stale/unsupported clicks resync. No client
creative injection, arbitrary NBT, item components, enchantments or item-entity
allocation. Inventory/health follows the authenticated player, not another world.
Respawn requests only succeed once for a dead member; living/dead state transitions
and subsequent equipment/chunk/teleport reconciliation stay authoritative.

## Bounds and failure behavior

Existing packet/rate/byte/admission/transform limits remain. Gameplay actions add a
32/s sender budget with burst 8, lazily refilled. Excess closes the offending player.
The directory queue remains bounded at 2048. Per-enabled-player critical delivery
is bounded at 32 messages; packets share immutable payloads across recipients.
Slow delivery closes the consumer rather than dropping health/death/respawn.
Writes retain existing deadlines and bounded send batches.
A synchronized sweep load exposed writer starvation within command batches; the
directory now yields after each gameplay action, preserving queue limits and
movement batching. The final repeat retained all clients and cleaned all state. No unbounded channel,
new per-player/world task, idle scan, per-world JVM or blocking Java callback.
Motion catch-up is at most 256 scalar steps, no work merely because a world exists.

Bridge-off creates no event resources. Bridge-on adds read-only combat snapshots
and noncancellable notices; no damage veto or Java round trip in combat decisions.
Existing authenticated same-OS-user observe/control grants, frame limits, event-ring
128, slow-sidecar disconnect and incarnation/UUID handles remain. Event sequence
and publication share a short optional notification mutex so concurrent trusted
Rust callers cannot reorder sequence numbers. It covers only counter assignment
and bounded in-process broadcast, never catalog/player mutation, IO, Java or await;
movement and Rust-only combat never acquire it. This explicit trade-off preserves
M2C ordering without taking the world catalog lock for each combat notice. A
concurrent publication test checks stream order. Rust public trusted embedding is
not an untrusted-plugin security boundary.

## Evidence and corrected failures

Real clients found an incorrect packet registration offset and M2C command ID
that the self-contained fixture missed. The resulting client NBT allocation failure
was corrected, and an independent official-codec decode/re-encode gate now checks
new outbound formats. F's changed enum, client attack-speed attributes, critical
feedback, corpse lifetime, sweep consequences, attack audio and sprint-key semantics
were separately corrected following user checks. See the protocol ledger and client
evidence; this history is retained rather than claiming initial fixtures were enough.

Regression evidence includes bounded decoder adversarial inputs, inventory authority,
charge/iframes/reach/motion equations, sender budget, encrypted combat/death/respawn/
reconnect/world isolation, three-client sweep, sprint-to-crit sequence, generation
checks and existing slow-viewer/bridge-fault/listener recovery. Platform results and
counts are in the engineering report; automated checks do not establish all GUI
behavior or internet gameplay capacity.

## Remaining limits

The movement envelope is not a complete anti-cheat or collision simulator. Flat-roof
support, voluntary jump motion and client-reported positions constrain the current
arena model; hostile clients may fake plausible movement inside the broad envelope.
No rollback/lag compensation, armor/shield/enchantment/team/survival rules, persistence
or general plugin sandbox. Cooldowns use lazy shared 50ms monotonic ticks; idle
elapsed time counts toward readiness. Audio/particles are scoped to same-world
visibility and distance, not a full vanilla acoustic simulation. These limits are
explicit; expanding them requires later authorization.

The final knockback profile intentionally follows 1.19.4-based equations/yaw/
precision while speaking 26.3 protocol. Confirmed-ground settling prevents stale
falling motion shrinking the next hit; it is documented local arena policy.
Opt-in connection tracing prints bounded numeric hit metadata. Only the exact
user-authorized diagnostic marker records its tick; general chat, names, auth
credentials and packet bodies remain unlogged. Tracing stays off in resource runs.
