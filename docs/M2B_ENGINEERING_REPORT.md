# M2B engineering report — shared templates and resource profiles

Date: **2026-10-29, America/Toronto**. Scope: M2B only, following explicit user
acceptance of M2A after user-reported real Minecraft 26.3 client verification.

**Outcome: M2B meets its documented acceptance gate.** Shared immutable template/
chunk data, sparse COW snapshots, clone/reset/reuse/park, validated world profiles/
budgets and resource accounting are implemented and measured. All **84 regression
tests**, formatting, Clippy and release/examples builds pass on macOS arm64 and
fresh Alpine Linux arm64. The opt-in active-resource test was explicitly run.
**Stop here: M2C, Java API/bridge, plugins, survival and later stages are not authorized.**

Before implementation, M2A acceptance was recorded in its report and AGENTS.md,
then the local Git repository/checkpoint was created: tag `m2a-accepted`, commit
`7a1ac2e3e46cbe17a3097065966f94a14339ab0b`. The
[user acceptance record](../evidence/m2a-stock-client-user-verification.json)
distinguishes the broad all-tests-passed assertion from specific two-account
world isolation/relog observations. No new M2B stock-client result is claimed.

## Goal → invariants → source evidence

Reduce empty-instance overhead and unnecessary copying while retaining bounded
resources, independent edits/players/ownership, shared registries/terrain and
explicit lifecycle cleanup. No per-world worker/thread/tick/timer; no mutable
shared base, global hot-path lock, new protocol layout or authentication shortcut.

The exact roadmap M2B gate remains: **tracked incremental RAM/timer work and
correctness under high churn; publish real measurements**. The design sources are
`docs/05_WORLD_INSTANCES.md`, `06_CHUNKS_AND_STREAMING.md`,
`07_FEATURES_AND_ZERO_IDLE.md`, `20_TEMPLATE_FORMAT_AND_LIFECYCLE.md`, resource/
security documents and ADR-005. [ADR-006](../decisions/ADR-006-m2b-shared-cow-overlays-and-budgets.md)
records trade-offs/dependencies; [protocol evidence](M2B_PROTOCOL_EVIDENCE.md)
separates inherited layouts from current fixture observations.

| M2B concept / gate | Actual implementation and acceptance evidence |
|---|---|
| Immutable chunks and shared templates | One existing Service stone body and registry set; Arc-owned immutable template descriptors shared across instances. Identity/content checks and unchanged-source hashes; no per-world base chunks or prewarming |
| Sparse COW mutable state | Empty overlay allocates nothing; immutable sorted chunk index and compact sparse chunk arrays. Clone shares root; writes copy index/touched chunks only. Pointer/content isolation and four-client wire tests |
| Clone/reset/reuse | Trusted `clone_world`, bounded batch edits, in-place reset with generation advance, destroy with retired ID. Occupied reset/destroy retain rejection policy; measured creation/reset/destruction distributions |
| Parked idle state | Explicit park and last-player leave; no world worker/timer. Ready instances also remain event-driven. Watch channels exist only while occupied; measured idle CPU/native threads through 4096 empty and 1000 edited worlds |
| World budgets | Validated per-world player/edit/chunk/logical-byte limits plus service-wide retained snapshot budget. Typed errors, transactional failure tests and an exact byte-limit release probe |
| Feature resolver | Supported editable/read-only profiles with fixed mandatory dependency closure before allocation; physics/mobs/persistence/Java requests explicitly unavailable. No constructors/workers/caches for those systems |
| Metrics | World state/profile/players/edit/chunk counts/logical charges/world-owned task/timer construction counts; global current/peak retained charges; independent RSS/CPU/thread sampling |
| Incremental RAM/timer work and high churn | Six fresh-process scenarios, 34,000 repeated create/edit/reset/destroy cycles, COW sharing/divergence/reset, concurrent lifecycle test and retained M1A–M2A regressions. All pass; measurements below |

## Design trade-offs → dependency impact

