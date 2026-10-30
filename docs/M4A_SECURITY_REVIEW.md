# M4A security and failure review

2026-10-30. Goal: treat map files and player packets as untrusted while preserving
M1A–M3 identity, isolated COW worlds, transport budgets and optional Java operation.

## Import boundary

Operator startup selects the world folder and aligned volume. IPC/network clients
cannot provide filesystem paths or execute content. Source files are opened read-only;
entity/POI/player/datapack files are not executed. No source save is written.
Actual source hashes and a result inventory accompany the engineering report.

Region files must be regular non-symlinks, sector-aligned and within size limits.
Location tables reject overlap, header/out-of-file sectors and missing selected
chunks. External/LZ4/custom compression is rejected. Decompression is capped before
NBT allocation and rejects trailing compressed bytes. NBT bounds depth, nodes,
strings, array/list lengths, offsets and exact consumption; duplicate keys, bad
palette indices/word counts, wrong version/coordinates/status and unknown biomes
fail. No data fixer, arbitrary dimensions or arbitrary world-format compatibility.

Canonical containment checks prevent supplied region names escaping the chosen
root; filenames derive only from bounded coordinates. This is not protection
against a privileged local user racing file replacement between validation/open.
Imports assume the operator controls that local source directory. Concurrent source
mutation is unsupported; save/exit the client before import. A parser failure does
not publish a partly imported template or bind the listener.

Strict mode rejects unsupported states. Explicit removal conversion records
unsupported palette entries and replaces them with air, omitting block entities.
It can remove fluids/decorations/containers and is lossy; it is not silently enabled.
Neither conversion nor import runs redstone, fluid, neighbor, entity or script logic.

## Runtime mutation and isolation

Player placement/mining uses authenticated directory membership/token, world
handle/generation, current accepted pose, chunk interest, reach, supported selected
item and server-owned inventory. Client hashes/predictions are not item authority.
Placement requires air, bounds and no player intersection, with occupancy rechecked
before committing a slow lighting result. Read-only worlds reject mutations.
Sender packet/byte budgets remain; mining/place/swing has its own bounded 64/s
burst-sixteen budget while combat retains 32/s burst eight. Slow critical readers
close; reliable state is not silently dropped.

Only one terrain transaction per world may be pending; one shared worker has an
eight-job queue plus one in flight. Lighting work is bounded by extent/frontier/
neighbor caps and does not hold catalog/player locks or run on the reactor. Commit
requires unchanged world generation and overlay snapshot. Reset/destroy invalidate
pending work; old completion packets carry the old handle and are filtered after
world switches. A failed/busy/over-budget transaction publishes neither blocks nor
lights. Strong scoped snapshots preserve source/sibling worlds. Retained old light/
block snapshots keep their accounting reservations until dropped.

Movement/combat consume immutable watch snapshots without the catalog lock. Nearby
collision queries cap candidate cells/shapes and coordinate arithmetic. The model
is a collision foundation, not a full anti-cheat: plausible malicious motion inside
the broad travel envelope, lag compensation and full entity/terrain physics remain
outside this milestone. Void recovery is an explicit acknowledged arena teleport.
No player/world ticking loop, required Java callback or per-world OS worker.

## Bridge and transport compatibility

SDK/schema 0.3.0/1.2 adds state/template/atomic edit controls. Observe-only grants
cannot mutate, handles remain incarnation/generation-bound, requests are bounded,
and terrain edits consume the shared bridge edit-rate budget. Old 1.0/1.1 subsets
negotiate their previous API strings; unrepresentable legacy material reads fail.
The bridge cannot import arbitrary server files. Java does not gate hits/collision.
Existing credential permissions, framing, deadlines, sequence publication and slow/
killed sidecar isolation remain.

Private protocol clients now use a separate bounded policy for receiving large,
highly compressible server chunk packets. Production server ingress remains capped
at 64 KiB/128x; its default work/byte/stream limits are unchanged. This adjustment
repairs the fixture, not a permission granted to untrusted player ingress. Full
network regressions include compression bombs and exact-stream/trailing checks.

## Evidence and limits

Adversarial NBT/region/compression/palette/extents/symlinks; inventory/reach/held
input; encrypted multi-client blocks, collisions, read-only rejection, isolation,
COW lighting, reset/stale completions and cleanup; real Java typed controls; old SDK
compatibility; existing bridge faults/listener recovery; actual codec/mechanics
oracles; full checks and Linux results are recorded in the M4A report.

No auth bypass, offline mode, account-token access/logging, new unsafe Rust, bespoke
crypto or JNI was added. Trace output remains opt-in numeric diagnostic metadata;
no ordinary chat bodies/names/credentials are retained in committed evidence.

Deferred: arbitrary map fidelity, full lighting/face occlusion, fluids, moving
blocks, full survival/effects/anti-cheat, drops/durability, persistence and a plugin
security boundary. In-process trusted Rust embedding can assemble invalid data;
it is not an untrusted-plugin sandbox. Stop after M4A.

Respawn/world-switch/void recovery checks the current edited terrain and chooses a
clear supported point within a bounded four-block horizontal radius if the configured
point is obstructed or unsupported. No safe point produces an explicit failure;
the accepted combat equations remain unchanged.
