# M4B — Items, Drops, Durability & Armor

User acceptance 2026-10-30; final audit 2026-10-31. Checkpoint `m4b-before` preserves accepted M4A `d6371cc` /
`m4a-validated`. **M4B is user-accepted and validated**, with final milestone tag
`m4b-validated`. No later milestone is authorized.

## Goal and invariants

Two unmodified 26.3 clients mine supported blocks, see and collect real dropped
items, use counted inventories and durability-bearing equipment, equip armor and
fight with armor mitigation. Rust owns the decisions; Java is optional. Shared
immutable templates, sparse COW, authentication/encryption, instance isolation,
bounded queues and accepted M3 unarmored knockback/contact behavior remain.

The user first reported item gameplay “Seems fine,” then requested replaceable
grass placement and actual 26.3 Netherite knockback resistance. After those were
deployed the user replied “Yup,” but reported rapid placement/mining and armor
clicks being rejected. Those reports prompted input-pipeline and delayed-mining
corrections. After restarting the corrected server and requesting the focused
rapid mining/placement and armor-menu retest, the user replied **“Seems good.”**
[Acceptance evidence](../evidence/m4b-vanilla-client-acceptance.json) records these
stages and the accepted binary hash. This is the user's broad confirmation of the
requested two-client scenario and latest corrections, not an independently
observed, individually timed certification of all 18 checklist steps. Final audit
then tightened disconnect snapshot/receipt lock ordering and added a concurrent
regression. The acceptance record distinguishes the user-tested build from this
final lifecycle correction; no additional GUI observation is invented. Client
observations are user-reported; computer access to Minecraft Launcher was not
approved. Actual codec tests do not replace GUI testing.

## Implemented scope

| Area | Behavior |
|---|---|
| Stacks | Validated registry ID/count/damage; maximum counts and durability; default components plus a real damage patch |
| Drops | 416 bounded deterministic loot rules; harvest-tool validation, stone→cobblestone, dirt/planks/colored wool and supported simple terrain loot |
| Entities | Actual item spawns/metadata, static-terrain movement, 500ms mining pickup delay / 2s player-drop delay, partial/full pickup, compatible stack merging, five-minute lifetime and removals |
| Ownership | World-local entity/cell/viewer indices; reset/destroy/last-departure cleanup; reserved budgets and generation/token checks |
| Durability | Iron/diamond swords and iron pickaxe; applicable mining/attack costs, damaged stacks, breakage, authoritative inventory/equipment reconciliation |
| Armor | Leather/iron/diamond/Netherite normal slots, vanilla points/toughness/mitigation, wear and breakage; visible equipment and own attributes |
| Netherite | Actual default resistance modifiers scale base, sprint and sweep strength by `1 - resistance`; unarmored equations and landing policy remain |
| Menu | Left/right pickup/place, splitting/merging, shift-click, hotbar/off-hand swaps, Q/Ctrl-Q, supported cursor dropping/double-click collection, armor equip/unequip and use-to-equip |
| Prediction | Valid stale-revision clicks execute against Rust state then fully resync; no client-predicted counts/components become authoritative |
| Terrain actions | Up to 16 admitted inputs queue behind commit; bounded lighting-busy retries; early mining finishes complete on shared deadlines, including consecutive starts |
| Lifecycle | Explicit arena-kit/keep/clear policies; in-process reconnect cache preserves counts/damage/cursor/equipment and resolves pending commit receipts first |
| Java | SDK 0.4.0/schema 1.3 immutable typed inventory/equipment/durability/armor snapshots; older 1.0/1.1/1.2 subsets remain negotiated |

`--inventory-policy arena-kit` remains the default. It retains controlled M3 kit
and reset-on-respawn behavior, unlimited M4A building supplies and rejected item
drops. `items-keep` and `items-clear` require imported terrain and enable real
counts, consumption, drops, durability and armor. Both start with the explicitly
provided test/arena equipment; keep retains it through death, clear destroys the
inventory/cursor at death and respawns empty. Neither mode implements survival
death loot or disk persistence. Restarting the process loses in-memory inventories.