M2A copied the full sparse HashMap when publishing each edit and allocated a
terrain watch channel for every empty world. M2B publishes cheap immutable overlay
handles, indexes edits by chunk and allocates terrain watches only for occupied
worlds. New subscriptions replay only that chunk's overrides after shared base
bytes; ordinary movement never acquires the global control catalog mutex.

Cloning shares a snapshot and immutable template, while creating fresh identity,
profile/occupancy and eventual subscriptions. Source edits, clone edits, resets
and destruction cannot mutate siblings. Reset reuses WorldId with a new generation;
destroyed IDs never recycle. There is no allocator/world-object free-list pool.
Reset/drop costs depend on released overlay nodes, never a complete virtual world.

Batches cap at **4096 edits**, validate all positions before publication, apply
input-order last-write-wins semantics and remove overrides restored to the base.
A rejected transaction leaves current content, logical counts and retained-budget
ownership unchanged. COW copies are O(modified chunks + touched chunk edits),
not constant-time mutations. Sparse arrays remain sparse; no unmeasured dense
palette-promotion threshold or generalized imported-world cache is introduced.
Live viewer reconciliation still scans old/new sparse edits on terrain publication;
large edited scenes and slow viewers need further latency/soak profiling.

Limits: **4096 worlds**, **16 template descriptors**, **512 aggregate Play members**.
Default per-world ceilings: **512 players, 65,536 overrides, 4096 modified chunks,
1 MiB logical overlay charges**. The service-wide retained-overlay budget is
**64 MiB**, including obsolete snapshots held by readers/clones until their final
reference drops. Shared nodes charge once globally; each world's logical limit
counts its referenced contents. Reset/destroy can free overlays without allocating
a replacement snapshot. Global physical-accounting limit does not promise that
an edit succeeds at full capacity while older readers pin versions.

Charge accounting uses node `size_of`, a two-usize Arc-header policy charge and
boxed array lengths. It **excludes allocator rounding/metadata, bounded temporary
mutation buffers, global manager/route/watch metadata, sockets, registries, terrain,
TLS and runtime memory**. It is not allocator live bytes, RSS or an OS memory cap.
Per-world logical committed quotas and global retained snapshot quotas are separate
from bounded transient mutation work. RSS is measured independently below.

The default `editable` profile preserves accepted trusted buried Air/Stone edits;
`read-only` rejects that engine capability. Required protocol/world/chunk/player/
presence/chat features stay enabled. Unavailable capability requests fail before
world allocation. CLI adds `--world-profile editable|read-only`; richer per-world
limits are trusted Rust controls. Minecraft Feature Flags remain unchanged.
No runtime feature-toggle, plugin permission or configuration-file schema is promised.

No new third-party package, workspace crate, Cargo feature or lockfile dependency.
[Preservation evidence](../evidence/m2b-preservation.json) confirms auth, network,
protocol codecs, chunk encoder and registry/tag inventories are byte-identical to
accepted M2A. Shared body is still **35,956 bytes**, registry packets **33,513 bytes**,
and reusable Play scratch **35,965 bytes**. The benchmark has a fixed controller
thread; production gains no thread, task, callback or tick per world.

## Regression and adversarial results

| Check | macOS arm64 | Alpine arm64 |
|---|---|---|
| Complete workspace regression | 84 pass / 0 fail; one opt-in resource probe ignored | 84 pass / 0 fail; same opt-in probe ignored |
| Formatting / Clippy `-D warnings` | Pass | Pass |
| Release binary / workspace examples | Pass | Pass |
| Listener descriptor-pressure recovery | Pass | Pass |
| Explicit separate-process active probe | 1/32/128/256 players and five 32-player churn rounds pass | Not run as a Linux performance benchmark |

Groups: auth 5; network unit/TCP 11/11; protocol 17; chunks 3; sessions 23;
world 14. Evidence: [macOS tests](../evidence/m2b-tests.txt),
[Clippy](../evidence/m2b-clippy.txt), [fmt](../evidence/m2b-fmt.txt),
[release](../evidence/m2b-release-build.txt), [examples](../evidence/m2b-examples-build.txt),
[Linux tests](../evidence/m2b-linux-tests.txt), [Linux Clippy](../evidence/m2b-linux-clippy.txt),
[Linux builds](../evidence/m2b-linux-build.txt) and
[Linux metadata/source hashes](../evidence/m2b-linux-meta.json).

