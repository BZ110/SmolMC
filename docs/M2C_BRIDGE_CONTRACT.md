# M2C bridge contract with revised M3 extension

Design decision, implemented scope: optional local Unix IPC, schema **1.1**,
SDK semantic version **0.2.0**, Java **17+** source/bytecode. Schema 1.0 / SDK
0.1.0 peers remain supported on their original operation/event subset. macOS and Linux.
No JNI, raw pointers, remote control endpoint, plugin loader or Bukkit/Paper ABI.
Rust owns gameplay, identities, worlds, packet ordering and resource limits.

## Enable, attach, launch, close

```sh
target/release/smolmc --bind 127.0.0.1:25565 \
  --bridge-dir /tmp/smolmc-private-new-path --bridge-access control
scripts/java-check.sh
java -cp target/java/classes TinyArena target/release/smolmc
```

`--bridge-dir` must not exist and its parent must exist. Creation is exclusive:
0700 directory, 0600 `socket` and `token`, 32 random bytes supplied by OpenSSL.
No credentials in CLI/environment/logs. Socket filesystem restrictions plus the
credential authenticate the configured OS-user trust domain. The default grant
is **observe**; **control** must be explicit. Grant is uniform for everyone with
that credential, not an untrusted-plugin/per-world permission sandbox. Observe
allows snapshots and lifecycle subscriptions, including player identity snapshots;
control additionally permits mutations, player messages/transfers and commands.
Never give an observe credential to someone who may read the server's control
credential/directory as the same OS user. No new player-authentication mode.

`MinecraftRustServer.attach(directory)` synchronously negotiates (5s bound), then
returns a control connection. `launch(executable)` owns one Rust child with one
bridge; returns `LaunchedServer`, exposing `.server()`, `.process()`, `.directory()`.
Rust does not launch Java. Attach close disconnects only that peer; launched close
also SIGTERMs/joins its Rust child (5s, then forced termination). Private bridge
files disappear after graceful Rust shutdown; launch logs remain in the private
parent for diagnostics. A SIGKILL/crash can leave a stale directory: confirm its
server is stopped, then remove that owned directory before choosing/reusing a
path. Never automatically unlink an existing socket or take over a running owner.
The bridge must be enabled before Play membership; late enabling is rejected
instead of hiding existing players. Control-bus lifetime is the Service lifetime;
this is a startup option, not a runtime feature-toggle framework. Bridge shutdown
closes/joins IPC peers; Service/session shutdown releases their control resources.
Bridge-off constructs no listener, credential, event ring, command watcher, player
message queue or JVM; existing session/world controls remain event-driven.

## Java-facing operations

The standalone SDK has no third-party Java dependency. All control operations
return `CompletableFuture`; only attach/launch/close manage blocking process/I/O
lifecycle. The SDK JAR is `target/java/smolmc-sdk-0.2.0.jar` after the check script.

- `templates().load("stone_flat")`: bind the single built-in immutable template.
  Other names/file imports fail explicitly; no arbitrary template parser.
- `worlds().defaultWorld/list/create/cloneWorld/reset/park/destroy/resources`:
  existing M2B lifecycle, fixed supported editable/read-only profiles. Create/clone
  adds to the existing round-robin join routes. Reset/destroy reject occupied
  worlds. Clone shares immutable state; mutations remain isolated.
- `worlds().editBatch`: transactional ≤4096 Air/Stone edits, existing
  `-64 <= y < 63`, x/z ±100000 bounds; duplicates are input-order last-write-wins.
  `readBlocks`: ≤4096 positions, -64..319, x/z ±100000. There is no surface editing,
  lighting simulation, inventory interaction, persistence or per-coordinate IPC loop.
- `players().list/message/teleportToSpawn`: verified identity/session snapshots,
  bounded system messages (prefixed `[System]`), transfer to a world's fixed spawn.
  No arbitrary coordinate teleport/game-mode/inventory API in this milestone.
- `commands().register/unregister/invoke`: one literal `[a-z0-9_]{1,32}`, optional
  greedy plain-text arguments. ≤64 registrations globally, unique name, ownership
  by connection; disconnect unregisters. Player unsigned slash packets and trusted
  operator invocation produce asynchronous command events. Every player can invoke
  an advertised registered literal; Java must validate arguments/game rules.
  No permission tree, Brigadier argument framework, custom completion or plugin host.
