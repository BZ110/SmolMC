# M4A protocol, storage and mechanics evidence

2026-10-30. Wire target exactly Java 26.3/protocol 777. Disk target data version
5023. Gameplay retains accepted M3's 1.19.4-based PvP policy. No Purpur.

## Observed reference facts

User-owned 26.3 reference paths, not redistributed bodies:

- `world/level/chunk/storage/{RegionFile,RegionFileVersion,SerializableChunkData}`:
  sector locations/lengths/compression and fully generated modern chunks.
- `nbt/ListTag`, `world/level/block/state/{BlockState,StateHolder}`: heterogeneous
  disk lists wrap non-compounds under an empty key; block palette entries may be
  compact identifiers or lowercase `id` / `properties` records. Historical uppercase
  `Name` / `Properties` records are also accepted in the bounded decoder.
- `world/level/chunk/{Strategy,PalettedContainer,LevelChunkSection}`: no-straddle
  packed words, minimum four-bit local block palettes, local biomes up to three
  bits, global palette widths, singleton layouts and section fluid counts.
- `core/Direction`, `world/entity/Entity`: Y then larger horizontal-axis clipping;
  static AABBs and bounded step foundation. The independent primitive check does
  not instantiate a complete server collision/anti-cheat world.
- `world/level/block/state/BlockBehaviour`, `world/entity/player/Player`,
  `world/item/{ToolMaterial,Items}`, `server/level/ServerPlayerGameMode`: hardness,
  correct-tool divisor, default speed, airborne penalty and held mining semantics.
- Protocol game registration and actual packet classes below.

Original adapters query the installed official client JAR and its registries. The
metadata catalog contains identifier/number/geometry facts only. Four supported
numeric mining-tag subsets are derived from the operator-owned reference and
checked with actual tool methods. No assets, source implementation bodies or
reference JARs are committed. Scripts never read account/launcher credential files.

## Independent checks

`check_terrain777_reference.py` imports the operator's actual `USE THIS` save into
a private corpus, checks 16 full-height chunks / 1,572,864 cells against the actual
26.3 disk palette decoder, and validates exact packet decode/consume/re-encode.
Supported original cells must match; the explicit conversion's unsupported cells
must be air. This verifies the conversion contract, not lossless arbitrary maps.
The corpus stays ignored under `.tools`; evidence saves hashes/results, not terrain.

`TerrainMechanicsReferenceCheck` invokes actual block destroy progress with real
items/tags/player grounding for 48 tool/hardness/airborne vectors and actual entity
shape clipping for 72 full-block vectors. The flat test-level adapter is original;
unsupported services throw. These numeric gates supplement real-client acceptance.
They do not establish every step shape, light engine, survival effect or renderer.

The separate actual 26.3 PvP codec oracle and actual Paper 1.19.4 yaw/velocity
quantization oracle remain regression gates. Their scope remains as in M3.

## Wire facts added or extended

| Direction / ID | Exact supported purpose |
|---|---|
| Clientbound `0x2e` | Full chunk+light: signed chunk coordinates; typed heightmap longs; 24 sections; correct local/global/singleton block and biome containers; no block entities; light masks/arrays |
| Clientbound `0x31` | LightUpdate: chunk x/z VarInt, four byte-array BitSet masks, sky/block arrays; sorted section bits |
| Clientbound `0x08` | BlockUpdate: packed signed block position and block-state VarInt |
| Clientbound `0x04` | BlockChangedAck: sequence VarInt |
| Clientbound `0x05` | BlockDestruction: breaker entity VarInt, packed position, stage byte (0–9; 255 removes) |
| Clientbound `0x27` | GameEvent type 3, game mode f32; read-only imported mode 2, building mode 0 |
| Clientbound `0x12` / `0x68` | Existing authoritative inventory/equipment supports default iron pickaxe item 1047 and bounded server-created block items |
| Clientbound `0x86` | Existing attack-speed attribute: base 4; swords -2.4f32→f64, iron pickaxe -2.8f32→f64; block/empty hand no modifier |
| Configuration `0x0e` | Existing tag packet, with four supported mining-tag memberships populated only for imported clients |
| Serverbound `0x29` | Start 0, change direction 1, abort 2, finish 3, drops 4/5, release 6, F swap 7; packed position, direction byte, sequence VarInt |
| Serverbound `0x42` | UseItemOn: hand, packed clicked position, face, bounded cursor floats, inside/world-border booleans, sequence |

Packet IDs include the bundle delimiter slot at zero. Actual codecs independently
check changed formats and **semantic** mining action enums; this is essential because
self-contained fixtures can mirror an incorrect enum/layout.

## Deliberate policies versus reference facts

Imported source is immutable; sparse world overlays and scoped watchers own edits.
The lightweight zero-emission skylight model is rebuilt for converted terrain and
local edits. Actual light packets are checked; full vanilla light propagation and
partial-face occlusion equivalence are not claimed. Client block updates maintain
edited local heightmaps; shared base chunks carry recalculated heightmaps.

Held mining uses `tool_speed / hardness / (correct_tool ? 30 : 100)`, divided by
five airborne. Progress follows elapsed common 50ms ticks plus the starting tick.
A finish at ≥0.7 is accepted as in the inspected vanilla handler. Premature finish
requests reconcile rather than running the complete delayed-destroy tick path.
Mining occurs without drops/durability or full effects/harvesting. World/tool/abort
transitions invalidate the tracked attempt. Existing actions carry crack feedback.

Collision uses nearby checked boxes, step ≤0.6, explicit map boundaries and arena
void teleport recovery. It is not a complete vanilla simulator or anti-cheat.
Imported hit classification uses confirmed support, not a future simulated contact.
M3 knockback functions, yaw/velocity precision and confirmed-ground settling remain.

## Corrected failures

The first 112–191 Y crop omitted lower terrain; the user reported missing ground
and a kick while falling. The final test imports -64–319. Void recovery now uses
an acknowledged teleport rather than spending normal movement-distance credit.
The user then clarified that held mining and sword timing were required. These
narrow mechanics supersede the initial instant-build interpretation; no full
survival expansion follows.

An initial mining implementation treated abort/finish as older enum ordinals;
plants broke on start but harder blocks never completed. Actual 26.3 inserts
CHANGE_DESTROY_DIRECTION before ABORT/STOP. This was corrected to 0/1/2/3 and
independently checked against actual enum names, with encrypted hold/finish tests.
The user accepted the corrected build: “Yes! Everything works. Continue on.”

The first full-height load fixture failed before measurement because its old
server-packet receiver was capped at 64 KiB/128x. The bounded client-side receive
policy now permits legal server chunks (256 KiB/1024x). Production server ingress
still defaults to 64 KiB/128x and does not select that client policy. Failed setup
logs are retained and are not reported as performance results.

Respawn/world-switch/void recovery checks the current edited terrain and chooses a
clear supported point within a bounded four-block horizontal radius if the configured
point is obstructed or unsupported. No safe point produces an explicit failure;
the accepted combat equations remain unchanged.