New tests cover template/root/unchanged-chunk sharing; clone divergence;
negative local/chunk coordinates; duplicate/reverted edits; atomic edit/chunk/
player/global-byte limit failure; disabled/unavailable features; stale reuse;
lazy terrain channels; retained snapshots; four-thread concurrent lifecycle churn;
and four TCP clients in two cloned worlds. The existing encrypted M2A switch and
M1D presence/chat/rejoin/spoof/slow-reader/coalescing/full-queue/cancelled-join
regressions remain. The 10,000-update ingress-coalescing test still processes one
latest snapshot. No changes to bounded auth, transport or packet codecs.

[Release cloned-world wire fixture](../evidence/m2b-cloned-wire.txt): all four clients
receive the initial shared correction; only A's clients receive its two-position
batch/reversion, B remains unchanged. Teardown returns active connections,
replicas/edges/frame leases and retained overlay charges to zero without protocol
errors/timeouts/task failures. These are synthetic **post-authentication** clients,
not real-account authentication or rendering capacity. Production cannot enable them.

[Standalone release byte-limit probe](../evidence/m2b-byte-budget-probe.json) sets
a world's limit to the measured one-edit logical charge (**132 bytes**), accepts
that exact boundary, rejects the next edit atomically, preserves current content/
charges and returns charges to zero after destroy. Its [source](../evidence/m2b-byte-budget-probe.rs)
and exact compiler command are preserved. It uses the same built release libraries.

The one ignored test remains the explicitly coordinated separate-process resource
probe: its server waits for SIGTERM and needs role/path/count/duration variables.
The active/churn scripts invoke `--ignored --exact`; this is not a hidden failure.

Linux validation used an ephemeral 2-vCPU/6-GiB Alpine 3.24.3 VM, Linux 6.18.58,
Rust 1.99.1, OpenSSL 3.5.9 and `RUSTFLAGS='-C target-feature=-crt-static'`. The initial
offline cache lacked a locked platform dependency; `cargo fetch --locked` supplied
it without changing Cargo.lock. The initial VM also lacked enabled loopback,
causing a network-test bind failure. The [failed environment run](../evidence/m2b-linux-initial-loopback-failure.txt)
is retained; after enabling loopback, **all checks were rerun successfully** with
matching source hashes. VM and local transfer helper were stopped before profiling.
This is fresh Linux build/conformance/fault validation, not Linux performance or
stock-client rendering evidence.

## Resource methodology and measured results

Host: Apple M5 Pro, 18 logical CPUs, macOS 27.0.1 arm64; Rust 1.99.1 and system
OpenSSL 4 linkage. [Environment/binary hashes](../evidence/m2b-validation-environment.json)
and [source inventory](../evidence/m2b-implementation-sha256.txt) pin the run.
[Raw world-resource evidence](../evidence/m2b-world-resources.json) links six fresh
release child processes controlled by `scripts/measure_m2b_worlds.py`.

Each child uses the real Service baseline but no listener or clients. Its async
executor stays live while one fixed helper thread blocks on stdin. Shared edit
inputs are prepared before the zero-world baseline. RSS includes the driver,
logging, latency-sample vectors and allocator high-water retention; it does not
isolate every object's allocator bytes. Per-operation latency samples time real
Service calls. Creation waves add to the previous tier; reset times cover the
whole current population. Logging is included: create/reset/destroy log lifecycle
events, whereas clone has no equivalent per-call log. Clone/create latency should
not be interpreted as a controlled algorithm-only speedup comparison.

Each phase has a three-second idle observation. CPU is process CPU-time delta /
wall time, **one core = 100%**, with `ps` 0.01s resolution. A zero delta means no
measurable CPU at this resolution, not mathematical zero. All world-profile phases
retain exactly **two fixed native threads**, with no growth versus world count;
world-owned task/timer construction counts remain zero. This omits listener/
connection timers, which are measured separately. Shared developer host, no core
affinity, repeated-run confidence interval or production latency guarantee.

