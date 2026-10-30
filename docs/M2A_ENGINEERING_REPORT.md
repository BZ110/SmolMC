# M2A engineering report — explicit isolated instances

Date: **2026-10-29, America/Toronto**. The user explicitly authorized completing
M2A and superseded the outdated M1D-only boundary. M2B is now authorized; M2C and later remain unauthorized.

**Outcome: M2A accepted by the user after real stock Minecraft 26.3 verification.**
The user reports that all requested tests passed. They specifically confirmed two
accounts joining with `--instances 2`, one in World=1 and the other in World=2,
remaining separate, and successful relog/reassignment to World=1. This supplements
the existing 76-test and four-client automated evidence. See the
[user verification record](../evidence/m2a-stock-client-user-verification.json).
No new agent-captured trace, timing, screenshot or credential is implied. Detailed
live in-process switching steps were not separately supplied; the broader all-tests-
passed statement and explicit acceptance are user-reported evidence.

M2B is now explicitly authorized; M2C and later are not. The original M2A gate,
“two identical arenas diverge safely with edits,” retains the restricted buried
edit scope documented below.

## Goal → invariants

Complete explicit identities, lifecycle, world changes, subscriptions and isolation
while preserving M1D's immutable terrain sharing and bounded/event-driven networking.
Production sessions have exactly one lifecycle membership and replication lease;
worlds never share mutable edits, visibility indexes or chat membership. Monotonic
world IDs and reset generations prevent stale world work; independent replication
tokens guard update/chat/leave. No task, thread, tick or timer per world. Trusted
lifecycle/admin locks never cover socket I/O, encoding or ordinary movement.

Auth/network/protocol/chunk sources, Cargo.toml and Cargo.lock are **byte-identical
to final M1D**, checked in [preservation evidence](../evidence/m2a-m1d-preservation.json).
Shared terrain remains 35,956 bytes plus one reusable 35,965-byte scratch per Play
session. Dirty 50ms publish/flush, coalesced latest ingress, bounded batching,
reserved leave, frame/auth/admission budgets and connection-local ingress remain.

## Source evidence → acceptance audit

The roadmap was re-read in the mandatory context of architecture, world lifecycle,
validation and security documents. The table covers **every M2A concept and its
unchanged gate**; it does not substitute M2B's larger template/profile promises.
See [ADR-005](../decisions/ADR-005-m2a-explicit-instance-ownership.md) and
[protocol evidence](M2A_PROTOCOL_EVIDENCE.md) for policies versus observations.

| Roadmap concept / gate | Actual implementation | Verification / remaining limit |
|---|---|---|
| WorldId and TemplateId | Distinct Rust types; process-unique non-recycled worlds; immutable descriptor identified separately | Lifecycle/isolation tests; conflicting same-ID template declaration rejected |
| Load | Declare/bind the existing virtual-stone descriptor synchronously; create Ready instance | Unknown template rejected; actual Service bootstrap tested; dimension height matches the existing 384-block encoder. No file/Anvil import, arbitrary terrain or asynchronous loader |
| Reset | Reject if occupied; drop edits, advance generation, return fresh handle, Ready | 100-cycle model test, service weak-reference cleanup and 30,000 lifecycle benchmark cycles |
| Destroy | Reject if occupied; retire ID, remove runtime snapshots/routing and world directory on last leave | Occupied/error/stale paths, cancelled membership, wire teardown and repeated churn |
| World changes | Trusted per-player coalesced target; wait for existing ACKs, transfer membership, remove old peers/Tab/chunks, teleport/re-subscribe | Four-client encrypted TCP switch passes. Stock-client verification subsequently reported successful by user; see acceptance record |
| Viewer subscriptions | Sent-chunk/occupant indexes are owned by each world; fixed 25-slot session view rebuilt on switch | Restored pending-teleport/unsent/negative-coordinate tests; exact 25 unloads + target reload |
| No cross-instance replication | One shared directory task, separate per-world member/index sets; scoped chat and edits | Four concurrent clients in two worlds at identical coordinates; exact peer sets, chat senders and edits checked |
| Two identical arenas diverge safely with edits | Bounded local Air/Stone edits over the same immutable base; live correction/reversion and join/switch replay | Model verifies sibling/template unchanged; independent wire decoder verifies scoped states, including negative-coordinate replay. **Only buried edits supported** |