- `subscribe(Set<EventKind>, Consumer<Event>)`: one subscription per connection;
  returned `Subscription.unsubscribe()` gives an async barrier. `.close()` initiates
  unsubscribe. Callbacks already running may finish; queued callbacks check local
  cancellation. A new subscription is permitted after the unsubscribe receipt.

World handles contain server incarnation UUID + WorldId + generation. Reset
returns a new generation; callers must replace their old handles. Player handles
contain server UUID + monotonic nonrecycled entity session ID + verified player
UUID. Reconnect is a new handle even for the same account. Old server handles fail
before sending in the SDK; Rust also checks the envelope incarnation. Invalid
player UUID cannot target another player. A list/get is a cold snapshot, not a
live mutable object or a network-rendering barrier.

Mutation replies mean **committed control state**, except player messages and
transfers, which mean **accepted**. Existing transfer requests coalesce. Membership
commits after outstanding chunk/teleport ACKs; `PLAYER_WORLD_CHANGED` announces
that commit before subsequent terrain/position delivery. Invalidated destinations
safely close that player session. A successful message receipt means queued for
that player, not rendered. Neither response nor event is an ACK barrier for clients.
A transfer to the current world is a no-op. No durable exactly-once/replay facility.

## Wire layout (big endian, no Rust/Java native layouts)

Every frame is `u32 bodyBytes` then body, **10..131072 bytes**, checked before
allocation. Strings are `u16 byteCount + strict UTF-8`, with operation-specific
bounds. No modified UTF-8/Java writeUTF. Counts are `u16`; flags/materials are `u8`.
World = `u64 id, u64 generation`; player snapshot = `i32 entity, UUID16,
string(name<=64 bytes), world`. UUID is 16 bytes in network order.

Request: `u8=1, u64 correlation, u8 opcode, body`. First request is opcode 0:
`u16 major=1, u16 minor=0, credential32`. No incarnation prefix for handshake.
All later request bodies start with server-incarnation UUID16. Correlations are
positive, strictly increasing per connection; duplicates/out-of-order close the
peer. Negotiation accepts minor 0 or 1 and echoes it with API 0.1.0 or 0.2.0
respectively. The new SDK requires 1.1 / 0.2.0; old peers retain 1.0 / 0.1.0.
Minor-0 peers cannot call new operations or subscribe to new events. Breaking
changes still require major negotiation.

Response: `u8=2, u64 correlation, u8 opcode, u16 status, payload`.
Handshake success payload: `UUID16 incarnation, u16 major, u16 minor,
string API version, u8 controlGranted`. Error payload: bounded diagnostic string.
Errors describe accepted control outcomes, never include secrets or raw requests.

| Opcode | Request after incarnation | Success payload |
|---|---|---|
| 1 default world | empty | world |
| 2 create | u64 template=1, u8 profile (0 read-only / 1 editable) | world |
| 3 clone / 4 reset | world | world |
| 5 destroy / 6 park | world | empty |
| 7 resources | world | string state (`Ready/Active/Parked`), u8 editable, six u64: players, edits, modified chunks, logical charges, owned tasks, owned timers |
| 8 batch edit | world, u16 count≤4096, repeated i32 x/y/z + u8 material (0 Air / 1 Stone) | empty |
| 9 read batch | world, u16 count≤4096, repeated i32 x/y/z | u16 count, repeated material u8 |
| 10 list players | empty | u16 count≤512, repeated player snapshot |
| 11 transfer to spawn | i32 entity, UUID16 player, world | empty (accepted) |
| 12 system message | i32 entity, UUID16 player, string≤1024 bytes, no controls | empty (accepted) |
| 13 subscribe | u32 mask: bit k for event k; minor 0: 0x7fe, minor 1: 0x3ffe | empty; replaces that peer's ring cursor |
| 14 unsubscribe | empty | empty |
| 15 register / 16 unregister | string name≤32 bytes | empty |
| 17 operator invocation | string command≤256 bytes, no control chars | empty (event emitted) |
| 18 list worlds | empty | u16 count≤4096, repeated world |
| 19 combat snapshot (minor 1) | i32 entity, UUID16 player | f32 health, u8 dead, u8 selected hotbar, i32 main item, i32 offhand item |