## Source evidence

[Protocol ledger](M4B_PROTOCOL_EVIDENCE.md), [ADR-011](../decisions/ADR-011-m4b-authoritative-items.md),
[security review](M4B_SECURITY_REVIEW.md) and `evidence/m4b-reference.json` distinguish
observed reference behavior from local arena policies and unverified scope.

Original adapters call the installed 26.3 codecs and primitive mechanics. Exact
decode/consume/re-encode covers component inventories, item spawn/metadata,
equipment/attributes, collection, motion and removal. Independent checks compare
72 Rust armor outputs, actual durability/break transitions, catalog component
facts, 12 actual knockback-resistance vectors and replacement flags. Supported
loot facts are derived conservatively from private tables; this is not a complete
runtime loot-engine comparison. No source bodies, JARs, terrain or assets are shipped.

## Design trade-offs and failure corrections

The existing directory owns items and interaction ordering. One shared deadline
set services only worlds with actual item/mining/retry work. No extra per-world,
per-player or per-item task/thread/timer is created; inactive worlds are not scanned.
Nearby cell candidates drive pickup and merging. Remaining dropped entities are
discarded when the last player leaves under the explicit ephemeral arena policy.

Inventory effects follow successful terrain commits through reserved directory
receipts, independently of client writes. Failed transactions cannot wear tools,
consume placement items or produce loot. Disconnects mark the old session inactive;
its pending receipt finalizes the cached inventory before a new session restores it.
The bounded cache never evicts an entry to grant a repaired starter kit.
Final review caught a race between disconnect's cache write and a pending receipt:
the old snapshot could overwrite the finalized inventory. Disconnect now holds its
session guard through the cache write, using the same game→cache order as attach
and receipt completion. A 128-iteration concurrent success/failure regression
checks that reconnect always restores final damage and clears pending state.
The shared cache lock is confined to join/leave/disconnected finalization; live
movement, combat, clicks, pickup and normal terrain completion do not acquire it.

User feedback exposed an overly strict assumption: a mismatched inventory revision
was treated as rejection. Actual vanilla executes the click and then resynchronizes.
The correction retains authoritative slots/counts/components while allowing normal
client prediction. Terrain work similarly queues bounded valid input instead of
discarding a mining start that arrived while placement lighting was finishing.
Early finishes retain their original server mining facts on shared deadlines;
subsequent starts do not erase those records. Busy lighting retries are bounded.

A dense armored sweep exposed too many reliable deliveries per action. Equipment
changes now share hurt packets, and per-recipient action results group in order
without adding a 50ms buffer or enlarging the existing queue. Packet/byte caps and
slow-reader closure remain. One fixture initially rejected equipment addressed to
the local player's own entity; the actual client supports that packet, so its
assertion was corrected. Failed runs remain diagnostic, not performance successes.

Replaceable grass uses actual state metadata. Matching tall-grass/large-fern halves
are edited together with expected-state checks. Solid grass blocks remain solid.
This is a narrow authorized correction, not a general neighbor-update simulator.

## Dependency and compatibility impact

No new external dependency or lockfile change. No unsafe Rust, JNI, custom crypto,
Bukkit/Paper host or required JVM. Bridge-off retains native gameplay and starts no
Java listener/event pipeline. SDK 0.4.0 adds a read-only operation and negotiates
schema 1.3; older peers keep their advertised SDK versions and subsets.

## Resource budgets

| Resource | Default / bound |
|---|---|
| Item entities | 1024/world, 8192 shared, including pending loot reservations |
| Configuration | `--max-items-per-world 1..4096`; shared up to 32768 and at least the per-world budget |
| Visible items | At most 256/viewer; spatial candidates refill freed edges |
| Item deliveries | ≤1024 packets / 64 KiB per group, split into existing ≤64 packet / 16 KiB transport writes |
| Queued reliable payloads | 128 KiB logical charge/viewer, 8 MiB shared; leases release on write/drop; allocator/assembly overhead is separate |
| Combat deliveries | Existing 32-message queue; ≤128 entries / 64 KiB per action group |
| Input waiting behind commit | 16 admitted operations/player |
| Delayed block work | 8 records/player; finite scheduled completions, not periodic player ticking |
| Lighting contention | At most 16 retries, spaced 20ms; existing eight-slot/one-worker lighting design |
| Reconnect identities | 1024 bounded in-process entries; new identities fail safely when full |