Production `--instances N` creates 1–4096 identical virtual arenas and assigns
successful joins round-robin over live worlds. Default remains one world.
Trusted in-process `Service::{create_world, edit_world, switch_player, reset_world,
destroy_world}` controls are real Rust methods, not illustrative Java signatures
or a stable SDK. There is no management socket, player command or auth bypass.

## Design trade-offs → dependency impact

The earlier partial M2A rewrite had restored movement coalescing but did not wire
world ownership into live sessions, switching or block delivery. The final change
adds a small lifecycle catalog/RAII membership, per-world immutable terrain watch
snapshots, switch orchestration and trusted routing. Four removed M1D tests were
restored: interest/negative coordinates/reference cleanup; teleport/sent-chunk
visibility; slow chat reader isolation; and per-sender unsigned chat admission.
The existing full-queue leave/duplicate-generation regression remains, augmented
by stale update/chat/leave coverage. Solo-world rosters correctly exclude self.

The pure manager binds an immutable template and owns sparse edits; session guards
pin occupied worlds and release counts on cancellation/drop. Reset and destroy
explicitly reject occupied worlds: first relocate or disconnect and await cleanup.
Reset-sensitive WorldHandle `{id, generation}` is required for mutations/reset/
destroy and switch requests. Pending switches do not reserve destinations; a
reset/destroy before commit fails the session safely. Acceptance of a switch
request is not acknowledgement of completion.

One event-driven directory holds per-world indexes. Empty indexes are removed
using the touched world's ID, not a scan of all worlds on movement. Update notices
carry only entity/world/token and read the shared latest slot. Chat now also
checks token, and a service-wide fanout allowance prevents multiplying M1D's
aggregate ceiling by world count. A conservative global allowance can drop chat
at overload; the bounded per-world sender/fanout policies still apply.

The cold control catalog serializes lifecycle/admin calls. Each edit clones the
bounded sparse map into a shared immutable watch snapshot; sessions compare
latest maps and replay overrides following shared chunk bytes. This avoids a
global world lock in movement/serialization but costs O(edits) admin copying,
per-subscription filtering and possible retention of old snapshots by readers.
These are explicit limitations; M2B overlay promotion/caches/resource profiles
are not implemented. World controls are trusted embedding operations, not a
permission boundary for hostile Rust callers.

Edits allow only Air/Stone at **-64 <= y < 63**, with existing +/-100,000 horizontal
bounds, capped at **65,536 distinct overrides/world**. Restoring base material
removes the entry; replacing an existing override at quota is valid. The intact
stone roof at y=63 keeps shared heightmaps and buried darkness valid. No surface
editing, arbitrary states, lighting simulation or client block placement/breaking.
Templates cap at 16, live worlds at 4096, Play memberships at 512. These are
safety limits, not measured capacity guarantees. Global routing-vector capacity
may remain allocated after destroy; instance objects and snapshots are dropped.

No new workspace crate, third-party dependency, Cargo feature or lockfile change;
no unsafe Rust, JVM, persistence, inventory, survival or M2B feature resolver.

## Measurable acceptance tests

[Full regression output](../evidence/m2a-tests.txt): **76 passed, 0 failed; one
resource probe ignored in ordinary tests and explicitly run below**. Final macOS
arm64 checks pass: [Clippy with warnings denied](../evidence/m2a-clippy.txt),
[formatting](../evidence/m2a-fmt.txt), [release binary](../evidence/m2a-release-build.txt),
[examples](../evidence/m2a-examples-build.txt) and
[release test executable](../evidence/m2a-probe-build.txt).

| Group | Passing tests |
|---|---:|
| Auth | 5 |
| Network unit / TCP | 11 / 11 |
| Protocol 777 | 17 |
| Chunks | 3 |
| Sessions, including lifecycle and headless integration | 20 |
| World | 9 |

