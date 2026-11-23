# Revised M3 — PvP Gameplay Foundation

Original capture filenames refer to the [local validation archive](../evidence/README.md).

2026-10-30. Final checkpoint: `m3-pvp-validated`. Rollback checkpoint:
`m3-pvp-before` → `08fef9a` (`m2c-validated`). Revised M3 is implemented and
user-accepted. The original plugin-ecosystem M3, broad M4 and M5 remain deferred.

## Goal and acceptance

Two authenticated, unmodified Java 26.3 clients can equip swords, see swings,
fight, lose health, receive knockback, die, respawn and continue. The user verified
the real clients at `127.0.0.1:25566`; the final consecutive-hit/landing check
received **“IT'S SO GOOOOD ty”**. This is user-reported acceptance, not independent
agent observation of the game windows. [Acceptance evidence](../evidence/m3-vanilla-client-acceptance.json)
records earlier defects, subsequent responses and binary hashes without inventing
individual step timings.

The user explicitly refined the gameplay target to **1.19.4/Paper-Spigot-style
knockback**, prohibiting Purpur. Protocol remains exactly **777 / 26.3**. The
accepted profile includes an explicit flat-arena landing consistency policy; it
is not bitwise Paper simulation or a claim about every competitive server.

## Invariants and implemented behavior

Rust owns health, damage, inventory and combat decisions. Existing online-mode
authentication, AES-CFB8, compression, packet/admission budgets, world generations,
shared immutable terrain/COW overlays and optimized spatial replication remain.
There is no new per-player/world task, thread, timer or idle scanning loop.

| Area | Implemented behavior |
|---|---|
| Setup | Startup `--pvp on\|off` (default off), `--arena-spawn X,Y,Z`; PvP arena uses adventure mode and read-only stone roof |
| Kit/inventory | 46 authoritative slots; iron sword in first hotbar slot, spare diamond sword in main inventory; selected slot, pickup/shift/hotbar/F swaps, main/off-hand equipment |
| Validation | Bounded exact decoders, authoritative revisions, client predictions discarded; stale/unsupported inventory operations resync; identity, readiness, generation, same-world, alive and reach checks |
| Combat | Visible air swings; bare/iron/diamond damage; charge/attack-speed indicator; weak/strong/critical/sweep/sprint-hit classification, appropriate attack sounds to attacker and scoped observers |
| Consequences | Health/metadata, hurt feedback, invulnerability differences, primary/secondary sweep damage, combined knockback, sprint cancellation, critical/sweep particles |
| Lifecycle | Death interface/animation; one-second corpse visibility; vanilla respawn request restores health/kit/spawn/chunks/visibility; isolation and reconnect |
| Optional Java | SDK 0.2.0/schema 1.1 health/equipment snapshot and noncancellable damage/death/respawn observations; existing world/command controls remain |

Default PvP-off retains the earlier creative/flight behavior. Configuration is
startup-scoped, not a live toggle. Dropping an item resynchronizes the inventory;
there is no item entity or item loss. Respawn always restores the deterministic kit.

## Source evidence and decisions

[Protocol/gameplay evidence](M3_PROTOCOL_EVIDENCE.md),
[ADR-009](../decisions/ADR-009-m3-authoritative-arena-pvp.md) and
[security review](M3_SECURITY_REVIEW.md) distinguish reference facts from local
policies and limits. Private source inventories are hashes/paths only; no Mojang
implementation bodies, JARs or assets are distributed.

- **Observed wire facts:** actual installed 26.3 codecs independently decode,
  consume exactly and re-encode 17 outbound vectors across 16 packet classes.
  The actual inbound F enum, registry item/attribute/particle IDs and semantic LP
  velocity are checked. Codec result (`evidence/m3-reference-codecs.json`).
- **Observed historical arithmetic:** actual Paper 1.19.4 build 550 classes match
  16 yaw and six velocity quantization vectors. Official download hashes and
  pinned Paper/CraftBukkit source/patch inventories ground the inspected base/
  sprint equations, motion restoration and sprint cancellation.
  Classic result (`evidence/m3-classic1194-reference.json`),
  source inventory (`evidence/m3-classic1194-sources.json`). Patch-only mode was
  used; no Paper server/plugin host was launched. No Purpur was used.