Event: `u8=3, u64 sequence, u8 kind, world (0/0 for operator command),
u8 playerPresent, [player snapshot], string text≤256 bytes`. Kinds:
1 created, 2 cloned, 3 reset, 4 destroyed, 5 batch edited, 6 parked, 7 player joined,
8 player left, 9 player world committed, 10 command. Text is empty except the full
command string. Events are read-only/noncancellable snapshots. No per-move/chat/
chunk/packet event serialization. Join is membership admission, not fully loaded
client; leave is RAII membership release, including cancellation. A cloned event
describes the new world; reset describes its fresh generation; destroy its retired
handle. Lifecycle mutation retains the cold catalog lock. Sequence assignment and
bounded broadcast publication share a short optional event mutex (no IO, Java,
await or world mutation), preserving order even for concurrent trusted Rust callers
without taking the catalog lock for combat notices. Server order is preserved per peer's event stream; futures may complete on
different Java workers, so callback and future completion order can differ.

Sequence is global and may skip masked events or commands owned by other peers;
only the owner receives its command events. Use snapshots after reconnect, then
subscribe/list and reconcile by IDs. There is no atomic snapshot+replay cursor
or durable event history. Event gaps from ring overwrite close the peer, never
silently claim reliable delivery. Current connection close is the resync signal.

Statuses: 0 success; 1 InvalidRequest; 2 VersionMismatch; 3 PermissionDenied;
4 StaleHandle; 5 NotFound; 6 Occupied; 7 BudgetExceeded; 8 UnsupportedFeature;
9 Backpressure; 10 Conflict; 11 WorldNotReady. SDK additionally exposes Timeout
and Disconnected. Invalid payloads/trailing fields are rejected **before mutation**;
invalid framing/auth/correlation, partial-frame timeout or event gap closes the
peer. No automatic retry of mutations. On timeout/disconnect an operation may
already have committed; reconnect, inspect snapshots and reconcile explicitly.
Cancelling a Java future cancels the receipt, not an already admitted Rust command.

## Bounded work and failure isolation

| Resource | Bound / behavior |
|---|---|
| Bridge peers | 4, excess closed; no task per world |
| Rust frames | 128 KiB, one incoming + one response per peer; no unbounded request queue |
| Rust request work | shared 2048 requests / fixed 1s window; 16384 edit inputs / window; excess explicit Backpressure + 1ms peer yield |
| IO | handshake 2s, partial frame total 2s, each write 2s; authenticated idle socket sleeps without a polling loop |
| Events | shared 128-entry bounded broadcast ring; overflow disconnects lagging receiver |
| Commands | 64 global literals, owner cleanup; one shared command-tree watch |
| Player system messages | 8 queued / enabled Play member, ≤1024 text bytes each; try-send rejection |
| SDK calls/outbound | 64 end-to-end slots, including completion work; 64 outbound frames; 5s deadline closes ambiguous connection |
| SDK events | 128 tasks + one running callback; overflow closes with Backpressure |
| SDK completions | 2 workers, 128 terminal/normal tasks; bounded by 64 end-to-end slots |
| SDK threads | 2 I/O, 1 deadline, up to 2 completion + 1 callback; per connection, not per world; JVM infrastructure measured separately |

Standard CompletableFuture rules apply: actions attached before completion can run
on SDK completion workers; actions attached after completion may run on the caller.
Compose futures, and use an application-owned executor for blocking work. Arbitrary
Java code cannot be forcibly terminated safely; an uncooperative callback can keep
its Java worker busy despite interruption. Queue/call bounds and process isolation
protect Rust. SDK close signals/interrupts its owned workers; graceful cooperative
callbacks retire. Callback exceptions disconnect that peer. A killed/frozen JVM
cannot hold a Rust world lock, execute on a networking callback, or prevent other
peers and Minecraft sockets progressing. Committed worlds survive sidecar loss;
the app must manage/reconcile those worlds. No automatic destructive cleanup of
worlds on disconnect.