Coverage includes legacy malformed/bounded input, local/shared ingress isolation,
crypto/Configuration, chunks/light/heightmaps, actual keepalive, duplicate identity,
full-queue reserved cleanup, cancelled joins, stale generations, 10,000 updates
collapsed to one latest snapshot, slow readers and M1D wire chat/pose/rejoin/spoof
isolation. New world tests cover sibling/template isolation, quota overwrite,
reset generations, conflicting templates, monotonic IDs and destruction.

The four-client test exercises **real TCP + AES-CFB8 + compression + actual Play
handlers**, with synthetic post-authentication identities. A/C initially see only
each other and B/D only each other. An A edit/reversion reaches A/C only; a B
negative-coordinate edit is replayed after A switches to B. A receives two unique
teleports, 25 unloads and 50 total chunks; A then sees B/D, C nobody, B sees A/D.
Occupied reset/destroy are rejected. Closing all clients permits reset/destroy;
stale handles fail, and active connections/replicas/edges/frame leases return to
zero without protocol errors/timeouts/task failures. The same switch fixture also passes in the [release test executable](../evidence/m2a-four-client-wire.txt). Separate tests abort a
membership task and prove release, and use Weak references to prove snapshots
are freed on empty-world reset/destroy. This is stronger than inferring cleanup
from RSS alone, but not proof of every allocation or long-term leak freedom.

### Why the one test is ignored

`m1d_probe::active_play_resource_probe` requires an explicit server/client role,
ready/report paths, player count and observation duration. The server deliberately
waits for SIGTERM, while a separate client process generates load. Running it as
an ordinary test without those variables would fail or wait without a coordinating
client. `#[ignore = "explicit separate-process resource workload"]` isolates this
opt-in benchmark; it does **not** hide a known failing assertion. The supplied
measurement scripts set up both processes and invoke `--ignored --exact`.

It was explicitly run for **1/32/128/256 players for 20 seconds after spawn**, and
five further rounds of 32 players for three seconds against one retained server.
Every run passes. Evidence: [active load](../evidence/m2a-active-play.json) and
[active churn](../evidence/m2a-active-churn.json), with raw logs linked by each JSON.
No synthetic fixture sends Mojang/session requests or measures real-account capacity.

## Resource profile: measured results, not targets

Host: Apple M5 Pro, 18 logical CPUs, macOS 27.0.1 arm64, Rust/Cargo 1.99.1,
OpenSSL 4 linkage. [Environment/binary hashes](../evidence/m2a-validation-environment.json)
and [source hashes](../evidence/m2a-implementation-sha256.txt) pin this run.
CPU is percent of one core from process CPU-time delta / wall time; `ps` resolution
is 0.01s. RSS is process resident memory, not allocator live bytes. Shared host;
no core affinity, statistical confidence intervals or latency guarantee.

### Increasing empty instances and lifecycle churn

The [world-resource sampler](../evidence/m2a-world-resources.json) controls a separate
release example using the real Service baseline (auth client/registries/shared
stone included), without a listener, clients or replication directory. It measures
0/1/10/100/1000/4096 worlds with three-second idle windows, destroys them, then
runs three 10,000-world create/edit/reset/destroy batches in the same process.
The final run keeps the async executor active while one fixed helper thread waits
for controller stdin. Earlier runs blocked the executor and therefore cannot
substantiate idle executor CPU behavior; their ownership assertions passed, but
those CPU results are superseded. Both [initial evidence](../evidence/m2a-world-resources-initial.json)
and [the second blocked-executor run](../evidence/m2a-world-resources-blocked-executor.json)
are retained. The corrected run follows all regression and active/transport work;
only its measurements appear below.

| Empty instances | RSS KiB | Increment over 0 KiB | Idle CPU % one core | Threads |
|---:|---:|---:|---:|---:|
| 0 | 9,648 | 0 | 0.00 | 2 |
| 1 | 9,648 | 0 | 0.00 | 2 |
| 10 | 9,696 | 48 | 0.00 | 2 |
| 100 | 9,760 | 112 | 0.00 | 2 |
| 1,000 | 10,768 | 1,120 | 0.00 | 2 |
| 4,096 | 13,744 | 4,096 | 0.00 | 2 |