- **Design policy:** confirmed roof contact settles stale downward motion to the
  ordinary resting gravity/drag vector before a same-phase hit. The rejected
  trace had old Y `-0.3015347`, yielding impulse Y `0.2492326`; settled contact
  yields `0.3608000`, then classic packet precision `0.36075`. Airborne Y remains
  unchanged. This is explicit timing/contact policy, not hidden vanilla parity.
- **Precision:** historical float-indexed yaw arithmetic is computed on demand
  without a retained 256 KiB sine table. Clamp/truncate velocity at classic
  1/8000 precision, then encode the mandatory modern LP vector. Hurt yaw uses
  standard atan2; coincident-vector RNG is bounded with a finite fallback.

Early self-contained fixtures missed an off-by-one clientbound registration
assumption and M2C's incorrect Commands ID `0x0f` (correct `0x10`). This caused a
real client NBT/Java-heap failure. F action 7 and actual item IDs 1045/1050 also
required correction. The independent codec gate now prevents those mirrored-
decoder mistakes; the M2C report retains a visible erratum. This was not resolved
by changing client memory settings.

The historical 26.3 primitive motion adapter initially omitted accepted-position
restoration. Later paired primitive checks reproduced the rejected landing dip,
but did not instantiate the full ServerPlayer/network/render path. They were
insufficient to establish gameplay feel. Marked failure evidence (`evidence/m3-knockback-diagnostic.json`)
and [diagnostic scope](../reference-tests/diagnostics/modern-26-3/README.md)
remain available; they are not current classic-profile acceptance gates.

## Ownership, latency and trade-offs

The existing shared directory serializes validated actions into instance-local
HashMap membership. Source and target use fresh accepted movement snapshots,
independent of 50ms publication. Sweep candidates come from chunk occupancy and
exact geometry, without scanning unrelated worlds. Immutable packet payloads are
shared across recipients; compression/encryption/writes remain connection-local.

Combat-critical transitions use a bounded 32-message queue per enabled session,
promptly flushed by its existing handler. Entity-addressed writes establish the
current roster first. No extra 50ms combat buffering is added. Slow readers close
rather than silently losing health/death transitions. Inbound gameplay budget is
32 actions/s with burst eight; directory ingress stays bounded at 2048.

A final synchronized sweep case exposed queue saturation: 32 directory commands
could run before writers drained, closing one group of 16 clients. The repair ends
a directory batch after a gameplay action and yields the existing executor.
Movement retains 32-command/1ms batching; capacities and fail-closed backpressure
are unchanged. The failed run (`evidence/m3-combat-queue-failure.json`),
targeted correction check (`evidence/m3-combat-fairness-check.json`) and final
three-case run are retained. Failed reduced-load CPU is not a valid improvement.
Numeric diagnostic tracing releases player-state locks before stderr IO.

Cooldown, invulnerability, motion catch-up and corpse deadline use existing event/
select processing. Flat-stone motion catch-up is bounded to 256 scalar steps; the
accepted position is restored after simulated steps while retaining velocity and
collision state. This is a narrow arena model, not full collision/anti-cheat.

Optional Java notifications use a short sequence/publication mutex so concurrent
trusted Rust publishers cannot reorder events. It covers only numbering and bounded
broadcast, never world/player mutation, IO, await or Java. Movement and Rust-only
combat do not acquire it. Java cannot veto a hit or block its calculation.

Dependency impact: sessions directly uses existing workspace OpenSSL for bounded
provider randomness; no new package/lockfile dependency, custom cryptography,
unsafe Rust or JNI. SDK 0.2.0 negotiates schema 1.1; old SDK 0.1.0/schema 1.0 remains
a compatible subset, tested with its preserved JAR. See the [bridge contract](M2C_BRIDGE_CONTRACT.md).

## Short measured resource profile

