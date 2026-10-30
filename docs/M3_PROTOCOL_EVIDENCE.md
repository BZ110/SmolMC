# Revised M3 protocol and gameplay evidence

2026-10-30. Wire target: unmodified Java 26.3, protocol 777. At the user's explicit request,
knockback uses a **1.19.4-based arena profile**, not full 26.3 gameplay parity. This extends the M1B–M2C
ledgers. Authentication, encryption, compression, Configuration and shared terrain
retain their existing contracts. The evidence below distinguishes observed reference
facts, local arena policies and what remains unverified.

## Independent evidence and a corrected assumption

Observed reference: the operator-owned `unz/net/minecraft/` inventory is pinned in
[evidence/m3-protocol-sources.json](../evidence/m3-protocol-sources.json). Only paths
and hashes are distributed. Packet classes live under `network/protocol/game/`;
registration order is in `GameProtocols.java`. Clientbound bundle delimiter occupies
slot **0** even though it is registered through `withBundlePacket`, not `addPacket`.

An initial extractor missed this slot, shifting the new IDs by one. The encrypted
fixture initially mirrored the error. Real clients exposed an NBT decode/Java heap
failure. This was a protocol defect, not insufficient client RAM. The same review
found M2C command advertisement used **0x0f**, corrected here to **0x10**. Naive Item
static-field ordinal counting was also wrong: actual registry IDs are iron sword
**1045**, diamond sword **1050**. The historical M2C report has an explicit erratum.

Independent check: [the original test adapter](../reference-tests/README.md) binds
the installed official 26.3 client's codecs and real built-in/dynamic registries.
Each original Rust outbound vector must decode to its expected Java packet class,
consume all bytes and re-encode byte-for-byte. It checks actual item/attribute/
particle IDs, incoming F action and semantic motion-packet decoding. Results and JAR/vector hashes are in
[m3-reference-codecs.json](../evidence/m3-reference-codecs.json). The separate JVM
is capped at 512 MiB. No account files, launcher tokens, Mojang implementation or
assets are copied into the repository. This codec check is independent of the
Rust fixture but does not establish rendering or full gameplay parity.