At 4,096 empty instances, measured incremental RSS is **4,096 KiB (4.00 MiB)**.
Destroying all 4,096 took **53.66ms**, including lifecycle logging. Creation of
the additional 3,096 worlds from 1,000 to 4,096 took **21.68ms**. These are
batch elapsed times, not per-world latency percentiles. The three 10,000-world
create/edit/reset/destroy batches took **191.54 / 195.37 / 256.40ms** including
logging and stale-handle assertions. Post-churn RSS was **13,936 KiB** after each
batch; OS peak **13,952 KiB**. Total process CPU including setup/control/cleanup
was **0.721883s**. Idle CPU delta was 0.00s in each three-second observation;
the two fixed benchmark threads did not increase with instance count.

No per-world timer/task/thread is constructed; the model contains plain state and
watch senders. The short idle CPU deltas support “no measurable incremental idle
CPU at this sampling resolution,” **not mathematically zero CPU**. The example has one fixed controller helper thread plus the live async executor;
it does not measure a listening server's shared reactor timers. The production idle profile below covers the listener separately.
RSS after destroy stays above cold startup; allocator/global capacity retention is
not equivalent to live world references. No allocator live-byte analysis or
8–24-hour soak was run.

### M1D active-load preservation

The same dense workload as M1D uses 20Hz circular movement/yaw, 1Hz crouch, one
chat sender every two seconds, 25 chunks/player, AES-CFB8/compression and real
batch/keepalive handlers. Client and server are separate release test processes;
only the server PID is sampled. All players share one instance here, intentionally
preserving the worst-case P*(P-1) directed fanout comparison.

| Play fixtures | Directed edges | Max sampled RSS KiB | CPU % one core | Threads | Result |
|---:|---:|---:|---:|---:|---|
| 1 | 0 | 11,136 | 0.25 | 2 | Pass |
| 32 | 992 | 13,952 | 10.87 | 2 | Pass |
| 128 | 16,256 | 25,648 | 53.87 | 2 | Pass |
| 256 | 65,280 | 53,440 | **99.69** | 2 | Pass; near saturation |

All reach requested concurrency/edges, have zero closed clients/protocol errors/
timeouts/task failures/I/O errors, and finish with zero connection/replica/edge/
frame leases. Historical M1D at 256 was 99.19% CPU/52,576 KiB; these short shared-host
observations are consistent with the existing saturation limit, not a statistically
proven performance improvement or 512-player promise. Delivery latency and
intermediate-snapshot loss/coalescing are not quantified.

Five-round active churn passes (160 fixture sessions). Post-cleanup RSS:
**14,080 / 14,960 / 15,072 / 15,088 / 15,664 KiB**. Total server CPU 2.139822s;
OS peak RSS 15,696 KiB. Retained RSS growth is reported explicitly; no long-term
leak-free claim. All rounds have zero lost clients and final leases/counters zero.

### Transport/failure and chunk regression profiles

[Production connection profile](../evidence/m2a-connection-resources.json): idle
10,016 KiB, 512 partial sockets 15,216 KiB, final worker-retired idle 15,520 KiB;
OS peak 15,568 KiB; total server CPU 0.132612s. All **1000 status exchanges** pass;
median 0.3105ms, p95 0.4543ms, p99 0.6051ms. Startup 11.69ms; shutdown 2.87ms.
Admission is explicitly raised for the loopback generator; this measures pending
connections/auth failures, not successful authentication or active players.
[Production two-instance CLI/status checks](../evidence/m2a-cli-check.json) pass, including invalid count rejection and shutdown.
[Descriptor-pressure recovery](../evidence/m2a-listener-recovery.json) passes:
established peer survives, fresh peer succeeds afterward, final cleanup succeeds.

[Chunk microbenchmark](../evidence/m2a-chunk-profile.json): shared body 35,956 bytes;
reused packet/scratch 35,965 bytes; 100,000 encodes average **385ns**, 1000
encode+fast-zlib iterations average **11,271ns**, mean compressed body 348 bytes.
This unchanged encoder benchmark excludes network, encryption and player fanout.
These are observations from one run, not service-level latency promises.

## Backward compatibility and known limitations

Default one-world status/auth/Configuration/stone/creative flight/presence/system
relay remain. Multiple worlds intentionally scope chat/Tab/presence. No on-disk
format or data migration; everything editable is ephemeral. Rust controls are
internal, with no stable plugin or Java API promise. M1D unsupported controls,
known-pack requirement and empty tag memberships remain as documented.

