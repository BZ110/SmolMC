# M4A — Real Worlds, Block Interactions & Terrain Physics

Original capture filenames refer to the [local validation archive](../evidence/README.md).

2026-10-30. Final checkpoint: `m4a-validated`. Rollback checkpoint:
`m4a-before` → `3f83f7d` (`m3-pvp-validated`). M4A is implemented and user-accepted.
No M4B, plugin ecosystem, broad survival, Bukkit/Paper compatibility or M5 work.

## Goal and real-client acceptance

Two unmodified authenticated Java 26.3 clients joined terrain imported from the
user-generated random-seed world **`USE THIS`**, built/mined supported blocks and
continued PvP. The final corrected mining/ground/stability/combat check received
**“Yes! Everything works. Continue on.”** [Acceptance evidence](../evidence/m4a-vanilla-client-acceptance.json)
records the questions, responses, failed intermediate builds and binary hashes.
This is user-reported acceptance; the agent did not operate/view game windows.
The response is broad acceptance, not individually timed certification of all
checklist steps. Encrypted fixtures additionally exercise isolation, reset, stale
handles, reconnect, death/respawn and consistent multi-client block publication.

The actual imported test volume is 16 chunks, X=-16..47, Z=-80..-17, full dimension
Y=-64..319. The original save is read-only. Explicit removal conversion replaces
unsupported states (including fluids and luminous cave decorations) with air and
reports each affected palette entry. Supported original terrain positions/states
remain exact. This is a bounded converted arena, not lossless arbitrary Anvil.

## Invariants and implemented capability

| Area | Result |
|---|---|
| Import | Bounded modern region/NBT decoder; exact 5023/full chunks; modern Overworld path; negative coordinates; exact palette words/indices and supported property lookup |
| Shared data | Immutable packed sections and encoded packets once/template; instances/clones share base and sparse block/light COW snapshots |
| Streaming | Arbitrary supported palettes, biomes, heightmaps, non-air/fluid counts, light masks/arrays, block/light corrections, existing batches/unloads |
| Interactions | Explicit imported `build` profile; authoritative inventory kit; validated placement and held mining; acknowledgements/prediction reconciliation; read-only rejection |
| Terrain physics | Nearby static collision boxes, floors/walls/ceilings, grounded/falling/landing, negative/chunk boundaries and bounded step-up; acknowledged corrections/void recovery |
| PvP | Accepted M3 classic equations, precision and ground settling preserved; imported contact uses actual support rather than predicted next-step landing |
| Java | Optional 0.3.0/schema 1.2 configured-template discovery, typed state reads and atomic edits; previous 0.1/0.2 subsets remain negotiated |
| Lifecycle | Reset drops edits/lights, clones diverge safely, destroyed/stale transactions cannot commit, reliable old-world completions are filtered on switch |

Authentication, encryption/compression, admission/byte/work/queue limits, spatial
replication/coalescing, combat scheduling fairness and world isolation remain.
No new world/player thread/task/tick loop. Java is never required for movement,
collision, mining authority or a hit. No persistence or imported content execution.

## Source evidence, design trade-offs and dependencies

[Terrain contract](M4A_TERRAIN_CONTRACT.md), [protocol/mechanics evidence](M4A_PROTOCOL_EVIDENCE.md),
[security review](M4A_SECURITY_REVIEW.md) and [ADR-010](../decisions/ADR-010-m4a-imported-arena-terrain.md)
separate observed runtime facts from deliberate arena policies and open limits.
The allowlist is 4,809 states / 447 block names. Original metadata adapters and
path/hash inventories contain no Mojang bodies, game assets or JARs.

Actual installed 26.3 codecs checked **1,572,864 cells** across 16 imported chunks,
exact packet decode/consumption/re-encoding, mining actions 0/1/2/3 and added light/
crack/game-mode packets. Actual runtime methods matched **48 mining vectors** and
**72 full-block collision vectors**. The M3 actual-codec and actual Paper 1.19.4
arithmetic checks still pass. These checks establish their exercised subset, not
full vanilla/Paper/lighting-engine equivalence. No Purpur was used.

