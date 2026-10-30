# M1D protocol-777 evidence ledger

Date: 2026-10-29. Exact target: Java Edition **26.3, protocol 777**.
This extends [M1B](M1B_PROTOCOL_EVIDENCE.md) and [M1C](M1C_PROTOCOL_EVIDENCE.md).
Authentication, encryption/compression, Configuration and terrain layouts remain.

## Provenance and verification

Observed reference behavior comes from the user-provided local archive. The
[33-path SHA-256 inventory](../evidence/m1d-protocol-sources.json) pins game protocol
registration, packet fields, registry ordering and nested codecs. Packet IDs
follow the M1B registration-order inventory, including bundling rules. Paths
below are relative to `unz/net/minecraft/`; packet basenames without a directory
are under `network/protocol/game/`. No implementation bodies, game assets,
account credentials or raw chat contents are redistributed.

The stock two-account client checks independently exercised visibility, walking/
flight, rotation/crouching, bidirectional chat, removal from the world/Tab list,
reconnect and keepalive. See [final client record](../evidence/m1d-vanilla-client-complete.json).
This establishes the exercised subset, not all legal packet combinations.
An emoji was optional in the final request and was not separately confirmed;
supplementary-character encoding is established by a literal automated fixture.

Community packet-wiki access was restricted again; no public field-level
corroboration is claimed. Maintainer [Tokio bounded-channel documentation](https://docs.rs/tokio/1.48.0/tokio/sync/mpsc/index.html)
was consulted for the stable channel API, not Minecraft field layouts. Locked
Tokio 1.54.1 local source was also inspected and dependency hashes recorded
for permit/watch behavior.
Local exact-version source plus the real client are the packet evidence.

## Clientbound additions

IDs are hex and bodies are in wire order. Fixed numeric fields are big-endian;
packed angles are single unsigned wire bytes. VarInt count prefixes are explicit.

| ID | Body used | Evidence |
|---|---|---|
| `47` Player Info Update | Fixed one-byte action bitset `8f`; VarInt count 1; UUID; action fields below | `ClientboundPlayerInfoUpdatePacket.java`, `FriendlyByteBuf.writeEnumSet`, `ByteBufCodecs.GAME_PROFILE_PROPERTIES` |
| `46` Player Info Remove | VarInt count 1, UUID | `ClientboundPlayerInfoRemovePacket.java` |
| `01` Add Entity | VarInt runtime ID, UUID, entity-type registry VarInt 159, three Double position, LPVec3 velocity, Byte pitch/yaw/head-yaw, VarInt data 0 | `ClientboundAddEntityPacket.java`, `world/phys/Vec3.java`, `network/LPVec3.java`, `world/entity/EntityTypes.java` |
| `23` Entity Position Sync | VarInt ID; PositionPath discriminator VarInt 0 (LINEAR), three Double position; Float yaw, Float pitch, Bool on-ground | `ClientboundEntityPositionSyncPacket.java`, `world/entity/PositionPath.java` |
| `55` Head Rotation | VarInt ID, Byte packed yaw | `ClientboundRotateHeadPacket.java` |
| `65` Entity Metadata | VarInt ID, index/type/value entries, terminator Byte 255 | `ClientboundSetEntityDataPacket.java`, `network/syncher/SynchedEntityData.java` |
| `4e` Remove Entities | VarInt count 1, VarInt runtime ID | `ClientboundRemoveEntitiesPacket.java` |
| `7c` System Chat | Anonymous NBT component (plain string), Bool overlay false | `ClientboundSystemChatPacket.java`, `network/chat/ComponentSerialization.java`, `nbt/StringTag.java` |

Player Info `8f` selects ADD_PLAYER (bit 0), INITIALIZE_CHAT (1), UPDATE_GAME_MODE
(2), UPDATE_LISTED (3), UPDATE_HAT (7). Entry order follows enum declaration:
UUID; username String max 16 UTF-16 units; property list (VarInt count, String
name/value, Bool signature present and optional String signature); Bool false
for absent remote chat-session data; VarInt game mode 1; Bool listed true;
Bool hat true. Properties come from the verified session-service profile and
remain bounded by the existing auth parser. No player-chat key is advertised.
Info precedes entity spawn; entity removal precedes info removal. Only entities
visible in the viewer's actually sent chunks have remote info entries in this
minimal profile; the Tab list is therefore visibility scoped.

Entity type 159 is derived from `EntityTypes.java`'s 161 registration calls:
PLAYER is zero-based entry 159, before FISHING_BOBBER. `EntityTypeIds.java`
establishes its name. This is a built-in entity registry ordering, distinct from
the client-known Configuration registries. Zero velocity is **one zero LPVec3
byte**, not three legacy velocity Shorts. Packed angles are floor(degrees*256/360)
modulo 256. Full absolute PositionPath.LINEAR updates avoid relative-delta base
drift; they are a deliberate bandwidth/CPU trade-off, not an older teleport body.

Metadata uses index 0 (shared flags), serializer 0 (Byte), bits 1 crouch and 3
sprint; index 6 (pose), serializer 20 (VarInt enum), STANDING 0 or CROUCHING 5;
then 255. These IDs come from `Entity.java` accessor declaration order,
`util/ClassTreeIdRegistry.java`, serializer registration order, `core/IdMapper.java`
and `world/entity/Pose.java`. No swimming, elytra, sleeping or survival animation
state is inferred. Head yaw is sent separately when it changes.

System components use the trusted component codec's plain-string alternative,
which is anonymous TAG_String (type 8, unsigned big-endian Short encoded byte
length, modified UTF-8 bytes). `StringTag.write` uses Java `writeUTF`.
Supplementary characters require two encoded UTF-16 surrogates, not a four-byte
UTF-8 code point. The literal fixture for `A`, NUL and U+1F600 checks bytes
`41 c0 80 ed a0 bd ed b8 80`. Network packet Strings remain normal UTF-8;
these two string encodings must not be confused. NUL is fixture-only; user chat
rejects control characters. No JSON/rich component supplied by a client is parsed.

## Serverbound additions and interpretation

| ID | Exact body / handling | Evidence |
|---|---|---|
| `09` Chat | String max 256 UTF-16 units; Long timestamp milliseconds; Long salt; Bool signature present plus optional fixed 256 bytes; VarInt last-seen offset; fixed 3 bytes for 20 acknowledged bits; Byte checksum | `ServerboundChatPacket.java`, `network/chat/MessageSignature.java`, `LastSeenMessages.java`, `ByteBufCodecs.INSTANT`, `FriendlyByteBuf.readFixedBitSet` |
| `06` Chat ACK | VarInt offset | `ServerboundChatAckPacket.java` |
| `2b` Player Input | Byte bits: forward 0, backward 1, left 2, right 3, jump 4, shift 5, sprint 6 | `ServerboundPlayerInputPacket.java`, `world/entity/player/Input.java` |
| `2a` Player Command | Existing entity/action/data VarInts; own runtime ID only; actions 1/2 start/stop sprint | `ServerboundPlayerCommandPacket.java` |
| `01` Attack | VarInt nonnegative target runtime ID; parsed exactly and discarded, no combat | `ServerboundAttackPacket.java` |

Accepted fixed packet bodies must consume all bytes. String, signature and
last-seen fields are bounded before allocation/relay. Invalid/truncated bodies,
nonzero last-seen offset/acknowledgements or negative Chat ACK offset reject.
Only offset 0 is accepted because this server never sends signed Player Chat
and therefore has no signed-message history to acknowledge. Checksum/timestamp/
salt/signature bytes are consumed but do **not** establish message authenticity.

Chat sender identity is the legitimate authenticated connection's profile.
Messages are plain text prefixed **`[System relay] <verified-name> ...`** and sent
only using System Chat. No Player Chat packet, trusted player signature, signed
message chain or remote signing key is forged. The existing Play Login's online
mode remains true, `enforcesSecureChat` false; initialized remote chat session
is absent. A welcome system message explicitly says chat signatures are not
verified. Incoming chat-session metadata remains bounded/discarded. Commands,
completions, reporting/signature validation and private messaging are not added.

Attack tolerance was added after the first real-client run: an accidental left
click sent this packet and closed that session when the packet was unsupported.
It now has no effect. F/off-hand swap is Player Action enum ordinal 7; inventory
actions remain outside scope. The second run's early rejection was independently
explained by the user pressing F. The final clean run avoided unsupported
inventory controls and recorded zero protocol failures/timeouts.

## Local policy, not wire requirements

M1D preserves M1C coordinate, packet-count, frame, compression, ACK and keepalive
limits. New policy is 128 KiB/s per-connection ingress work (wire plus decoded),
charged before the shared 8 MiB/s allowance; fixed one-second windows, not exact
bandwidth shaping. Trusted library test receivers explicitly raise their own
limit for clientbound terrain. Production server sessions do not.

Player state publishes at most every 50ms when dirty. Each publisher keeps only
its latest pending snapshot and at most one queued update notice. Viewer state
and rosters also coalesce. The directory services one command, then up to 31 more with a 1ms check on
additional batch work, and yields. A single command is not preempted. These choices do not create a world tick or promise 20 FPS under load.

Chat admits a three-message burst and refills one message/second **before**
shared ingress. The directory independently enforces sender/fanout policies.
Its aggregate fanout allowance is 8,192 deliveries/second with equal burst
capacity. Chat storage is 32 bounded messages per viewer, each generated from
the validated 256-unit text and bounded name; current generated packets are
under 1 KiB. Overflow closes only that slow viewer. Commands are bounded to
2,048 slots; each registered lease reserves a leave slot. No network await or
global movement mutex exists in the directory. A short per-player pending-state
mutex copies snapshots only; it never covers I/O or serialization.

## Tests and remaining limits

Tests exercise exact spawn/path/metadata/control fixtures, signed and unsigned
chat shapes, all truncated chat prefixes, nonzero acknowledgements, excess text,
control/formatting characters, modified UTF-8, a 20,000-case bounded Play corpus,
negative-coordinate visibility, latest-state coalescing, slow readers, stale
generation tokens, cancelled joins and full-queue leave cleanup. A separate
encrypted/compressed TCP test validates info-before-spawn, moves/pose/chat,
leave/rejoin and isolation from a spoofed own-entity command. It is a private
post-auth fixture, not evidence of account authentication.

Real-account client evidence and exact workload/resource definitions are in
[the M1D report](M1D_ENGINEERING_REPORT.md). Remaining unverified areas include
all legal controls/latency extremes, secure chat chains, additional versions,
heterogeneous terrain, Windows and M2A world isolation. No later protocol layout
or gameplay system is inferred from the tested subset.
