# M1B protocol evidence — Java Edition 26.3 / protocol 777

Date: 2026-10-29. Scope: legitimate online Login, Configuration and minimum Play
entry only. The supplied archive remains private reference material.

## Source hierarchy and pins

`evidence/m1b-protocol-sources.json` pins 54 inspected reference paths by SHA-256.
`evidence/m1b-packet-ids.json` inventories state/direction registration indices,
including Play's bundle delimiter at zero. These contain names, numbers and
hashes only. `scripts/inventory_protocol_ids.py` reproduces the ID inventory.

Indices were checked against `ProtocolInfoBuilder`/`IdDispatchCodec`; field codecs
were inspected separately. A signed-in stock 26.3 client then completed the
exchange, including Configuration ACK and inbound Play packets on the final
binary. Installed-client provenance from M1A is in `evidence/protocol-sources.json`.

M1A's public maintained version table corroborates 26.3/777. Minecraft Wiki
packet/encryption pages remained inaccessible through the web tool. No independent
public field-level specification is claimed: local reference, independent fixtures
and real-client interoperability are distinguished. Older-version layouts were
not substituted. No implementation bodies or registry assets are redistributed.

## Implemented packet ledger

IDs are hexadecimal. Paths abbreviate `unz/net/minecraft/`; packet classes are
under `network/protocol/` unless otherwise stated.

| State/direction | ID | Verified purpose/layout | Reference |
|---|---:|---|---|
| Handshake SB | 00 | Protocol VarInt, String(255) hostname, unsigned port, intent | `handshake/ClientIntentionPacket`, `HandshakeProtocols` |
| Status SB/CB | 00, 01 | Empty request / JSON string; signed long ping/pong | M1A ledger, `status/StatusProtocols`, `ping/*` |
| Login SB | 00 | String(16) name, untrusted claimed UUID | `login/ServerboundHelloPacket` |
| Login CB | 01 | Empty server ID, DER key array, challenge array, authenticate=true | `login/ClientboundHelloPacket` |
| Login SB | 01 | Encrypted shared-secret and challenge arrays | `login/ServerboundKeyPacket` |
| Login CB | 03 | Threshold VarInt, sent before compression activation | `login/ClientboundLoginCompressionPacket`, server login listener |
| Login CB | 02 | Verified UUID/name/properties, then separate server-session UUID | `login/ClientboundLoginFinishedPacket`, `codec/ByteBufCodecs` |
| Login SB | 03 | Empty Login ACK; Configuration transition | `login/ServerboundLoginAcknowledgedPacket` |
| Login CB | 00 | JSON component string rejection in a known transport state | `login/ClientboundLoginDisconnectPacket` |
| Configuration CB | 0d | Feature identifier set, vanilla only | `configuration/ClientboundUpdateEnabledFeaturesPacket`, `world/flag/FeatureFlags` |
| Configuration CB/SB | 0f/07 | Known-pack tuples; response bounded to 64 | `configuration/*SelectKnownPacks`, `server/packs/repository/KnownPack` |
| Configuration CB | 07 | Registry key, ordered entries, entry name and optional NBT flag | `configuration/ClientboundRegistryDataPacket`, `core/RegistrySynchronization` |
| Configuration CB | 0e | Registry map of named tags / numeric membership arrays | `common/ClientboundUpdateTagsPacket`, `tags/TagNetworkSerialization` |
| Configuration SB | 00 | Language, view distance, chat preferences, model flags, main hand, filtering/listing, particles | `common/ServerboundClientInformationPacket`, `server/level/ClientInformation` |
| Configuration SB | 02 | Identifier and bounded custom payload; brand parsed | `common/ServerboundCustomPayloadPacket`, `common/custom/BrandPayload` |
| Configuration CB/SB | 03/03 | Empty Finish Configuration and ACK | `configuration/*FinishConfigurationPacket` |
| Configuration CB | 02 | Anonymous NBT component disconnect | `common/ClientboundDisconnectPacket`, `network/chat/ComponentSerialization` |
| Play CB | 32 | Minimal Login / CommonPlayerSpawnInfo, registry-indexed dimension type | `game/ClientboundLoginPacket`, `game/CommonPlayerSpawnInfo` |
| Play SB | 0a | Chat-session UUID, expiry long, encoded key array (512 max), signature array (4096 max); discarded | `game/ServerboundChatSessionUpdatePacket`, `network/chat/RemoteChatSession`, `world/entity/player/ProfilePublicKey` |
| Play SB | 0d, 2c | Empty tick-end / player-loaded notification | `game/ServerboundClientTickEndPacket`, `game/ServerboundPlayerLoadedPacket` |
| Play SB | 0e, 16 | Client information / custom payload | `game/GameProtocols`, common codecs |
| Play CB | 20 | Anonymous NBT component probe-completion disconnect | `game/GameProtocols`, `common/ClientboundDisconnectPacket` |