Import and initial serialization run before listening, off the reactor. Edited
blocks and skylight commit atomically through one lazy shared worker, with eight
pending slots, one in-flight job and one pending transaction/world. Generation/
snapshot/occupancy checks guard commit. Busy/oversized/stale requests fail without
partial mutation. The static zero-emission light model rebuilds converted terrain
and an edit's bounded neighborhood; full partial-face light behavior is deferred.

Movement/combat read immutable watch snapshots without the catalog lock. Collision
checks nearby static boxes with a 64-box stack buffer and bounded fallback. Shared
encoded packets are sent directly; imported-session scratch is released after a
view completes. Per-client encryption/compression remains unavoidable and private.
The accepted flat M3 motion path remains available for virtual-stone worlds.
Join/reconnect, world switch, respawn and void recovery select a clear, supported
point on the current edited snapshot. Recovery searches at most 81 nearby columns
within the selected height; a fully blocked neighborhood fails explicitly.

User clarification added held mining to the original instant-building plan.
Hardness, tool effectiveness, airborne speed and matching client tags are grounded
in 26.3. Swords mine with their own timings; bedrock resists. Vanilla's 0.7 finish
threshold is retained. Premature finishes reconcile rather than implementing the
full delayed-destroy tick path. Crack updates use existing actions. Drops,
durability, harvesting, effects/enchantments and crafting remain deferred.

Dependency impact: world directly uses existing workspace flate2; the lockfile
adds that dependency edge, no new external package. No unsafe Rust, custom crypto
or JNI. The private client receiver needed a bounded server-packet policy for
large/high-ratio chunks; deployed server ingress remains 64 KiB/128x. This repairs
fixture coverage without weakening player ingress security.

## Short resource measurements

Preserved M3 baseline (`evidence/m4a-before.json`),
final imported-map runs (`evidence/m4a-after.json`),
import/COW profile (`evidence/m4a-resources.json`),
block latency histograms (`evidence/m4a-block-timing.json`).
CPU is server PID percent of **one core**; RSS is peak sampled **KiB**. Client/JVM
costs are excluded from these Rust-only cases. macOS arm64 release, one-second
all-ready warmup and about eight seconds active/case. No long load sweep or soak.

Pacing, radius-two circles, yaw, crouch/chat, encryption/compression and existing
replication schedule match M3. Imported coordinates are translated to the same
within-chunk offsets. A shared 7×7 COW platform at Y=199 controls the combat path
while the actual non-flat imported world remains loaded below. This isolates added
terrain lookup/collision cost from arbitrary map obstacles. Non-flat gameplay is
covered by user acceptance and focused tests; this synthetic case is not a claim
about every terrain layout or internet capacity.

| Scenario | M3 CPU → M4A CPU | M3 RSS → M4A RSS KiB | Position packets/s before → after |
|---|---:|---:|---:|
| 128 players / one world, movement | 29.56% → 30.76% | 26,992 → 27,488 | 305,976 → 303,364 |
| 256 players / 16 worlds, movement | 23.98% → 25.05% | 31,488 → 27,184 | 75,168 → 74,689 |
| 32 players / two worlds, combat | 4.60% → 6.22% | 14,816 → 19,760 | 9,248 → 8,976 |

The added movement cost is approximately 1.1–1.2 core percentage points (4–4.5%
relative); combat adds about 1.6 points (35% relative from a small baseline). It is a
measured cost of enabled geometry/state, not a claimed speedup. Combat output
varies with death/loading: baseline 15 deaths/respawns versus final 16; fixed input
pacing was retained. Final 536 attacks, 571 damage applications and all 32 clients
connected. Movement visibility edges remain 16,256 / 3,840; combat peak 480, end-of-active 450
during death/loading, and zero after cleanup.

