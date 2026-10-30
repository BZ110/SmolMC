# 01 — User-provided reference archive: factual audit and usage policy

## Archive examined
`unz.zip` (supplied in this conversation). `unz/version.json` reports:
- `id`: 26.3; `protocol_version`: **777**; `world_version`: **5023**.
- Resource pack: **97.1**, data pack: **121.0**.
- Build time: **2026-09-15T11:20:48Z**, Java component `java-runtime-epsilon`, Java major **25**, stable: true.

## Inventory facts from inspecting ZIP directory entries
- **14,939** non-AppleDouble files, including **5,037 `.java`**, **8,372 `.json`**, and **1,511 `.nbt`** files (remaining are metadata and miscellaneous).
- **432** Java files in `net/minecraft/server/` subtree; **416** in `net/minecraft/network/`; **1,418** in `net/minecraft/world/level/`; **727** in `net/minecraft/world/entity/`.
- **9,884** files under `data/minecraft/`, including bundled registry/world data.
- File counts are inventory measurements; they do not measure implementation complexity or reveal exactly what the protocol requires.

## High-priority reference paths (path names, not redistributed source)
| Concept | Relative path within `unz/` | Why inspect when authorized |
|---|---|---|
| Protocol state enum | `net/minecraft/network/ConnectionProtocol.java` | legal transitions |
| Framing output | `net/minecraft/network/PacketEncoder.java` | packet serialization |
| Framing input | `net/minecraft/network/PacketDecoder.java` | inbound decoding |
| Status listener | `net/minecraft/server/network/ServerStatusPacketListenerImpl.java` | ping |
| Login listener | `net/minecraft/server/network/ServerLoginPacketListenerImpl.java` | authentication / login |
| Configuration listener | `net/minecraft/server/network/ServerConfigurationPacketListenerImpl.java` | pre-play negotiation |
| Play listener | `net/minecraft/server/network/ServerGamePacketListenerImpl.java` | movement and chat |
| Players | `net/minecraft/server/players/PlayerList.java` | global player bookkeeping |
| Player chunk sender | `net/minecraft/server/network/PlayerChunkSender.java` | chunk scheduling |
| Chunk manager | `net/minecraft/server/level/ChunkMap.java` | region/chunk availability |
| Chunk source | `net/minecraft/server/level/ServerChunkCache.java` | chunk storage/caching |
| World | `net/minecraft/server/level/ServerLevel.java` | simulation and lifecycle |
| Player | `net/minecraft/server/level/ServerPlayer.java` | session-to-entity mapping |
| Chunk | `net/minecraft/world/level/chunk/LevelChunk.java` | chunk sections |
| Palette | `net/minecraft/world/level/chunk/PalettedContainer.java` | block-state storage |
| Registry loader | `net/minecraft/resources/RegistryDataLoader.java` | runtime data registry |
| Registry sync | `net/minecraft/core/RegistrySynchronization.java` | client sync |
| Chat in | `net/minecraft/network/protocol/game/ServerboundChatPacket.java` | chat parsing |
| System chat out | `net/minecraft/network/protocol/game/ClientboundSystemChatPacket.java` | safe outbound status |
| Storage manager | `net/minecraft/world/level/storage/LevelStorageSource.java` | save lifecycle |

## Research method
The archive provides an **index of real implementation topics**, not a license or blueprint to translate files line-by-line. Use clean-room design where possible; independently check packets against protocol-777 community documentation; keep path/name inventories but do **not** copy code or bunded Mojang assets into release outputs.

## Why no generated source extraction in this pack?
This ZIP is intended to be safe to share with an AI coding agent. Bundling Mojang sources, proprietary registry/asset JSON, or decompiled file contents creates redistribution and licensing concerns. The original user-provided file remains separate.

## Provenance caveat
The `.java` files are present in the archive, but their precise upstream provenance (official source, decompilation, mappings, preprocessing) cannot be established solely from file names. All reference behavior must be independently confirmed by tests.