### Empty parked worlds

| Empty parked worlds | RSS KiB | Increment over 0 KiB | Idle CPU % one core | Threads |
| --- | --- | --- | --- | --- |
| 0 | 9,680 | 0 | 0.00 | 2 |
| 1 | 9,696 | 16 | 0.00 | 2 |
| 10 | 9,696 | 16 | 0.00 | 2 |
| 100 | 9,744 | 64 | 0.00 | 2 |
| 1000 | 10,240 | 560 | 0.00 | 2 |
| 4096 | 11,504 | 1,824 | 0.00 | 2 |

At 4096 empty worlds incremental RSS is **1824 KiB (1.78 MiB)**; no overlay payload
is allocated. Historical accepted-M2A sampling on the same host measured 4096 KiB
incremental RSS, with a different driver/run. This is observed lower overhead,
not a statistically controlled percentage improvement or universal memory promise.

### Independent modified worlds

Each process uses 1, 100 or 5000 unique buried edits per world. Concentrated edits
stay in one chunk for 5000 edits; the spread workload uses 16 chunks. Targets rise
through 1/10/100/1000 independently edited worlds, not shared editable state.

| Independent modifications per world | 1 world ΔRSS KiB | 10 worlds ΔRSS KiB | 100 worlds ΔRSS KiB | 1000 worlds ΔRSS KiB | 1000-world retained charges, bytes |
| --- | --- | --- | --- | --- | --- |
| 1 edit | 16 | 32 | 112 | 720 | 132,000 |
| 100 edits | 16 | 96 | 176 | 1,280 | 528,000 |
| 5000 edits / one chunk | 288 | 656 | 3,648 | 33,040 | 20,128,000 |
| 5000 edits / 16 chunks | 288 | 592 | 3,680 | 34,432 | 21,088,000 |

All idle CPU deltas are 0.00s at sampling resolution. At 1000 worlds, actual RSS
increments for 5000 edits are about **32.27 MiB concentrated / 33.63 MiB spread**,
while deterministic retained charges are about 19.20 / 20.11 MiB. The difference
is not hidden: allocator rounding/retention, controller buffers and uncharged
metadata are included in RSS. No extrapolation to maximum edited-world counts.

### Creation/reset latency

| Scenario / creation wave | Create p50 μs | Create p95 μs | Reset p50 μs | Reset p95 μs |
| --- | --- | --- | --- | --- |
| Empty / final 3096 additions | 5.12 | 7.04 | 8.29 | 10.83 |
| 1 edit / final 900 additions | 4.08 | 6.33 | 7.71 | 30.75 |
| 100 edits / final 900 additions | 4.17 | 7.42 | 7.83 | 11.33 |
| 5000 one-chunk edits / final 900 additions | 4.67 | 8.08 | 6.83 | 10.92 |
| 5000 spread edits / final 900 additions | 5.08 | 13.38 | 8.75 | 15.83 |
| Clone of edited source / final 900 additions | 0.12 | 0.21 | 7.79 | 11.62 |

Raw JSON also includes sample counts, p99, maximum and total call time. These are
short local measurements, not first-login, network delivery or rendering latency.
The sparse COW design does not promise constant-time edits; spread/concentrated
batch edit timing is recorded alongside creation/reset/destruction.

### Clones and COW divergence

A fresh source receives 5000 edits; up to 1000 new worlds clone its snapshot.

| Phase | Total worlds (includes source) | Retained overlay charges, bytes | ΔRSS KiB |
| --- | --- | --- | --- |
| clones 1 | 2 | 20,128 | 256 |
| clones 10 | 11 | 20,128 | 256 |
| clones 100 | 101 | 20,128 | 304 |
| clones 1000 | 1001 | 20,128 | 576 |
| cow | 1001 | 20,144,128 | 22,160 |
| reset | 1001 | 20,128 | 22,160 |
| clear | 0 | 0 | 22,160 |

