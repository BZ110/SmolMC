# ADR-010 — bounded imported arena terrain and interactions

2026-10-30. Status: implemented; user accepted corrected gameplay, final checks recorded
in the M4A report. Authority: explicit M4A request and subsequent mining-duration
clarification. Checkpoint `m4a-before` preserves accepted M3 `3f83f7d`.

## Goal → invariants

Render a real operator-owned non-flat 26.3 world, collide with supported terrain,
place and mine supported blocks, preserve accepted PvP and independent COW arenas.
Keep immutable decoded/encoded content shared, sparse overlays versioned, queues/
work/memory bounded, Java optional and outside movement/combat decisions. No world/
player worker or ticking loop; no full survival, persistence, plugin host or Purpur.

## Source evidence

Observed private 26.3 `SerializableChunkData`, `RegionFile`, `BlockState`, `ListTag`,
`Strategy`, `PalettedContainer`, `LevelChunkSection`, `Direction`, `Entity`,
`ServerPlayerGameMode`, `Player`, tool properties and packet registrations. Actual
runtime adapters independently compare disk/network cells, packet re-encoding,
mining enums and arithmetic, full-block collision, and historical M3 vectors.
Inventories/hashes and the scope of each check are in M4A_PROTOCOL_EVIDENCE.md.
No Mojang implementation bodies, JARs or assets are included.

## Design trade-offs

Operator startup import reads a selected aligned volume, exact data version 5023,
fully generated Overworld chunks only. Default strict mode rejects unsupported
states. Explicit `--import-remove-unsupported` removes unsupported cells to air
and reports affected palette entries; block entities are not executed/preserved.
Full-height import is preferred: an early Y crop exposed terrain holes and failed
user acceptance. The original save remains read-only. This is an arena conversion
subset, not lossless arbitrary Anvil compatibility or a data fixer.

Retain palette-packed immutable sections and pre-encoded coordinate-independent
chunk suffixes and shared coordinate-bearing packets once per template. Virtual stone remains a separate compatibility
path and is not allocated for imported worlds. Instances share the template and
sparse chunk/light snapshots. Reset drops their overlays. Block edits and skylight
publish together after a generation/snapshot check; an old completion cannot update
another incarnation or send corrections after a player changes worlds.

Initial static skylight is rebuilt for the selected volume. Dynamic edits relight
a bounded neighborhood in one lazy shared worker, outside the networking reactor.
Its bounded queue has eight pending slots, one in-flight job, and at most one pending
transaction per world. Busy/invalid/over-budget work fails without mutation.
No dedicated thread/task per world/player and no timer when worlds are empty.
This small static zero-emission model does not claim full vanilla light-shape,
fluids, emissive blocks or neighbor mechanics. Costs/limits are measured explicitly.

Collision queries inspect nearby verified static boxes, resolve Y then the larger
horizontal axis, and support a bounded 0.6 step. Ordinary queries use a fixed stack
buffer; larger queries have a bounded fallback. Airborne/grounded state comes from
confirmed geometry, not predicted next-step landing or the client flag. Imported
motion uses local collision while preserving the accepted classic knockback
functions/precision/contact settling. The virtual-flat path remains unchanged.
Join/reconnect, transfer, respawn and void recovery use a clear supported point
from the current edited snapshot, searching at most 81 nearby columns when the
configured point is blocked. A fully blocked neighborhood fails explicitly.
This is a collision foundation and broad movement envelope, not a new anti-cheat.

`build` is explicit and separate from trusted `editable` and `read-only` profiles.
Placement uses server kit items, reach/interest, air target, occupancy, budgets and
world generation. User clarification added held mining with actual hardness/tool
progress, default airborne penalty and matching supported client mining tags.
Start/direction/abort/finish are 0/1/2/3 in 26.3; an initial finish-enum mistake was
caught by the user and now has an independent actual-codec regression. Swords mine
at their own rates. Drops/durability/harvesting/crafting remain deferred.

The lazy mining model resolves upon validated finish requests, with vanilla's 0.7
finish tolerance; premature finishes reconcile instead of scheduling vanilla's
full delayed-destroy tick path. Crack updates piggyback on existing actions. No
per-player mining task is created. The accepted scope is documented without
claiming every survival/tool/effect circumstance.

Movement/combat read immutable watch snapshots without taking the world catalog
lock. Block/lifecycle mutations use the existing short catalog owner. Java 0.3.0 /
schema 1.2 adds configured-template discovery, typed state reads and bounded atomic
edits; imports stay operator-configured, with no filesystem path exposed over IPC.
Old 1.0/1.1 subsets remain; unsupported legacy material reads fail explicitly.

## Dependency impact

World directly uses existing workspace flate2; no new external package. No custom
cryptography, unsafe Rust, JNI or required JVM. A bounded client-side receiver policy
lets private probes receive legal server chunk payloads. Production server ingress
retains its 64 KiB/128x limits, exact-stream checks and work/byte budgets.

## Measurable acceptance → open questions

Full suite/Clippy/fmt/release/Java/clean Linux; actual codec/mechanics oracles; two
real-client user acceptance; short M3-matched movement/combat with import, memory,
clone/reset/edit/stream/collision measurements. No internet-capacity inference.
The engineering report records outcomes, corrected failures, deviations and rollback.
Future arbitrary-map fidelity, full lighting/neighbor rules, mining effects, drops,
durability, general survival, additional templates at runtime and later milestones
require new authorization. Stop after M4A.
