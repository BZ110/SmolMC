# 03 — Protocol 777 (Java Edition 26.3): first useful connection

## Absolute rule
Never invent a field, ID, ordering dependency, or packet payload from memory. Packet IDs are **state-, version-, and direction-specific**. This document is a workflow/checklist and **not** a byte-accurate packet implementation spec. Authoritative references must be consulted during future implementation. See `docs/18_SOURCES.md`.

## State machine
**Handshake** branches into **Status** or **Login**. The gameplay path is **Handshake → Login → Configuration → Play**. This is not optional for modern Java Edition. Status ping is a separate branch. Protocol transitions are explicit and may require acknowledgements. Do not write a legacy pre-1.20.2 login implementation.

## Milestone wire journey and hazards

| Phase | Required purpose | Selected protocol topics | Main failure mode |
|---|---|---|---|
| Status | Server list status and ping | handshake intention, status request/response, pong | wrong framing/status JSON |
| Login | identity and session start | login start, online encryption challenge, compression, success/ack | invalid session / wrong state |
| Configuration | negotiate client data | client info, known packs, registry data, tags, feature flags, config finish/ack | registry mismatch / client disconnect |
| Play entry | establish player in dimension | play login, spawn position, abilities, view center, chunk batches | void / invalid dimension |
| World stream | render world | chunk+light, block states, biomes, light masks, batch acknowledgements | void, black terrain, disconnect |
| Player loop | interactive session | keepalive, position sync, teleport confirmation, movement, player list, entity spawn/sync | rubber-banding or invisible peer |
| Chat | allow messages | inbound chat and outbound system chat for MVP | signature/trust misrepresentation |
| Leave | reclaim resources | disconnect, subscription cleanup, overlays, player state | leaking players/chunks |

## Important implementation notes for 26.3
- Minecraft Wiki's protocol 777 table documents configuration `registry_data`, `finish_configuration`, `update_tags`, known packs negotiation, `update_enabled_features`, play `level_chunk_with_light`, `chunk_batch_start`, `chunk_batch_finished`, `player_abilities`, `login`, and the serverbound chat/position/ack variants. **Field-level layouts must be version-pinned during coding.**
- Modern registries refer to **numeric IDs by ordering**. The client will disconnect on unknown references; never assume dimension data can be represented by a single string.
- **The client protocol's 'Feature Flags' packet has nothing to do with this project's engine capability toggles.** The client packet informs vanilla registry/experimental feature support; our profile system controls optional server-side subsystems.
- Some clientbound packets are required even though gameplay features are off (e.g., registry negotiation, world bootstrap, keepalive, chunk streaming). 'Disabled gameplay system' does not equal 'delete protocol packet'.
- Compression and encryption are per connection; share immutable **pre-encryption** packet payloads, not encrypted stream bytes.
- Handshake hostname is client-controlled data; do not trust it for identity or routing without validation.
- Online mode must verify Mojang account identity; no bare unverified UUID claims in production. The reference flow includes encrypted login and session verification.
- Chat is a *distinct protocol/security problem*. For M1, receive permitted chat packets and broadcast **clearly labeled server/system messages**. Do not forge player chat signatures or present unsigned messages as securely signed.
- Movement and player visibility are not just `teleport`: player info entries, entity spawn packets, rotations, on-ground flags, position reconciliation, and view distance must be coordinated.
- Creative flying needs correct game mode + abilities and handling of movement packets; it does not mean trusting unbounded coordinates.

## Proposed protocol test fixture corpus (future work)
At least: ping/status; login transition; compression on/off; encryption/online mode; successful config registry load; malformed registry ID; player spawn; correct flat chunk payload; view-distance change; chunk unload; move across chunk border; teleport/confirm; join second player; message relay; disconnect cleanup. Save **sanitized** packet descriptions and validation results; protect secrets/token materials.

## Choosing protocol scope
**Initial target:** vanilla Java Edition 26.3 client only. Support via Velocity optional later. Multi-version backwards compatibility, ViaVersion interoperability, Bedrock, and modded clients are deliberately deferred.

## Packet source-of-truth process
1. Pin version and protocol 777.
2. Read Minecraft Wiki packet table (version/date) and relevant registry documentation.
3. Inspect class path in reference archive if necessary, without copying source.
4. Validate against an actual unmodified 26.3 client and packet traces in a controlled environment.
5. Freeze packet fixtures; record a discrepancy as an issue, never guess a field.

## Sources
- https://minecraft.wiki/w/Java_Edition_protocol/Packets
- https://minecraft.wiki/w/Java_Edition_protocol/Encryption
- https://minecraftdocs.dev/systems/networking/protocol-phases
- https://minecraft.wiki/w/Protocol_version