| Server-side p99 upper bound | M3 → M4A 128/1 | M3 → M4A 256/16 | M3 → M4A combat |
|---|---:|---:|---:|
| Directory processing | 8 → 8 µs | 8 → 4 µs | 16 → 16 µs |
| Movement flush | 8.192 → 8.192 ms | 2.048 → 2.048 ms | 1.024 → 1.024 ms |
| Sampled publication → completed write | 65.536 → 65.536 ms | 65.536 → 65.536 ms | 65.536 → 65.536 ms |
| Action enqueue → processing | N/A | N/A | 4.096 → 8.192 ms |
| Critical event → completed write | N/A | N/A | 2.048 → 4.096 ms |
| Imported movement/collision processing | 4 µs | 8 µs | 32 µs |

Histograms are log2 microsecond upper bounds; flush includes awaits/yields. They
are server-side timing, not physical input, RTT or rendered latency. Existing
movement buffering is unchanged; combat writes gain no extra 50ms window.

Production idle after settling, three-second sample: M3 10,128 KiB; M4A flat 10,480
KiB; imported 16,992 KiB. All observed 0.00% CPU and one thread. This is below-resolution
short observation, not mathematically zero CPU. Loaded map/metadata overhead is
separate from empty instance overhead; no empty-world periodic work was added.

Final standalone profile:

- Import/decode/rebuild static skylight: **22.99 ms** for 16 chunks, 116,329 selected
  compressed input bytes; decoded policy charge **1,400,584 bytes**.
- Shared packet serialization: **29.89 ms**, **1,325,581 encoded bytes**; copying a
  chunk into scratch averaged **0.895 µs** over 1,600 copies. Live imported streams
  send shared mapped packets directly; compression/encryption cost is in load rows.
- 1/128/256 instance RSS: **16,272 / 16,352 / 16,352 KiB**, one thread, zero overlay
  charge before edits. Total observed increase 80 KiB, not a claim of zero bytes
  per clone; allocator/page granularity dominates this short measurement.
- 255 clone creations: p50 **0.083 µs**, p99 **3.0 µs**, max **4.75 µs**. Reset
  across 256 instances: p50 **5.833 µs**, p99 **8.167 µs**, max **41.375 µs**. These are
  in-process control timings, not client-ready spawn latency.
- 128 sparse edits across eight worlds: **121,216 logical retained block/light
  bytes**, RSS **16,704 KiB**. Eight asynchronous edit batches: p50 **10.009 ms**,
  max **10.259 ms**, including bounded light reconstruction/commit. Two threads
  immediately afterward include the shared blocking worker; it is not world-owned.
- 10,000 local collision queries: mean **0.07789 µs/query** in the isolated release
  microprobe. Real movement processing includes more validation/state work, as in
  the histogram row; the microprobe is not whole-player tick cost.
- Release encrypted placement/mining/reconnect test: three committed edits
  (including one trusted control edit), two client enqueue-to-write samples; all
  commit and enqueue-to-completed-write samples in the **≤16.384 ms** bucket. Small sample, no universal
  latency guarantee; rejection/ack/held-input behavior is additionally tested.
- Reset/destroy returned retained overlay charge to **zero**. RSS remained 16,704
  KiB while the template/service/allocator stayed alive; zero charge is not a claim
  of returning all OS RSS. All final load cases ended with zero connections,
  replicas, visibility edges and retained frame bytes, and zero protocol errors,
  timeouts, task failures, IO errors or slow viewers.

A diagnostic first load pass exposed retained per-session scratch: RSS 37,184 /
45,408 KiB for 128/256. Sending shared packets and freeing finished imported scratch
reduced those to 27,488 / 27,184 KiB in the final short run. That measured regression
justified a repeat; a further short repeat checked the recovery correction. Failed fixture setup and the earlier
buffer pass remain labeled diagnostic evidence, not final capacity claims.
Lightweight stack samples remain in raw evidence. Waits dominate wall samples;
network fanout, encryption/compression and active geometry/state are remaining
costs. Sparse stack samples do not establish exact function CPU percentages.

## Tests and corrected failures