Previous game type in CommonPlayerSpawnInfo is Optional VarInt (zero for absent),
not legacy byte -1; current game type is also VarInt. Dimension type uses its
synchronized registry's holder index. Bootstrap uses the transmitted overworld
type and `smolmc:bootstrap` dimension key, without a simulated world or chunk/entity
stream. All unrelated packet IDs remain unsupported, not silently guessed.

## Encryption, identity and compression

Observed reference `util/Crypt`: RSA-1024, AES-128-CFB8 with secret as key/IV, SHA-1
of empty server ID + shared secret + DER key, Java BigInteger signed hexadecimal
hash. The login listener verifies challenge, enables encryption, verifies account,
negotiates compression and awaits ACK. OpenSSL supplies primitives; signed-hex
encoding has known vectors. Only certificate-validated success from the fixed
Mojang session service creates an authenticated profile. Names, IDs, properties,
signatures and total body size are bounded. The server never reads access tokens.

Encryption is continuous per direction, including prefixes. Buffered bytes after
Key Response remain raw until encryption activation. Compression wraps payload
inside framing before encryption. Set Compression is flushed in the old mode;
later frames use the new mode. Cipher/framing/write state survives cancellation.

M1B's supported profile is stricter than the reference's broad wire ceiling:
64 KiB control/decompressed limit, ratio 128, shared 8 MiB/s wire+decoded work.
Uncompressed data at/above threshold, false decoded sizes, trailing zlib bytes,
oversized prefixes and invalid fields are rejected. These are deliberate profile
limits, not complete Play coverage. No client-supplied NBT is accepted by this
slice; common disconnects encode fixed bounded text as anonymous NBT strings.

## Registry/tag policy and client corrections

The core tuple is exactly `minecraft`, `core`, `26.3`, observed in
`BuiltInPackSource`/`KnownPack`. Data omission requires an exact acknowledgement.
Declined/mismatched packs cause a tested explicit rejection; NBT fallback is absent.

The registry script inventories filenames only: 32 registry keys, 432 ordered
entry names, 773 tag names in network-safe registries. No JSON/NBT contents or
vanilla membership data are copied. Entry numeric IDs follow transmitted order.

Tags are explicitly empty server-owned bootstrap definitions. An empty Update
Tags **map** was insufficient: the first client reported missing named bindings
while loading its core data. Named empty tags resolved those registry failures.
This is a valid non-gameplay bootstrap, not vanilla tag-membership parity.

Attempt two reached Play but closed on an unsupported packet; its exact rejected
ID was not captured. Bounded automatic chat-session handling resolved the next
attempt, whose first decoded Play packet was 0x0a. That key/signature metadata is
discarded, never treated as verified chat identity or used to forge signatures.

The final check verified account identity, Login ACK, Configuration ACK after
registry loading, Play Login and **403** decoded Play messages. The user confirmed
the planned completion message on the final binary. Loading Terrain remains
visible because M1B sends no chunks. See `evidence/m1b-vanilla-server.log`,
`evidence/m1b-vanilla-client-check.json` and the retained failed-attempt records.