## Protocol-777 command evidence and boundaries

Observed user-owned reference inventory (not redistributed source):
`unz/net/minecraft/network/protocol/game/GameProtocols.java` gives unsigned
serverbound chat command index **0x07**, clientbound commands index **0x0f**.
`ServerboundChatCommandPacket.java` encodes a single UTF-8 Minecraft string.
`ClientboundCommandsPacket.java` defines root/literal/argument flags, child indexes,
name and argument-parser registry ID; `ArgumentTypeInfos.java` places
`brigadier:string` at index **5**; `StringArgumentSerializer.java` encodes greedy
phrase as VarInt **2**. SmolMC's bounded independent encoder/decoder sends one root,
executable literal plus executable greedy argument per registered name. Signed
command 0x08 remains unsupported; no signature verification claim. No Mojang
implementation bodies/assets are included. Headless encrypted fixtures check this
layout and player invocation; no new graphical stock-client rendering is claimed.

Primary platform references: [JDK SocketChannel](https://docs.oracle.com/en/java/javase/26/docs/api/java.base/java/nio/channels/SocketChannel.html)
(concurrent read/write Unix sockets) and [Tokio bounded broadcast](https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html)
(explicit lag detection). Implementation targets pinned Cargo.lock and Java 17
APIs; platform references do not constitute minigame/protocol test evidence.

## Revised M3 additive combat observations

`players().combat(handle)` returns a cold authoritative `CombatState` snapshot;
PvP-disabled sessions return UnsupportedFeature. UUID/incarnation checks remain.
Minor-1 event kinds 11 PLAYER_DAMAGED, 12 PLAYER_DIED, 13 PLAYER_RESPAWNED append
`CombatState` fields followed by `i32 sourceEntityId, f32 actualHealthLoss` to the
common event envelope. Respawn source/damage are zero. Actual health loss is capped
at remaining health, not raw overkill. Damage then death are emitted in that order;
respawn announces reset state before subsequent terrain/teleport ACKs. These are
not client rendering barriers, veto callbacks or mutation APIs. Typed Java Event
has nullable `CombatChange`; its former five-argument constructor remains available.
Observe grant permits these snapshots/events; no new control grant or Java combat
dependency. No per-movement event was added. Bridge-off constructs no event bus.

Protocol erratum (2026-10-30): the M2C command advertisement encoder used Play ID
0x0f. Actual 26.3 Commands is **0x10** (bundle delimiter occupies ID 0). Revised M3
corrects it and checks real vanilla codec decode/re-encode independently. Earlier
self-contained encrypted tests mirrored the wrong ID and did not establish stock
GUI command rendering. See [M3 protocol evidence](M3_PROTOCOL_EVIDENCE.md).

## M4A additive extension: schema 1.2 / SDK 0.3.0

Schema 1.0/1.1 still negotiate 0.1.0/0.2.0 and retain their subsets. Schema 1.2
adds profile byte 2 (`BUILD`) and operations below. Existing incarnation, grants,
request/event framing, rate budgets and sequence rules remain. Observe grants
cannot mutate. No IPC filesystem importer or arbitrary path input.

| Opcode | Request | Successful response |
|---|---|---|
| 20 | empty | template id u64, configured name string (`stone_flat`/`imported`) |
| 21 | supported protocol block-state i32 | id i32, name string, properties string, full-cube byte |
| 22 | world handle, u16 count and positions | u16 count, supported state i32 per position |
| 23 | world handle, u16 count, position + supported state i32 per edit | empty after atomic block+light commit |

At most 4096 positions/edits; unsupported IDs fail. Terrain edits consume the shared
edit budget and may return explicit backpressure while one bounded worker or world
transaction is busy. Stale/reset/destroyed results cannot commit. Existing edit event
kind 5 follows committed publication. `templates().configured()` discovers the
operator-selected template; `load(name)` verifies its name. No dynamic map loading
from Java. Old Air/Stone operations stay available on their representable subset;
legacy reads fail explicitly on other states instead of mapping them to Air.
See [M4A contract](M4A_TERRAIN_CONTRACT.md) and [M4A report](M4A_ENGINEERING_REPORT.md).