The separate [classic numeric check](../evidence/m3-classic1194-reference.json)
executes actual Paper **1.19.4 build 550** `Mth` and velocity packet code: 16 yaw
vectors and six velocity quantization vectors. Its original adapter is under
`reference-tests/Classic1194ReferenceCheck.java`. The JAR was obtained from the
[official build metadata](https://fill.papermc.io/v3/projects/paper/versions/1.19.4/builds/550),
SHA-256 verified, and patched locally in patch-only mode. No server was launched,
EULA accepted, plugin loaded, or proprietary artifact redistributed by that check.

Private source inspection also pins official 1.19.4 server/mappings and
[Paper's archived 1.19.4 tree](https://github.com/PaperMC/Paper-archive/tree/3e933fe49d526b6d5e671cb6314ade7e95d69d86),
including its exact CraftBukkit submodule. Base/sprint knockback, primary motion
restoration, default sprint cancellation, yaw arithmetic and serialization were
inspected. No Purpur artifact/source was used. These are source facts, not a claim
that every competitive server uses default Paper settings or no combat plugins.

Historical diagnosis: a 26.3 primitive-motion adapter initially omitted listener
position restoration; further user feedback exposed that omission. A later paired
primitive replay reproduced the 10-tick landing dip but did not instantiate a real
ServerPlayer/network loop. It was insufficient to establish full gameplay feel.
The user rejected the modern profile and selected 1.19.4 as the target. Current
acceptance is the real-client classic-profile feedback plus the explicitly scoped
checks above. [Marked numeric traces](../evidence/m3-knockback-diagnostic.json)
retain the failure rather than treating earlier scalar passes as GUI acceptance.

## Clientbound additions/corrections

Fixed fields are big endian; counts/IDs described as VarInt use that codec.

| ID | Packet / relevant exact fields |
|---|---|
| 0x10 | Commands: existing bounded literal tree; corrected M2C ID |
| 0x12 | ContainerSetContent: container 0, state ID, 46 ItemStacks, cursor |
| 0x19 | DamageEvent: victim, damage-type registry ID, source entity+1, direct entity+1, optional source-position absent |
| 0x22 | EntityEvent: entity i32, event byte 3 (death) |
| 0x2b | HurtAnimation: entity VarInt, hurt yaw f32 |
| 0x30 | LevelParticles: particle VarInt, two booleans, xyz f64, three spread f32, three speed f32, count VarInt, randomization VarInt 0 |
| 0x45 | PlayerCombatKill: victim VarInt, anonymous string-NBT death message |
| 0x54 | Respawn: CommonPlayerSpawnInfo, keep-data byte 0; adventure game mode 2 |
| 0x65 | SetEntityData: entity VarInt, health accessor 9, float serializer 3, health f32, terminator 255 |
| 0x67 | SetEntityMotion: entity VarInt, LP Vec3 |
| 0x68 | SetEquipment: main hand slot 0 with continuation bit, offhand slot 1, default-component ItemStacks |
| 0x6a | SetHealth: health f32, food VarInt 20, saturation f32 5 |
| 0x77 | Sound: inline holder tag 0, sound identifier, optional fixed range absent, PLAYERS category 7, xyz i32 in eighth-block units, volume/pitch f32 1, seed i64 |
| 0x7b | SwingAnimation: entity, hand, type WHACK=1, duration 6 (all VarInt) |
| 0x86 | UpdateAttributes: attack_speed registry ID 5, base 4, sword modifier minecraft:base_attack_speed, amount f32(-2.4) promoted to f64, ADD_VALUE=0 |
| 0x02 | Animate: entity VarInt, action byte 1 (critical particles) |

Particles use actual IDs sweep_attack **77**, damage_indicator **14**. Named inline
sound holders avoid duplicating the sound registry. Attack weak/strong/crit/sweep/
knockback/nodamage sounds reach the attacker and scoped nearby observers. Damage
and death event handling already produces vanilla hurt/death audio; sending extra
sound packets would double it. LP Vec3 uses two low bytes followed by a big-endian
high u32, 15-bit components and scale bits/optional VarInt, not three i16 velocities.
The codec oracle covers these details, including equipment and death-message NBT.

## Serverbound handling

| ID | Supported handling |
|---|---|
| 0x01 | Attack: target entity VarInt |
| 0x0c | ClientCommand action 0: respawn only while dead |
| 0x0d | ClientTickEnd: finish known-movement sample; no new timer |
| 0x12 | ContainerClick: bounded container/state/slot/button/type and changed hashed stacks; predictions discarded |
| 0x13 | ContainerClose: own inventory only; return cursor/resync |
| 0x1a | Interact: target, hand, LP relative location, boolean; supported swords have no player right-click consequence |
| 0x29 | PlayerAction: Drop 4/5 (resync/no drops), Release 6, **Swap offhand 7**; action 6 is not F in 26.3 |
| 0x2a | PlayerCommand: actual START_SPRINTING=1 / STOP_SPRINTING=2, authenticated entity must match |
| 0x2b | PlayerInput: crouch/intent. Sprint-key bit is **not** authoritative sprint state |
| 0x2e | Punch: empty body, visible air swing and charge reset |
| 0x36 | SelectedSlot: i16 0..8 |
| 0x42 / 0x43 | Use-on/use item: exact bounded fields/sequence ACK; sword use has no active ability |

Decoder tests cover truncation, invalid ranges, nonfinite vectors, bounded hash
predictions and trailing bytes. Default-component item stacks contain only
server-created supported IDs; no arbitrary client NBT/component authority.

## Gameplay equations and classic profile policies

`Player.attack`, `getCurrentItemAttackStrengthDelay`, `getAttackStrengthScale`,
`isSweepAttack`, `doSweepAttack`, `causeExtraKnockback`, `LivingEntity.hurtServer`,
`dealDefaultKnockback`, `knockback`, `travelInAir`, `aiStep` and
`ServerGamePacketListenerImpl` movement/input handlers ground the narrow subset.

- Bare hand 1, iron sword 6, diamond sword 7 base damage. Cooldown is
  `clamp((elapsed_ticks + .5) / delay, 0, 1)`; damage multiplier `.2 + .8*charge²`.
  Delay follows the actual base/modifier double-to-float conversion (nominal sword
  speed 1.6, nominal delay 12.5 ticks). Full-strength threshold is strictly >.9.
- Charged falling, airborne, nonsprinting attacks crit for 1.5×. No climbing,
  fluids, mounts or blindness are enabled in this arena. Sprint knockback cancels
  actual sprint state after success; holding the sprint key alone does not restart
  it. A later charged falling attack can therefore crit in the same input sequence.
- Grounded charged sword, nonsprinting/noncritical and horizontal known movement
  below `(float(.1)*2.5)²` sweeps. The primary target receives normal damage.
  Secondary AABBs intersect the primary AABB inflated by (1,.25,1), and attacker
  feet distance² must be <9. Unenchanted sweeping ratio 0 yields `1*charge` damage.
- Fresh hurt applies full damage, then for ten ticks only the difference from a
  larger hit applies. The 20-tick damage cooldown is represented lazily by its
  relevant first ten ticks; no per-player counter task.
- Knockback normalizes the away vector. X/Z become old/2 + direction*power.
  Ground Y is `min(.4, oldY/2 + power)`; airborne Y is unchanged. Base power is
  `.4000000059604645`. Sprint extra .5 is a second stage using attacker yaw.
  Both stages combine before a single primary-player velocity packet; its old
  motion is then restored, as in the reference. Successful sweep secondaries
  receive their extra yaw-directed .4 and retain resulting server motion.
- **Explicit arena consistency policy:** on confirmed flat-roof landing, stale
  downward server motion is settled to `-.08 * float(.98)` before a same-phase hit.
  The captured failure used oldY=-.3015347 and sent Y=.2492326; settled motion yields
  the ordinary Y=.3608000 (classic packet precision .36075). This removes the
  alternating miniature hop at the 10-tick boundary. This is a deliberate local
  contact/timing policy, not a claim of bitwise default Paper behavior. Airborne Y
  and the classic base/sprint equations remain unchanged. The user explicitly
  accepted its resulting real-client feel.
- Server motion is separate from client displacement. Lazy 50ms flat-stone
  advancement applies gravity .08, vertical drag float(.98), horizontal drag
  float(.91) with float(.6) ground friction, vanilla near-zero thresholds, jump
  float(.42), sprint jump .2 and roof collision at y64. Each step restores the accepted position after motion/collision, as the real
  server listener does; it does not accumulate autonomous player displacement.
  Work after long idle is bounded to 256 scalar steps. Accepted movement reanchors the motion position;
  actual world/player ticking tasks are not introduced. Catch-up truncation after
  more than 256 uneventful ticks is a bounded-work arena policy, not an exact
  simulation of every possible long-idle airborne state.
- Classic yaw axes use 1.19.4 float multiplication/indexing into the 65,536-step
  sine table, computed on demand instead of retaining a 256 KiB table. Outgoing
  impulses first use classic truncation at 1/8000 block/tick with ±3.9 bounds, then
  the required protocol-777 LP vector envelope (which adds its own small rounding). Coincident positions use bounded provider RNG
  triangular offsets; after 16 failed draws/provider failure, a finite eastward
  fallback is deliberate. Hurt yaw uses standard atan2 rather than the reference's
  approximation; this is near-equivalent, not bit-for-bit parity.
- Reach uses eye-to-target AABB distance² <=36: vanilla's server interaction range
  plus tolerance. It is not a claim that the vanilla client's normal reach is six.

## Local policies and limits

Opt-in adventure arena; immutable stone roof, deterministic kit, no item drops,
durability, armor, enchantments, shields, teams, hunger, regeneration, fall damage,
fluids, mobs, general collision engine or lag compensation. Movement still has the
existing coordinate/travel envelope; this is not full vanilla anti-cheat. Ground
support is specialized to the intact flat roof. Full server/client physics and
all legal survival interactions remain unverified/deferred. A one-second corpse
visibility deadline permits the vanilla animation before removal; respawn rebuilds
kit/health/terrain/entity visibility through the existing acknowledged switch path.