Full inventories leave uncollected remainders in the world. Entity-budget rejection
keeps the original inventory/block. Resource caps are not capacity promises.

## Measurable acceptance and validation

[macOS manifest](../evidence/m4b-macos-meta.json) and
[Linux manifest](../evidence/m4b-linux-meta.json) link the final sources and raw
logs. Both passed formatting, strict all-target Clippy, full workspace tests,
production/examples release builds, Java SDK checks, four real encrypted Java
integration cases, bridge fault injection and listener recovery. The workspace
run has 147 successful executions (141 distinct test bodies; six bridge tests
also run through the server module), zero failures and six explicitly ignored
tests. Four Java cases were separately run successfully; the two resource probes
were exercised in the short measurement runs. Java compiles with `--release 17`;
macOS and Alpine arm64 Linux used their installed JDKs. Preserved SDK 0.1.0,
0.2.0 and 0.3.0 JARs also passed against the final server.

Independent actual reference checks passed for the new item codecs/mechanics,
old M3 codecs and classic arithmetic, and M4A's 16 chunks / 1,572,864 cells,
48 mining and 72 collision cases. See
[item oracle](../evidence/m4b-reference.json),
[terrain oracle](../evidence/m4b-terrain-reference.json),
[terrain mechanics](../evidence/m4b-terrain-mechanics.json),
[M3 codecs](../evidence/m4b-m3-reference-codecs.json) and
[classic oracle](../evidence/m4b-classic1194-reference.json).
The twelve protected M3 function bodies remain identical to the checkpoint
([source comparison](../evidence/m4b-m3-invariants.json)).

Linux validation used an ephemeral Alpine arm64 musl VM, Rust 1.99.1, OpenSSL
3.5.9 and the installed OpenJDK 25 (exact version in the manifest). Initial infrastructure attempts exposed insufficient
2 GiB guest memory, a disabled loopback interface and stale Cargo timestamps
after extracting source archives. These were corrected with a 4 GiB guest,
loopback setup and explicit source timestamp refresh before the final all-pass
run. They are not counted as successful runs or Linux performance evidence.
One later macOS fixture reset immediately after closing its sockets and observed
the correct `Occupied` rejection while server membership was still draining.
It now awaits empty membership with a bounded deadline before asserting reset;
the production occupied-reset protection remains intact.

The focused interaction regression reproduces same-revision armor clicks,
place→immediate mine, premature finish, a subsequent mining start before the
first completion, and two simultaneous placements. Separate tests cover pending
disconnect receipts, count conservation, merging/full/partial/competing pickup,
durability/breakage, armor, policies, isolation, cleanup and reliable queue limits.

## Short resource measurements

[Baseline and diagnostic runs](../evidence/m4b-performance.json),
[final M4B runs](../evidence/m4b-performance-final.json) and
[item/mining probes](../evidence/m4b-gameplay-resources.json) retain raw output and
binary hashes. CPU is percent of one core; RSS is whole Rust PID KiB. These cases
run without Java, so JVM memory is not applicable. The optional manual-arena Java
helper is a separate process and is not included. Active comparisons use the
same 20 Hz synthetic encrypted clients, imported-map translation/platform and
approximately eight-second sampling window after readiness. The server and load
generator are separate processes; these are local regression workloads, not
internet capacity or long-soak certification.

| Workload | M4A CPU % | M4B CPU % | M4A RSS KiB | M4B RSS KiB |
|---|---:|---:|---:|---:|
| 128 players / one world movement | 29.50 | 30.08 | 27472 | 27792 |
| 256 players / 16 worlds movement | 16.67 | 26.71 | 26960 | 25936 |
| 32 players / two worlds arena combat | 2.84 | 6.44 | 19600 | 19856 |
| 32 players / two worlds real items, iron armor and durability | n/a | 6.32 | n/a | 20112 |