**Client acceptance resolved:** the user reports successful stock 26.3 verification
and explicitly accepts M2A. Their specific two-world join/isolation/relog observations
are recorded above; the fixture remains the independently captured switch evidence.

Also untested: current M2A Linux/Windows builds; long soak; maximum edited-world
and retained-snapshot memory; many simultaneous switches under slow writes;
full legal-control/latency matrix; arbitrary/surface edits and template import.
Historical M1D Alpine results do not establish new M2A Linux conformance. No
comparison with Java servers or claim of vanilla gameplay parity was made.

## Reproduction and rollback

```sh
scripts/cargo-local.sh fmt --all -- --check
scripts/cargo-local.sh test --workspace --locked
scripts/cargo-local.sh clippy --workspace --all-targets --locked -- -D warnings
scripts/cargo-local.sh build --release --locked
scripts/cargo-local.sh build --release --workspace --examples --locked
scripts/cargo-local.sh test --release -p smolmc-sessions --no-run --locked
python3 scripts/measure_m2a_worlds.py --output evidence/m2a-world-resources.json
python3 scripts/measure_m1d_active.py --executable TEST_EXECUTABLE --players 1,32,128,256 --seconds 20 --output evidence/m2a-active-play.json
python3 scripts/measure_m1d_churn.py --executable TEST_EXECUTABLE --players 32 --rounds 5 --seconds 3 --output evidence/m2a-active-churn.json
python3 scripts/check_listener_recovery.py --output evidence/m2a-listener-recovery.json
python3 scripts/measure_m1b.py --output evidence/m2a-connection-resources.json
scripts/cargo-local.sh run --release --example profile -p smolmc-chunks --locked
target/release/smolmc --instances 2 --bind 127.0.0.1:25565
```

`TEST_EXECUTABLE` is the sessions library test artifact printed by the release
no-run build (current path/hash in environment evidence). Choose new output paths
to preserve prior measurements. Scripts use macOS `ps`/`wait4` units. The production
binary cannot run synthetic fixtures. Two real accounts joining the last command
should land in separate worlds; switching uses the trusted in-process Service API,
not a new unauthenticated operator endpoint or player command.

Rollback **was not performed**. The following archive instructions are historical;
a local Git checkpoint of accepted M2A is now being created before M2B. The local
`.tools/m2a-precheckpoint-m1d.tar.gz` archive SHA-256 is
`70d9da2a48f4a0ed7bf0b7a077f8ec4280e8419dade2ced64c9ea0527479a7f0`.
[Archive verification](../evidence/m2a-rollback-verification.json) confirms **every
file** in the final M1D implementation inventory matches archive contents.
The pre-task partial-M2A input is separately preserved at
`.tools/m2a-input-20261029.tar.gz`, SHA-256
`544cccfb1f84b2e66b6cfd878aedc8260590020f03eb403c4c363a38f40a2883`;
it is a recovery snapshot, not the accepted M1D baseline.

1. Stop M2A with SIGTERM/Ctrl-C and confirm no active connections/tasks/leases.
2. Preserve current reports/evidence and authorization; ephemeral edits have no
   persistence and will be lost.
3. Extract the verified M1D archive into a **new empty directory**, never over a
   running/current checkout. Build with pinned Rust and compatible native OpenSSL.
4. Run its regression, fmt, Clippy, release and status/legitimate-login/stone/
   flight/two-player chat checks before replacement. Its archived authorization
   text is historical; retain the current M2A authority separately.
5. Use one-world M1D configuration; remove `--instances` from launch arguments.
   No save/SDK migration exists. Returning to this M2A candidate likewise requires
   a fresh process; no live-state rollback is promised.

## Open questions and stop boundary

M2A client acceptance is resolved by the user report linked above. General surface
editing, arbitrary imported templates, long soak and current M2A Linux/Windows
validation remain untested. M2B alone is now explicitly authorized for shared
immutable templates, COW overlays, resource profiles/metrics and measured lifecycle
performance. M2C and later remain unauthorized; stop after M2B.