Before (`evidence/m3-before.json`) is the preserved accepted M2C release fixture;
after (`evidence/m3-after.json`) records final production/probe hashes and raw
logs/histograms/profiles. Both use one-second all-ready warmup and about eight
seconds of active measurement. Separate server/client PIDs run encrypted,
compressed post-authentication Play on local macOS arm64. CPU is percent of **one
core** for the server PID; RSS is peak sampled KiB. No JVM in these load cases.

| Scenario | Before CPU | After CPU | Before RSS KiB | After RSS KiB | Position packets/s before → after |
|---|---:|---:|---:|---:|---:|
| 128 players / one world, movement, PvP off | 32.55% | 29.33% | 26,576 | 27,024 | 306,143 → 307,134 |
| 256 players / 16 worlds, movement, PvP off | 26.42% | 23.34% | 30,784 | 31,120 | 75,169 → 75,041 |
| 32 players / two worlds, grounded combat | N/A | 5.24% | N/A | 14,560 | N/A → 9,215 |

M2C has no combat; a comparable combat-before value does not exist. Combat clients
attack every 13 client ticks, use grounded circles to exercise sweep consequences
and automatically request respawn. Death/loading changes movement and edges;
combat is not directly comparable with the movement rows. Final whole-run counters
were 535 attack actions, 568 primary/secondary damage applications and 16 deaths/
respawns. All 32 clients remained connected; peak and end active combat edges were
480. Movement edges were 16,256 and 3,840, with unchanged 20Hz inputs/yaw/pose/chat.
No deliberate reduction in movement publication pacing or delivery correctness.

| Server-side p99 upper histogram bound | Before 128/1 → after | Before 256/16 → after | Combat after |
|---|---:|---:|---:|
| Directory processing | 8 → 8 µs | 4 → 8 µs | 16 µs |
| Movement flush (includes awaits/yields) | 8.192 → 8.192 ms | 2.048 → 2.048 ms | 1.024 ms |
| Sampled publication → completed write | 65.536 → 65.536 ms | 65.536 → 65.536 ms | 65.536 ms |
| Action enqueue → processing | N/A | N/A | 8.192 ms |
| Critical event enqueue → completed write | N/A | N/A | 2.048 ms |

Log2 histogram bounds are not exact quantiles. Action timing starts after network
validation/enqueue; write timing is server-side, not physical input, RTT, rendering
or display latency. Movement retains its prior 50ms coalescing schedule. Critical
writes are independent. Test-only instrumentation is absent from production paths.

Production idle after 2.5s settling, sampled for three seconds: baseline RSS 10,032
KiB / 0.00% CPU / one thread; final PvP-off 10,128 KiB and PvP-on 10,144 KiB, both
0.00% observed CPU and one thread. This short below-resolution observation is not
proof of mathematically zero CPU. Empty instance scheduling/storage architecture
is unchanged from M2B; no new idle-world loop or copied template was introduced.

All final cases ended with zero active connections, active replicas, visibility
edges and retained frame bytes; zero protocol errors, timeouts, task failures,
IO errors and slow viewers. Listener and bridge fault checks separately exercise
failure cleanup. Fixtures terminated their processes; the user's manual arena is
intentionally separate from fixture cleanup accounting.

Lightweight one-second/10ms stack samples are in the raw evidence. Waits dominate
wall samples; the dense movement sample includes OpenSSL CFB8 encryption. Sparse
samples do not establish a reliable per-function CPU percentage. Remaining active
costs include unavoidable recipient fanout, per-client encryption/compression,
serialization and outbound IO. The existing replication optimizations are retained;
this milestone does not claim a newly proved algorithmic CPU speedup. Lower CPU in
these short movement runs may include ordinary host/timing variance. This is
regression evidence, not internet capacity, long-soak or peak-load certification.

## Measurable acceptance and regression evidence

The final source/hash manifest and raw platform logs are recorded under
macOS manifest (`evidence/m3-macos-meta.json`) and
Linux manifest (`evidence/m3-linux-meta.json`), with adjacent raw logs. Saved text logs normalize trailing blank lines only.