All 1000 clones plus their source share **20,128 charged bytes** until divergence.
Removing one override in each clone increases retained charges to **20,144,128
bytes**, proving private written chunks while the source stays at 5000 edits.
Resetting all clones releases their versions, leaving only the source's 20,128
bytes. Destroying everything returns charges to zero. RSS stays at its high-water
mark; released ownership does not require pages to return immediately to the OS.
Unit tests additionally pin old snapshots across reset/destroy and verify they
remain charged until their final holder drops.

### Repeated lifecycle operations and cleanup

| Lifecycle workload | Cycles | Batch elapsed ms | Final charges, bytes | Post-churn RSS KiB |
| --- | --- | --- | --- | --- |
| 0 edits / independent | 10000 | 173.72 | 0 | 11,680 |
| 1 edits / independent | 10000 | 181.85 | 0 | 11,488 |
| 100 edits / independent | 10000 | 201.29 | 0 | 12,080 |
| 5000 edits / independent | 2000 | 115.43 | 0 | 43,168 |
| 5000 edits / spread | 2000 | 242.23 | 0 | 44,416 |

These are **34,000** create/(optional edit)/reset/destroy cycles, up to 100 live
worlds per round. Every round asserts zero retained charges, not merely process
exit. The concurrent lifecycle regression adds 400 source/clone/reset/park/destroy
iterations across four threads. No long-soak or allocator-precise leak-freedom
claim; persistent RSS after cleanup is reported explicitly. All metadata/node
ownership is scoped; manager/routing high-water buffers are discarded when empty,
while process libraries/allocator/driver high-water state can remain resident.

### Active Play and previous performance protections

[Dense active evidence](../evidence/m2b-active-play.json) uses the existing separate
server/client release fixtures: 20Hz movement/yaw, 1Hz crouch, one sender every
two seconds, actual TCP/AES-CFB8/compression/chunk/batch/keepalive handlers and a
20-second observation after everyone spawns. All players share one world to retain
M1D's dense P*(P-1) fanout case; only the server PID is sampled.

| Play fixtures | Peak directed edges | Max sampled RSS KiB | CPU % one core | Threads | Result |
| --- | --- | --- | --- | --- | --- |
| 1 | 0 | 11,312 | 0.20 | 2 | PASS |
| 32 | 992 | 14,048 | 10.20 | 2 | PASS |
| 128 | 16256 | 25,776 | 63.72 | 2 | PASS |
| 256 | 65280 | 53,424 | 94.58 | 2 | PASS |

All requested concurrency/edges are reached, with zero lost clients, protocol/
timeout/task/I/O failures, and final connections/replicas/edges/frame leases zero.
Run-to-run CPU variation is visible: M2A measured 53.87% at 128 and 99.69% at 256,
versus **63.72% / 94.58%** here. This does not establish a speedup or statistically
bounded regression; pacing and intermediate-snapshot coalescing/delivery latency
are not quantified. Dense 256 remains near one-core capacity; 512 is an admission
safety bound, not a gameplay capacity promise.

[Five-round 32-player churn](../evidence/m2b-active-churn.json) also passes (160
fixture sessions). Post-cleanup RSS is **14,208 / 15,248 / 15,248 / 15,264 / 15,296
KiB**; total process CPU 1.853971s and OS peak 15,328 KiB. All final leases/counters
are zero. Authentication costs/capacity are not simulated.

[Production transport profile](../evidence/m2b-connection-resources.json): ready/
five-second idle RSS 9840 KiB, 512 partial sockets 15,024 KiB, final idle 15,328
KiB after blocking-worker retirement; OS peak 15,392 KiB. Total server CPU
0.121951s. All 1000 status reconnects pass: median 0.2905ms, p95 0.4282ms,
p99 0.4963ms. Startup 9.84ms; shutdown 2.75ms. Admission rates are explicitly raised
for the loopback generator; this is transport/auth-failure load, not successful
real-account authentication or active-world capacity.
[Listener recovery](../evidence/m2b-listener-recovery.json) and
[CLI/profile/status/shutdown](../evidence/m2b-cli-check.json) pass.

[Unchanged chunk encoder benchmark](../evidence/m2b-chunk-profile.json): 35,956-byte
shared body, 35,965-byte reusable scratch, 100,000 encodes average **323ns**;
1000 encode+fast-zlib operations average **10,818ns**, mean compressed body 348
bytes. Excludes encryption, sockets and player fanout. No compressed cache added.

