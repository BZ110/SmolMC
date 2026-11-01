# ADR-011 — Counted items, bounded drops and authoritative input reconciliation

Date: 2026-10-30. Status: accepted and validated at `m4b-validated`.

## Goal

Implement the authorized M4B item/drop/durability/armor slice on accepted M4A
`d6371cc`, preserved by `m4b-before`. Keep native gameplay independent of Java.

## Invariants

Rust owns inventories, cursor stacks, equipment, health, item entities and world
membership. Immutable templates, sparse COW and generation-checked terrain commits
remain. The accepted M3 unarmored knockback equations, precision and contact policy
remain unchanged. The user's later clarification explicitly enables exact 26.3
Netherite resistance on those equations' strength inputs. No per-item/player/world
task, thread, periodic inactive-world scan, or synchronous Java gameplay callback.

## Source evidence

Operator-owned 26.3 classes and installed codecs are the primary observed sources:
`ItemStack`, `ItemEntity`, `ToolMaterial`, `ArmorMaterials`, `CombatRules`,
`LivingEntity.knockback`, `ServerGamePacketListenerImpl.handleContainerClick`,
`ServerPlayerGameMode`, `BlockPlaceContext` and the registered packet codecs.
Original adapters export identifier/component/shape facts and independent numeric
checks; no implementation bodies, game assets or JARs are distributed.
See the M4B protocol ledger and `evidence/m4b-reference.json`.

Vanilla executes menu clicks even when the predicted menu revision differs, then
resynchronizes. Revision mismatch is therefore a reconciliation signal, not an
authorization failure. Counts, components and cursor state always come from Rust.

## Design and trade-offs

- Replace one-item enum values with a compact validated ID/count/damage stack.
  Only the bounded catalog and default components plus damage are supported.
- Derive 416 deterministic single-pool loot rules. Enchantment, random, function
  and multi-pool variants remain explicit deferrals. Harvest-tool checks precede
  loot selection; unsupported loot never becomes a guessed normal block item.
- The existing directory owns each active world's item map, spatial cells and
  viewer edges. One shared deadline set wakes only worlds with actual item or
  delayed interaction work. Last-player departure discards remaining world items
  under this ephemeral arena policy. Reset/destroy still require an empty world.
- Item reservations count against world/shared caps before consuming inventory or
  admitting a loot-producing edit. Motion, merging, pickup and lifetime are bounded.
  Visibility is capped at 256 item entities/viewer, with refill when edges disappear.
- Terrain/lighting remains atomic. A reserved directory receipt applies inventory
  consumption/wear and loot once after commit, independently of socket writes.
  Up to 16 already-admitted inputs can wait behind a player's pending terrain work.
  Contended lighting submissions retry up to 16 times at 20ms intervals; no new
  lighting worker or unbounded queue is introduced.
- Early mining finishes use up to eight bounded server-created completion records
  with original mining start/tool/material facts. They share the directory deadline
  mechanism. A subsequent block can start without losing an earlier valid finish.
  Tool changes, aborts, death, transfers and disconnects cancel appropriate work.
- Armor uses points/toughness and pre-mitigation wear, including breakage before
  reading remaining attributes. Netherite resistance is the sum of actual default
  attribute modifiers and scales base/sprint/sweep strengths by `1 - resistance`.
- Reliable combat results are grouped per action/recipient, preserving packet order
  and the existing 32-message queue. Groups are capped at 128 entries / 64 KiB.
  Item updates also batch within 1024 packets / 64 KiB. Payload/header leases cap
  queued reliable work at 128 KiB/viewer and 8 MiB shared, releasing on write/drop. Transport frame/write limits
  and fail-closed slow-reader handling remain.
- Explicit `arena-kit`, `items-keep` and `items-clear` policies preserve controlled
  kit/reset behavior while allowing real item gameplay. A bounded 1024-identity,
  in-process cache preserves item counts, damage, equipment and cursor on relog.
  Pending commits resolve before the same identity can restore its inventory.
  No persistence or silent cache eviction that refreshes equipment is added.
- SDK 0.4.0/schema 1.3 adds an immutable read-only inventory snapshot. Existing
  schema 1.0/1.1/1.2 peers retain their negotiated versions and operation subsets.

## Dependency impact

No external package or lockfile change. Original Rust/Java/Python adapters and
numeric catalogs use the existing toolchain and dependencies. No unsafe Rust, JNI,
custom cryptography, Bukkit/Paper host or required JVM.

## Measurable acceptance tests

Count conservation, full/partial insertion, merge/lifetime, competing pickup,
world isolation/cleanup, malformed/predicted inventory input, real component
codecs, tool/weapon/armor wear and breakage, armor/toughness/resistance arithmetic,
stale click pipelines, rapid placement/mining, consecutive delayed finishes,
simultaneous placements, disconnect commit receipts, slow viewers and armored
sweep queue bounds. Full prior M3/M4A regressions remain required. Real two-client
observations and short matched release measurements supplement automated gates.

## Open questions and deferrals

Full survival/persistence, arbitrary components, enchanted/random loot, crafting,
containers, drag-crafting, shields/effects, entity fluid/fire interaction and exact
general item physics are not implied. The explicit ephemeral park/death policies
are arena choices. The user confirmed the latest interaction fixes with “Seems
good.” Final platform, reference, client and resource evidence is linked from the
M4B engineering report. Stop here; later milestones need explicit authorization.
