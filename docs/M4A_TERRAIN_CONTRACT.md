# M4A terrain, import and interaction contract

Minecraft Java 26.3 / protocol 777 / data version 5023. These are implemented
subset contracts; research documents and older milestone limits are historical.

## Import and supported states

```sh
target/release/smolmc --import-world '/path/to/world' \
  --import-bounds '-16,-64,-80,48,320,-16' --world-profile build --pvp on
```

Import bounds are inclusive minima/exclusive maxima, aligned to chunks/sections
(16 blocks). The example is a 64×64 full-height volume; use bounds appropriate to
the source. No generation, entities, datapacks or scripts run. Input is read-only.
Overworld region locations are `dimensions/minecraft/overworld/region` (26.3) or
legacy `region`. Exact fully generated chunks required; missing chunks fail.
Gzip/zlib/raw region compression is supported; external/LZ4/custom streams fail.
No data-version migration, arbitrary dimensions or arbitrary Anvil compatibility.

The checked catalog contains 4,809 supported states / 447 block names. It includes
common opaque terrain, ores, planks/logs/leaves, wool/concrete/terracotta, glass,
selected passable plants, slabs/stairs and snow shapes. The exact allowlist,
properties, state/item IDs, collision boxes and hardness facts are in
`crates/world/src/block_catalog.rs`; default-state placement is narrower than
import. No fluid, block entity, light-emitting or unverified shape states.
Non-default/invalid properties fail strict import. Unsupported biome names fail.
Negative Y and floor-divided negative chunk coordinates are supported.

Default is strict. Optional `--import-remove-unsupported` is a **lossy arena
conversion**: unsupported palette entries become air and are listed in startup
output; their occurrence counts are palette entries, not block-cell counts.
Non-Minecraft namespaces fail; block-entity data is omitted, never executed.
Source structure blocks/positions within the supported subset remain exact.
The accepted `USE THIS` test uses full height and explicit conversion, including
removed fluids and luminous cave decorations. This is not a lossless replica.
Y cropping can expose voids; full height avoids removing lower ground.

## Bounds and ownership

| Resource | Bound |
|---|---|
| Selected chunks | 256 |
| Retained decoded template policy charge | 64 MiB |
| Encoded shared template suffixes | 32 MiB total |
| Region file | regular non-symlink, 8 KiB–64 MiB, sector aligned |
| Selected compressed input | 128 MiB total; each chunk ≤2 MiB |
| Decoded chunk NBT | 2 MiB |
| NBT | depth 32, 131,072 nodes, string 4,096 bytes, array/list 262,144 entries |
| Palette | exact packed long count and every index checked; up to section cell count |
| Initial lighting grid | at most 256×256×384 cells; bounded frontier and relaxation work |
| Edit lighting grid | at most 2,097,152 cells; 1,048,576-entry frontier; 200M neighbor-work cap |
| Shared lighting ingress | eight pending jobs plus one in flight; one pending transaction/world |
| Collision query | 8,192 candidate cells, at most 32,768 static boxes |
| Ordinary collision storage | 64 stack boxes, bounded heap fallback |
| Existing world edit budgets | 65,536 overrides, 4,096 modified chunks, 1 MiB logical block+light charge/world, 64 MiB retained shared budget |

Template charges are accounting policies, not RSS or exact allocator memory.
Import/serialization happens before listening in a blocking worker. Edit lighting
uses one lazy shared worker; movement/combat never wait for Java or perform file
IO. Empty worlds own no simulation task/timer. Clones share template/overlay/light
Arcs; modifying one cannot mutate another. Reset/destroy reject occupied worlds.
Busy, stale, occupied, unsupported or over-budget edits do not partially commit.

## Streaming and lighting

Protocol-777 local/global palettes, singleton cases, section non-air/fluid counts,
biomes, typed heightmaps, byte-order light masks and arrays are serialized. Immutable
suffixes are shared; only coordinates are prepended. Edits replay before batch ACK
and update relevant loaded chunks; light patches are separately reconciled.
Heightmaps are recomputed for base chunks; edited cells use normal block updates
and light packets. Clients maintain their edited local heightmaps. No mutable
whole-map copy per player/instance, and no unbounded serialization cache.

The static light model supports zero-emission blocks and skylight attenuation.
It rebuilds the converted volume and an edit's local neighborhood, retaining fixed
outside boundary light. It does not implement every vanilla partial-face occlusion,
block emission, fluid/light interaction or neighbor rule. No full lighting parity
is claimed for decorative non-full shapes. Independent checks verify the actual
wire envelope and cells; those checks do not prove the whole lighting algorithm.

## Movement and combat

Player AABB 0.6 wide ×1.8 high (1.5 crouched), nearby boxes, walls/floors/ceilings,
negative/chunk boundaries and bounded 0.6 step. Geometry clips requested movement;
meaningful divergence triggers an acknowledged position correction. Horizontal map
edges are blocked. Falling below the supported void recovery threshold returns
through an acknowledged spawn teleport, bypassing ordinary travel-distance credit.
This is an arena policy, not vanilla void damage or a complete anti-cheat.

Accepted M3 base/sprint knockback functions, historical yaw/precision and ground
contact settling are preserved. Imported grounded state uses confirmed contact;
predicted collision on the next simulated step does not turn a falling hit into
a sweep. The old virtual-flat compatibility path remains. General entity collision,
swimming, ladders, moving blocks, fluids and full survival physics are deferred.

## Building and mining

`--world-profile build` explicitly enables client terrain edits on imported maps.
`editable` retains trusted edit controls; `read-only` rejects mutation. Imported
players use survival mode for build controls, adventure for read-only; flight stays
disabled. PvP itself remains `--pvp on|off`. The kit supplies both swords, an iron
pickaxe and supported full blocks. Inventory is server-owned; placements do not
consume the renewable arena kit. Placement checks reach, chunk interest, supported
item, air target, player occupancy, generation and resource budgets.

The user added narrow held mining to M4A after rejecting instant building. Hardness,
tool speed/correctness, default attack speed and airborne speed penalty are checked
against 26.3. Swords can mine. Bedrock cannot. Abort/tool/world changes cancel the
tracked attempt. A legitimate finish at sufficient progress commits asynchronously;
premature/stale/unsupported requests reconcile. Existing actions carry crack
updates; there is no mining thread/tick loop. Only the four relevant supported
mining-tag subsets are populated for imported clients; other bootstrap tags remain
empty. Effects/efficiency/enchantments/submerged mining and vanilla delayed-destroy
processing are not implemented. No drops, durability, crafting or harvesting.

Gameplay retains the 32/s burst-eight combat/action budget. Imported mining/place/
swing input uses a separate bounded 64/s burst-sixteen allowance so legitimate held
input does not consume combat tokens. Overall packet/byte limits remain intact.

## Typed Java controls

SDK 0.3.0 / schema 1.2. `templates().configured()` discovers the operator-loaded
immutable template. `Profile.BUILD`, `worlds().state(id)`, `readStates` and bounded
`editStates` expose supported state facts and transactional edits. Existing create,
clone, reset, destroy and committed-edit event remain. IPC cannot select arbitrary
filesystem imports. Java remains optional, asynchronous and outside collision.
Schema 1.0/1.1 legacy operations remain negotiated; legacy Air/Stone reads fail on
unrepresentable states instead of returning a misleading value.

Respawn/world-switch/void recovery checks the current edited terrain and chooses a
clear supported point within a bounded four-block horizontal radius if the configured
point is obstructed or unsupported. No safe point produces an explicit failure;
the accepted combat equations remain unchanged.