CPU increased in these final samples; no speedup or statistically isolated
overhead estimate is claimed. Earlier passing samples varied materially (for
example M4A movement 32.01%/22.43%, armored M4B combat 3.77%). Host scheduling,
short duration and extra item/queue authority all limit attribution. Comparable
movement output was retained: 128-player published updates 20532→20465 and
position packets 2,487,575→2,482,459; 256-player updates 41147→40608 and position
packets 611,793→602,548 (sampling windows 8.159→8.051 seconds). All final runs kept
every client connected, recorded zero slow viewers/protocol or I/O errors, and
returned active replicas, visible edges and retained frame bytes to zero.

Idle baseline→final RSS was 10640→10688 KiB without imported terrain and
16864→17120 KiB with it, one Rust thread. Both three-second samples observed
0.00% cumulative CPU at the sampler's 0.01-second resolution; this is below
measurement resolution, not proof of mathematically zero CPU.

The release item-owner microprobe used one viewer, default quotas, real spatial
maps and queue drains. From 7456 KiB baseline, 100 items used 8096 KiB (+640),
1000 used 8944 KiB (+1488); allocator/page granularity prevents an exact per-item
byte claim. One hundred settled passes averaged 11.14 µs / 112.77 µs respectively.
Spawning and merging 1000 single items took 1228.96 µs, leaving 16 entities and
all 1000 items; pickup processing took 181.17 µs and conserved all 1000. This
isolates native ownership work and excludes sockets/encryption/rendering.

The encrypted mining/drop/pickup/armor fixture completed in 2.87 seconds,
19760 KiB RSS, 3.57% CPU. The rapid placement/consecutive-mining/menu fixture
completed in 1.30 seconds, 19392 KiB RSS, 9.74% CPU. These fixture PIDs include
Rust clients and server, so CPU/RSS
are explicitly inclusive and cannot be compared with server-only load rows.
Inventory/equipment-containing reliable packet writes had a histogram p99 upper
bound of 256 µs in both small fixtures (30/32 samples); block commit and
enqueue-to-write upper bounds were 32.768 ms for both small fixtures (only 1–6
block samples). Full armored-load reliable writes had p99 ≤4.096 ms, action
processing ≤8.192 ms; movement sampled
publish-to-write p99 was ≤65.536 ms. These are server histogram bounds, not
client input, RTT or render latency guarantees.

Failed armored runs diagnosed queue pressure and a fixture self-equipment
assertion. Earlier intermediate successes and failures remain in the diagnostic
evidence; only `m4b-performance-final.json` and `m4b-gameplay-resources.json`
describe the accepted final binary/probe. Failed/canceled runs are never treated
as lower-resource improvements.

## Open questions and deferred mechanics

Only the supported tools, armor and default/damage component subset are covered.
Crafting/furnaces, containers/drag-crafting, enchanting/Fortune/Silk Touch/Unbreaking/
Mending, shields/effects, custom attributes, hunger/regen, mobs, fluids/redstone,
general neighbor/entity physics, arbitrary components and survival persistence
remain deferred. Unsupported loot variants produce no invented normal loot.
Full vanilla simulation and Bukkit/Paper compatibility are not claimed.

## Rollback and stopping point

To retain the current engine with controlled inventory semantics, restart with
`--inventory-policy arena-kit`. For exact M4A rollback, stop only the relevant
server/sidecar and create a separate checkout:

```sh
git worktree add --detach ../SmolMC-pre-M4B m4b-before
```

Use a separate target directory and SDK 0.3.0 for that M4A server. No persistence
migration exists; edits/inventories/health/handles are ephemeral. The source
`USE THIS` save remains read-only and needs no reverse conversion. Preserve
evidence before removing artifacts. The final commit is identified by
`m4b-validated`; stop at M4B. No M5 work is authorized.