- Full workspace Rust tests: **117 passing test executions**, 111 distinct bodies
  (six bridge bodies also compile inside the private sessions fixture), three
  ignored opt-in tests. The two ignored real-Java Play/PvP tests were separately
  invoked and passed; the ignored resource probe ran in the short scenarios.
- Formatting, strict Clippy across workspace/all targets (`-D warnings`), release
  workspace examples and production release binary passed.
- Java strict `-Xlint:all -Werror`, Java 17 bytecode SDK/integration, real encrypted
  Play commands and PvP observation passed. Preserved SDK 0.1.0 integration passed.
- Decoder/adversarial inventory bounds, selection/F swaps, charge/iframes/reach,
  crit/sweep/sprint cancellation, captured landing vector and on-wire grounded
  knockback tests pass. Encrypted lifecycle tests cover damage/death/respawn,
  world isolation, reconnect and sweep effects. Existing auth/network/COW/lifecycle/
  movement/bridge regressions remain in the full suite.
- Bridge credentials/grants/schema/trailing/oversized/partial input, nonreading
  peer, slow/killed JVM, saturation/backpressure and registration cleanup pass.
  Low-file-descriptor listener fault recovery preserves existing connections and
  resumes status service. Bridge faults (`evidence/m3-bridge-faults.json`),
  listener recovery (`evidence/m3-listener-recovery.json`).
- Independent official 26.3 wire and actual Paper 1.19.4 arithmetic checks pass.
- Fresh Linux arm64/musl passed all ten recorded checks: fmt, workspace tests,
  strict Clippy, release examples/binary, Java integration, both real-Java Play/PvP
  tests, bridge faults and listener recovery. All 72 transferred source hashes
  match the final tree. Alpine 3.24.3 / kernel 6.18.58, Rust 1.99.1, OpenSSL 3.5.9,
  OpenJDK 25.0.4; dynamically linked musl OpenSSL (`-crt-static` disabled), debug
  info and incremental builds disabled for the bounded VM. This is conformance/
  build/Java/failure validation, not Linux performance or GUI acceptance. No Linux
  load sweep or long soak was required.

The user accepted gameplay before the final directory-fairness and trace-lock
repairs; accepted combat equations were unchanged afterward. Acceptance evidence
keeps the live accepted binary hash separate from the final tested release hash.
The intentionally running arena on 25566 uses that accepted build; automated
measurements use the final build. The user's separate 25565 server and game clients
were left alone.

## Known limits and open questions

Flat read-only arena only. No armor, shields, enchantments, durability, drops,
crafting, containers, hunger/regen, fall/environment damage, mobs, general block
physics, fluids, persistence, teams or broad survival. No complete movement
anti-cheat, lag compensation, full historical tick simulation or arbitrary terrain
support. Inventory interactions outside the supported subset resynchronize.
World reset/destroy still reject occupied worlds. Java adds observations, not
Bukkit events/plugins/compatibility. Full Paper behavior, custom competitive
plugins/settings, hostile internet gameplay and long-running load remain unverified.
These are later decisions requiring authorization, not unfinished hidden M3 claims.

## Rollback and stopping point

1. For the current build without gameplay, restart with `--pvp off`. This retains
   corrected wire IDs and optional bridge compatibility; it is not a runtime toggle.
2. For the full historical checkpoint, stop the relevant server/sidecar and create
   an isolated worktree with `git worktree add ../SmolMC-m3-rollback m3-pvp-before`.
   Rebuild there with a separate `CARGO_TARGET_DIR`; use SDK 0.1.0 for that server
   and rerun the M2C checks. Do not mix new artifacts with the old process.
3. The original M2C checkpoint contains the Commands `0x0f` defect discovered here.
   Backport the `0x10` correction for stock-client command advertisement, or keep
   its optional bridge off. The historical tag is preserved unchanged.
4. Worlds/health/inventory are ephemeral; restarting loses in-memory state and
   invalidates incarnation-bound handles. There is no persistence migration.
   Preserve evidence before removing worktrees/artifacts.

Final commit/tag `m3-pvp-validated` preserves this result; `m3-pvp-before` preserves
the original baseline. **Stop after revised M3. No next milestone is authorized.**
