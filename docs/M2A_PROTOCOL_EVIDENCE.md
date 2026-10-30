# M2A protocol evidence — isolated instances

Date: 2026-10-29 (America/Toronto). Protocol **777 / Java Edition 26.3**.
Status: M2A accepted following user-reported real stock 26.3 client verification,
supplementing the automated post-authentication wire evidence. The user reports
all requested checks passed; specific two-world joining/isolation/relog observations
and provenance are in [the acceptance record](../evidence/m2a-stock-client-user-verification.json).
No new agent-captured stock trace or visual recording is claimed.

## Source evidence and unchanged layouts

No packet ID, field, registry asset, NBT layout, compression setting or authentication
mode was added. [M1D preservation hashes](../evidence/m2a-m1d-preservation.json)
compare current auth, network, protocol and chunk files/manifests with the final
M1D implementation inventory; every compared file is byte-identical. Pinned
field/path evidence remains in [M1C](M1C_PROTOCOL_EVIDENCE.md) and
[M1D](M1D_PROTOCOL_EVIDENCE.md). Those ledgers distinguish observed reference
behavior from local policies and stock-client observations.

A fresh read of the [public packet table](https://minecraft.wiki/w/Java_Edition_protocol/Packets)
was blocked by the browsing service (restricted URL). No fresh public field-level
cross-check is claimed. M2A reuses existing verified encoders; no speculative
Respawn layout was added. No new Mojang reference-archive inspection, source translation or
redistribution of Mojang bodies/assets was performed.

| Existing clientbound packet | M2A use | Evidence classification |
|---|---|---|
| `0x4e` entity removal, `0x46` player-info removal | Clear old entities/Tab entries | Existing layouts; new ordering is a design decision exercised by the headless decoder |
| `0x26` chunk unload | Unsubscribe all sent source-world chunks | Exact 25 removals verified in switch fixture |
| `0x60` center, `0x63` spawn, `0x27` start loading, `0x49` position sync | Reinitialize interest and teleport in the same dimension | Existing encoders; stock renderer/loading-screen behavior unverified |
| `0x2e`, `0x0c`, `0x0b` chunks/batches | Shared base followed by target overrides before batch completion | 25 initial + 25 target chunks; bounded batch ACKs decoded |
| `0x08` block update | Replay buried overrides, live changes and reversions | Exact position/state decoded, including negative coordinates |
| Info/spawn/path/head/pose | Scope to selected directory | Four fixtures at identical coordinates; exact entity sets before/after switch |
| `0x7c` system chat | World-scoped relay | Synthetic sender names checked; no signed-player-chat claim |

Teleport acknowledgements retain complete destination position/rotation fields.
Switch teleport IDs advance within the connection. Switching waits for prior
teleport and chunk batch ACKs, so an old batch ACK cannot acknowledge a new-world
batch. Movement is suppressed during a pending teleport; visibility follows
confirmation and sent chunks. Keepalive/chat limits remain connection-scoped.

## Automated observation

`m1d_probe::m2a_wire_isolation_edits_switch_reset_destroy_and_cleanup` runs four
concurrent TCP clients, assigned round-robin to two worlds. It uses real
AES-CFB8/compression/framing, the Play loop, chunk writer and directory. The
independent packet reader checks info-before-spawn, membership, chat text, block
positions/states, unloads and unique teleport acknowledgements.

A/C see only one another; B/D see only one another. An A-world edit and reversion
reach A/C only. A negative-coordinate B override is replayed to A after A switches
to B. A receives two teleports, 25 unloads and 50 chunks; A then sees B/D, C sees
nobody, B sees A/D. Occupied reset/destroy are rejected. After clients close,
reset expires handles, destruction rejects edits, and connections/replicas/edges/
frame leases return to zero without protocol errors, timeouts or task failures.
See [regression output](../evidence/m2a-tests.txt).

The boundary is **post-authentication Play** with synthetic identities and a fixed
test-only key. It does not exercise Handshake/Login/Configuration, Mojang session
validation, stock rendering, inventory or gameplay. Production cannot enable this
fixture. Existing auth/Configuration tests run separately; unchanged M1D account
evidence is linked, not relabeled.

## Policies and unverified questions

WorldId is process-unique; WorldHandle additionally carries reset generation.
Neither is client-provided authority. Arenas share the existing `smolmc:bootstrap`
wire dimension/world key. Switching uses removals/unloads/re-subscription, **not
Respawn**. Whether the stock 26.3 renderer/loading UI accepts every switch sequence
has user-reported acceptance; detailed live switch steps were not separately supplied. If it does not, investigate exact protocol-777 Respawn
evidence before adding it.

Trusted Rust edits allow Air/Stone at `-64 <= y < 63` and existing horizontal
bounds. The intact y=63 roof preserves shared heights and dark buried lighting.
No surface edits, player placement/breaking, lighting simulation or arbitrary
block-state support is claimed. This is not a general editable creative world.
