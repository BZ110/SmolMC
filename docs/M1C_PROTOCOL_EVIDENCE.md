# M1C protocol-777 evidence ledger

Original capture filenames refer to the [local validation archive](../evidence/README.md).

Date: 2026-10-29. Target: **Java Edition 26.3, protocol 777 only**.
This extends [M1B's ledger](M1B_PROTOCOL_EVIDENCE.md); login, encryption,
compression, legitimate session verification and Configuration are preserved.

## Provenance and confidence

**Observed reference behavior:** the user-provided local `unz/` archive's
`version.json`, game/common protocol registration, packet readers/writers and
nested codecs were inspected. Source inventory (`evidence/m1c-protocol-sources.json`)
pins 57 paths with SHA-256 hashes and derived facts. The packet ID basis is
`evidence/m1b-packet-ids.json` (registration order, including protocol bundling
rules), not an older wiki version. All paths below are relative to `unz/`.
No proprietary source body or registry/dimension asset is bundled.

**Independent observation:** the stock signed-in 26.3 client rendered terrain,
flew across multiple chunks and remained connected; see
client check (`evidence/m1c-vanilla-client-check.json`). This proves the exercised
subset, not all legal inputs or full vanilla conformance.

**Public corroboration limit:** access to the community packet wiki was denied
by robots; no field-level public wiki cross-check is claimed. Maintainer flate2
documentation was consulted for zlib behavior. The exact-version local reference
and real client are the wire evidence. Reader tests alone are not authority.

## Clientbound Play additions/changes

Numbers are hex IDs; all fields are in wire order. Integers/IEEE floats/longs
are big-endian unless a field explicitly says otherwise. Strings and arrays
retain their bounded protocol prefixes.

| ID / packet | Verified body | Reference |
|---|---|---|
| Existing Login | Existing M1B body; game mode byte changes from 2 to 1 (creative), view radius remains 2 | `network/protocol/game/ClientboundLoginPacket.java`, `CommonPlayerSpawnInfo.java` |
| `2e` chunk with light | x Int, z Int, chunk data, light data (below) | `ClientboundLevelChunkWithLightPacket.java` |
| `0c` batch start | Empty | `ClientboundChunkBatchStartPacket.java` |
| `0b` batch finished | VarInt number of chunks | `ClientboundChunkBatchFinishedPacket.java` |
| `60` cache center | VarInt x, VarInt z | `ClientboundSetChunkCacheCenterPacket.java` |
| `26` forget chunk | Packed ChunkPos Long: x low 32 bits, z high 32 bits; big-endian bytes therefore z then x | `ClientboundForgetLevelChunkPacket.java`, `world/level/ChunkPos.java` |
| `41` abilities | Byte flags, Float flight speed, Float walk speed | `ClientboundPlayerAbilitiesPacket.java` |
| `49` player position | VarInt teleport ID, three Double position, three Double delta, Float yaw, Float pitch, Int relative flags | `ClientboundPlayerPositionPacket.java`, `world/entity/PositionMoveRotation.java`, `Relative.java` |
| `63` default spawn | GlobalPos (dimension resource-key String, packed BlockPos Long), Float yaw, Float pitch | `ClientboundSetDefaultSpawnPositionPacket.java`, `world/level/storage/LevelData.java`, `core/GlobalPos.java`, `BlockPos.java` |
| `27` game event | Byte event 13 (start level chunks), Float value 0 | `ClientboundGameEventPacket.java` |
| `2d` keepalive | Long challenge | `network/protocol/common/ClientboundKeepAlivePacket.java` |
| `73` time | Long game time, VarInt map size, each clock registry VarInt ID + VarLong total ticks + Float partial tick + Float rate | `ClientboundSetTimePacket.java`, `world/clock/ClockNetworkState.java`, `WorldClock.java` |
| `04` block changed ACK | VarInt sequence | `ClientboundBlockChangedAckPacket.java` |
| `08` block update | Packed BlockPos Long, VarInt block state | `ClientboundBlockUpdatePacket.java` |

Packet file names without another directory above are under
`net/minecraft/network/protocol/game/`. Abilities flags are invulnerable=1,
flying=2, may-fly=4, instant-build=8; M1C sends 0x0d initially with speeds
0.05/0.1. Teleport relative flags and velocity are zero. Spawn's dimension is
the existing M1B `smolmc:bootstrap` key, not a separate live world instance.
The time map uses the shared registry's `minecraft:overworld` clock ID,
total ticks 6000, partial/rate zero (frozen noon). Positive VarLong 6000 has
the same bytes as VarInt 6000; the specialized encoder documents this fact.

### Exact chunk content

`ClientboundLevelChunkPacketData.java` defines a VarInt-count map of heightmap
enum VarInts to VarInt-count big-endian Long arrays, a VarInt-length section
byte array, and a VarInt-count block-entity list. **Heightmaps here are not NBT.**
`Heightmap.java` establishes types 1,4,5 used for WORLD_SURFACE,
MOTION_BLOCKING and MOTION_BLOCKING_NO_LEAVES. `SimpleBitStorage.java` pads
entries so they do not straddle Longs: 9 bits, seven heights per Long, 37 Longs
per 256-entry map. All heights are first air 64 minus minimum y -64 = 128;
unused bits in the final Long are zero.

`LevelChunkSection.java`, `PalettedContainer.java`, `SingleValuePalette.java`
and `Palette.java` establish each section's nonempty Short, **fluid-count
Short**, block palette and biome palette. Singleton format is bits Byte 0,
registry ID VarInt, **no storage-array length or data**. There are 24 sections
for min y -64/height 384: eight stone sections (4096 nonempty, zero fluid),
sixteen air sections (zero counts); biome plains in all sections. Air/stone
states 0/1 are derived from `Blocks.java` registration and `Block.java` state
registration with singleton air/stone definitions, not recalled version IDs.
Plains ID 41 is looked up in the shared ordered biome identifier inventory.
The overworld dimension asset was inspected only for min y/height facts.

`ClientboundLightUpdatePacketData.java`, `LevelLightEngine.java` and
`DataLayer.java` establish four masks (sky-present, block-present, sky-empty,
block-empty), then two VarInt-count lists of byte arrays (sky, block).
`ByteBufCodecs.BIT_SET` / `FriendlyByteBuf` define a **VarInt byte length and
trimmed little-endian bytes**, matching Java BitSet byte representation. This
is not a VarInt-count Long array. For 26 light sections (two sentinels), masks
are 0x3fffe00, 0, 0x1ff, 0x3ffffff. The sky list contains 17 arrays of 2048
bytes 0xff; block list is empty. Opaque bottom sections and bottom sentinel
have empty sky; all sections have empty block light. No lighting engine runs.

With live biome ID 41 the coordinate-independent body is 35,956 bytes; packet
ID and two coordinates make the complete uncompressed payload 35,965 bytes.
Transport compression/encryption framing is separate and unchanged.

## Serverbound Play subset

Every accepted fixed body must consume the packet exactly; bounded variable
metadata uses existing M1B parsers. Nonfinite coordinates/angles/rates reject.

| ID / packet | Verified body / handling | Reference |
|---|---|---|
| `00` accept teleport | VarInt ID, **three Double position, two Float rotation**; match issued destination | `ServerboundAcceptTeleportationPacket.java` |
| `0b` batch received | Float desired chunks per tick; positive and finite | `ServerboundChunkBatchReceivedPacket.java` |
| `1c` keepalive | Long challenge, exact match | `network/protocol/common/ServerboundKeepAlivePacket.java` |
| `1e` position | Three Double position, Byte flags | `ServerboundMovePlayerPacket.java` |
| `1f` position+rotation | Three Double position, two Float rotation, Byte flags | Same |
| `20` rotation | Two Float rotation, Byte flags | Same |
| `21` status | Byte flags; bit 0 on-ground, bit 1 horizontal collision | Same |
| `28` abilities | Byte bit 1 flying | `ServerboundPlayerAbilitiesPacket.java` |
| `2a` command | VarInt own entity ID, VarInt action 0..6, VarInt nonnegative data; no simulation | `ServerboundPlayerCommandPacket.java` |
| `29` player action | VarInt action 0..8, packed BlockPos Long, Byte direction 0..5, VarInt sequence; read-only ACK/correction | `ServerboundPlayerActionPacket.java` |
| `2b` input | Byte seven input bits; accepted, no physics | `ServerboundPlayerInputPacket.java`, `world/entity/player/Input.java` |
| `36` carried item | Short slot 0..8; discard | `ServerboundSetCarriedItemPacket.java` |
| `13` close container | VarInt container ID; discard | `ServerboundContainerClosePacket.java`, `FriendlyByteBuf.readContainerId` |
| `0d`, `2c`, `2e` | Empty tick-end, player-loaded, punch respectively; discard | Registration inventory; corresponding packet readers |
| `0a`, `0e`, `16` | Existing bounded chat-session metadata, client information, custom payload; discard | M1B ledger/parsers |

`ServerboundClientTickEndPacket.java` and `ServerboundPlayerLoadedPacket.java`
empty layouts were inspected during M1B and appear in its pinned source ledger;
`ServerboundPunchPacket.java` is additionally pinned in M1C. Metadata handling
does not add chat, trust a client UUID, or bypass legitimate authentication.
Unknown gameplay packets reject readably. Inventory clicks/creative item
editing, use-item/placement and chat are outside this milestone.

## Design policies, not wire requirements

Radius two/25 slots, four chunks per batch/one batch in flight, 50–500ms batch
pacing, five-second write timeout, ten-second teleport/batch ACK, twelve-second
keepalive interval/twenty-second response, 256 Play packets/second, 32-block
movement burst/64 blocks-second refill and coordinate bounds are local M1C
resource policies. They are not claimed as vanilla defaults. No body layout is
changed to accommodate a policy. No full world simulation is required to send
immutable terrain and validate session controls.

## Failed evidence and corrections

Attempt 1 used the wrong nested bitset representation; client decoding failed
with `ByteArray with size 16255 is bigger than allowed 2048`. Its initial test
decoder repeated the encoder's assumption. The correction traces the nested
codec and adds a literal byte fixture rather than relying on round-trip success.
Attempt 2's full 34-byte teleport packet exposed an ID-only parser; it now reads
all echoed fields, checks the destination and tests truncated prefixes. The
third attempt passed rendering, flight and sustained connection. Failed logs
remain under `evidence/m1c-client-attempt{1,2}*`; they are not silently discarded.

Remaining unverified areas include every legal control combination, external
auth outage behavior under sustained successful-account load, long-distance
flight/latency extremes, large/heterogeneous terrain, additional versions and
later gameplay packets. Tests and one client observation do not close them.