## Backward compatibility → known limitations

Accepted M1A–M2A wire behavior, authenticated identities, default editable trusted
controls, read-only player block interactions, creative flight, scoped presence/
system chat, chunk pacing, keepalive and switching remain. No on-disk format or
saved-state migration. Internal WorldInstance/template and overlay representations
changed; rebuild Rust embedders and use template `.id` where needed. No stable
Rust binary layout, Java SDK or plugin compatibility is promised.

Only the existing virtual-stone template and buried Air/Stone bounds are supported.
No surface editing, lighting simulation, arbitrary template/Anvil import, general
compressed cache, dense palette promotion, async loading, physics, persistence,
plugins, Java or survival. Profiles are creation-time supported contracts, not a
general runtime-toggle framework. New real-account/stock-client M2B checks were
not run; M2A user acceptance and unchanged codec evidence are kept distinct.

Unverified: 8–24-hour soak; Windows/glibc/x86_64; maximum edited-world and slow-reader
snapshot retention combinations; allocator-precise memory/fragmentation; large
mutation delivery latency and fairness; many concurrent switches under slow writes;
real-account authentication capacity. Logical/retained charges are not hard RSS
caps, and no comparator-server performance superiority is claimed.

## Reproduction → rollback

```sh
scripts/cargo-local.sh fmt --all -- --check
scripts/cargo-local.sh test --workspace --locked
scripts/cargo-local.sh clippy --workspace --all-targets --locked -- -D warnings
scripts/cargo-local.sh build --release --locked
scripts/cargo-local.sh build --release --workspace --examples --locked
scripts/cargo-local.sh test --release -p smolmc-sessions --no-run --locked
python3 scripts/measure_m2b_worlds.py --seconds 3 --output evidence/m2b-world-resources.json
python3 scripts/measure_m1d_active.py --executable TEST_EXECUTABLE --players 1,32,128,256 --seconds 20 --output evidence/m2b-active-play.json
python3 scripts/measure_m1d_churn.py --executable TEST_EXECUTABLE --players 32 --rounds 5 --seconds 3 --output evidence/m2b-active-churn.json
python3 scripts/check_listener_recovery.py --output evidence/m2b-listener-recovery.json
python3 scripts/measure_m1b.py --output evidence/m2b-connection-resources.json
scripts/cargo-local.sh run --release --example profile -p smolmc-chunks --locked
target/release/smolmc --instances 2 --world-profile read-only --bind 127.0.0.1:25565
```

Select `TEST_EXECUTABLE` from the release no-run build; its exact current path/hash
is in environment evidence. Use fresh output names to preserve runs. Process
samplers use macOS units; Linux validation commands are the same Cargo/fault checks
with pinned Rust and the musl flags stated above. Source hashes in Linux metadata
match current host source. Linux performance was not measured.

Rollback was **not performed**. The accepted M2A checkpoint was created before any
M2B code change and remains available locally. To prepare a safe rollback:

1. Stop M2B with SIGTERM/Ctrl-C and confirm clean connection/session/lease shutdown.
2. Preserve this report/current authorization. Ephemeral edits are lost on restart;
   there is no persisted state to migrate or live-state rollback guarantee.
3. Create a separate checkout, leaving current work intact:
   `git worktree add --detach ../SmolMC-M2A-rollback m2a-accepted`.
4. In that checkout use pinned Rust 1.99.1 and native OpenSSL to run tests, fmt,
   Clippy, release, status and legitimate-client checks before replacing a binary.
   Use a normally installed toolchain or explicitly reuse this workspace's local
   toolchain with the new manifest; a fresh checkout does not include `.tools/`.
5. Remove the new `--world-profile` option and M2B-only embedding calls when using
   M2A; `--instances` remains supported. Preserve the current M2B authorization
   separately because checkpoint docs describe the earlier stage boundary.

The M2B implementation/report form a reviewable diff against `m2a-accepted`.
**M2B work stops here; M2C and later require new explicit user authorization.**