Full workspace: **128 passing test executions / 122 distinct bodies**, with six
bridge bodies also compiled inside the private session fixture. Four opt-in tests
are ignored by default: three real-Java Play/PvP/terrain gates were explicitly run,
and the resource probe ran in short scenarios. Formatting, strict all-target
Clippy (`-D warnings`), release workspace examples/binary and Java 17-bytecode
`-Xlint:all -Werror` checks pass. Preserved SDK 0.1.0 and 0.2.0 integration passes.

Tests cover invalid NBT depth/lengths/duplicates, compression bombs/trailing input,
region overlap/symlinks/extents, palettes/negative section boundaries, static boxes/
slab step/floor/ceiling, held input and mining finish, wall correction/void recovery,
read-only rejection, multi-client blocks, COW light isolation, pending-reset stale
results, cleanup and all prior auth/network/world/PvP/bridge guarantees. Real Java
checks typed states/edits/template discovery/clone/reset/stale handles. Existing
slow/killed Java, grants/schema/backpressure and low-FD listener recovery pass.
Actual codec/mechanics gates are described above and linked in the evidence ledger.

Clean Linux arm64/musl checks and source correspondence are recorded in the Linux
manifest: all Rust/Java/manifests/runtime data match exactly; one reference-check
Python script had only its trailing blank line removed afterward, recorded in
`delivery_normalizations`. This is conformance/build/Java/failure validation, not Linux performance
or independent GUI acceptance. Final platform manifests identify exact sources and
binaries; manual acceptance retains its separate accepted build hash. Short-load
evidence retains its own exact probe/binary hashes: final changes afterward added
the join/reconnect safe-point check, required support for fallback recovery points,
updated CLI/status text and repaired test cleanup. These cold-path changes were
validated by regressions and Linux checks rather than another load campaign.

User feedback caught real defects: the first vertical crop removed lower ground;
void recovery spent ordinary travel credit; held input required a separate bounded
budget; predicted future contact could misclassify falling hits; initial mining
finish enums were wrong. Those intermediate builds were rejected. Full-height
conversion, acknowledged void teleport, confirmed-contact classification, correct
tags/tool timing and actual 0/1/2/3 enum checks resolved the exercised failures.
No client memory/auth/security setting was weakened to hide errors.

## Known limits and open questions

See the exact [terrain contract](M4A_TERRAIN_CONTRACT.md). Conversion can remove
unsupported features; strict import rejects them. No arbitrary dimensions/version
migration, block entities/containers, full lighting/neighbor rules, fluid/emission
simulation, general moving terrain/entity physics, complete anti-cheat, lag
compensation, mining effects/delayed-destroy parity, drops/durability/crafting,
armor/enchantments/mobs/full survival, persistence or plugin compatibility. Source
folders must not change while importing; local filesystem race-hardening against
a privileged same-user attacker is not promised. Multiple dynamic template files
are not exposed through Java. These remain explicit later decisions.

## Backward compatibility and rollback

Virtual stone retains its prior buried Air/Stone trusted-edit and flat PvP path.
Startup defaults preserve no-JVM operation. Imported building is explicit. New SDK
operations negotiate schema 1.2; older subsets stay compatible where representable.
The accepted 1.19.4-based PvP equations and precision remain, with terrain-aware
confirmed contact. Real stock 26.3 wire/auth remains the target.

To disable M4A terrain, restart without import flags; choose the prior stone
profile and `--pvp` option. For exact historical rollback, stop the relevant test/
deployment processes and create an isolated checkout:

```sh
git worktree add --detach ../SmolMC-pre-M4A m4a-before
```

Rebuild with the pinned toolchain/OpenSSL and a separate `CARGO_TARGET_DIR`; use
SDK 0.2.0 for that M3 server and rerun its checks. No persistence migration exists:
world edits, health/inventory and incarnation handles are ephemeral across restart.
The imported source save was never modified and needs no reverse conversion.
Preserve evidence before removing artifacts. The accepted manual build remains on
25567; final validation uses the separately hashed final build. User M3 on 25566
and the separate server on 25565 remain untouched by final fixture cleanup.

Final commit/tag `m4a-validated`; rollback `m4a-before`. **Stop after M4A.**
