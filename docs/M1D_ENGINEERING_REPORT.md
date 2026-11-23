# M1D engineering report — player presence and system messaging

Original capture filenames refer to the [local validation archive](../evidence/README.md).

Date: 2026-10-29. Authorization: **M1D only**, after provisional M1C acceptance
and review. The roadmap gate passed with two legitimate stock clients. **Stop
here: M2A, Java integration, survival and all subsequent milestones remain
unauthorized.** No mutable terrain, world instance manager or simulation worker
was added.

## Outcome and acceptance

Two unmodified Java Edition **26.3 / protocol 777** clients, signed into distinct
legitimate accounts, saw each other walk/fly, turn and crouch, exchanged readable
`[System relay] <name> ...` messages, and removed/restored presence in the world
and Tab list on disconnect/reconnect. They stayed connected for the requested
minute. Final user observation: **“Done, both work well.”** Visual confirmation
is user-reported; no screenshot or client credentials were collected.

The final client record (`evidence/m1d-vanilla-client-complete.json`) and
server trace (`evidence/m1d-client-complete-server.log`) corroborate four
legitimate authentications/Configuration/Play entries, peak two simultaneous
sessions, four confirmed teleports, **17 keepalive replies, 1,021 movement
packets, four chat messages/eight deliveries**, peak two directed visible edges,
and zero protocol errors, timeouts or task failures. Shutdown returns active
connections, authenticated sessions, replicas, visibility edges and frame leases
to zero. Production release SHA-256 matches the final client-tested binary:
`82df53a00ee2250d2a652417d0e68b92d0b217e55fcd96cdacd005384cd6a909`.

| Exact M1D roadmap concept / gate | Implementation and evidence |
|---|---|
| Second player entity | Verified profile info before runtime-ID/UUID player spawn; two-account visual check and independently decoded TCP fixture |
| Visibility | Directional subscriptions to actually sent chunks; indexes, range exit/re-entry and negative coordinates tested |
| Pose/rotation/movement | Full absolute PositionPath updates, separate head yaw and standing/crouching/sprinting metadata; client and fixtures |
| Readable system chat | Plain text labeled `[System relay]`, authenticated connection name; both clients receive both senders' messages |
| Join/leave cleanup | Entity then player-info removal; reserved leave permits/generation checks; cancellation, full queue, rejoin and churn tests |
| No false secure chat indication | No signed Player Chat packet or remote signing key; secure-chat enforcement false, explicit unsigned system relay welcome |
| Review, regressions and resources | M1C ingress fix; load-discovered ingress-coalescing fix; 65 tests/two platforms, descriptor recovery and workloads below |

M1A status/ping and M1B online authentication/Configuration remain. M1C immutable
terrain, creative spawn/flight, bounded movement, chunk batches and keepalives
remain. The final clean client run stayed near spawn; the earlier M1D run also
recorded eight chunk boundaries, and the encrypted terrain regression verifies
positive/negative boundary streaming. No cross-instance gate is claimed.

## Review and high-severity issues resolved

[M1C review](M1D_M1C_REVIEW.md) was recorded before replication expansion.
One authenticated connection could spend the entire shared 8 MiB/s inbound work
allowance using legal-size control frames before the Play packet-rate gate ran.
A local **128 KiB/s wire-plus-decoded** gate now runs before shared charging,
body allocation and decompression. The regression fails on preserved M1C and
passes after the fix, while an independent peer remains usable. It uses private
transport fixtures, not an external-account attack. The default permits normal
supported controls; test client receivers explicitly raise their own receive
allowance to accept terrain. Multiple malicious accounts can still reach the
aggregate ceiling; this is not a complete weighted-fair bandwidth scheduler.

Active load exposed a second issue in the initial M1D design: a directory that
queued every published state and yielded after each command accumulated stale
snapshots. A diagnostic 256-player run lost **111 connections** to replication
ingress saturation, with peak concurrency only 240. This is **failed evidence**,
retained in `m1d-active-256-diagnostic.json` and the pre-coalescing raw logs.
The first failed attempt also remains under `m1d-active-initial-failure/`.

The correction keeps one latest pending snapshot and at most one update notice
per publisher. A short player-scoped mutex protects copying/replacing that value;
it never covers I/O, serialization or a global player table. Notice commands
contain ID/generation, not queued copies of the full state. The directory services
one command plus up to 31 more with a 1ms check on additional batch work, then
yields; a single command is not preempted. A regression replaces 10,000 pending
states with one processed latest state. Final 256-player runs preserve all
65,280 visibility edges with no losses or protocol/timeouts/task/I/O failures.

Chat has an independent session-local admission gate before shared command
ingress, so a single authenticated sender cannot fill it with rejected chat.
The directory independently limits sender rate and aggregate delivery fanout.
No unresolved high-severity defect was established in the reviewed paths after
these corrections. This is a focused engineering review, not a complete security
audit, long soak or proof of all possible resource fairness properties.

## Architecture and ownership

[ADR-004](../decisions/ADR-004-m1d-presence-and-system-relay.md) records the goal,
invariants, evidence, trade-offs, dependency impact, tests and open questions.

| Layer | M1D change / ownership |
|---|---|
| `network` | Local ingress work gate; existing persistent cancellation-safe framing/transforms, shared leases and listener supervision remain |
| `auth` | Unchanged provider-backed encryption challenge and fixed-origin legitimate Mojang verification |
| `protocol-777::presence` | Pure exact-version spawn/info/path/head/pose/remove/system encoders and bounded chat parser |
| `world` / `chunks` | Existing shared immutable stone body; fixed view gains copy/hash/iteration support, no instances or edits |
| `sessions::player` | Own connection controls, validated movement/pose, deadlines, local chat budget and publish pacing |
| `sessions::replication` | Internal bootstrap-realm membership, chunk indexes, shared peer snapshots, bounded chat and generation-safe lease cleanup |
| `server` | Thin assembly and updated stage/help/status; no new worker pool or JVM |

There is **one lazily started event-driven directory task for the service**,
not a task/thread/tick per world. It waits on a bounded command channel when
inactive. After the last leave it discards member/index high-water capacity;
its small service-owned channel/task remain until shutdown without a periodic
timer. There is no mutable world in networking or session handlers. Future world
ownership, world-scoped indexes/chat and switching transactions require M2A
authorization; the private bootstrap directory is not a public WorldId API.

Authenticated UUID, connection ID and monotonic runtime entity ID remain distinct.
The login result retains its verified profile in an Arc; one cached info packet
per peer preserves verified skin properties without copies per viewer. The
directory queries occupied chunks and sent-chunk subscribers, rather than
broadcasting motion to every connected player. The target is not exposed until
initial teleport confirmation; the viewer receives entity packets only after
the corresponding chunk bytes have been sent by its single ordered writer.

Latest state and rosters use watch snapshots; Notify coalesces viewer wakeups.
Each session owns its last-sent visibility table and emits only changed position,
head or pose. Dirty publication/view flushing has a 50ms minimum interval; there
is no periodic world pass. Full absolute position paths simplify correctness
and avoid relative-base drift, with an explicit bandwidth/CPU cost. Intermediate
movement samples may coalesce under load: no 20Hz delivery/latency guarantee is
claimed. Dense scenes inherently cost P*(P-1) visibility edges. Roster copying
during crowded membership churn and control-only wakeups remain profile topics.

## Resource policies and security

Existing limits remain: 512 default connections/authenticated slots; 16 MiB shared
frame reservations; 4 KiB raw read-ahead/connection; 1 KiB pre-auth and 64 KiB
control inbound frames; 256 KiB outbound packets and one pending writer frame;
8 MiB/s shared inbound work; compression threshold 256, ratio 128 and exact-stream
checks; eight concurrent authentication operations/two admitted RSA jobs. Runtime
is one async thread with at most four blocking workers and two-second idle
retirement. Platform TLS may own additional threads. Runtime caps are not a
promise of a particular total native thread count.

M1C's 25 chunk slots, one batch in flight/four chunks per batch, 35,956-byte shared
chunk body and 35,965-byte per-Play scratch remain. Its movement/coordinate bounds,
256 incoming Play packets/s, five-second send groups, ten-second teleport/batch
ACK and twelve-second keepalive/twenty-second reply deadlines remain. Outbound
presence flushing shares a five-second deadline. A slow writer fails locally.

M1D adds 2,048 bounded command slots, one reserved leave slot per registered
lease, one latest pending state/update notice per player, one current shared
peer state, bounded rosters/last-sent entries (at most 511 peers at capacity),
and 32 bounded chat messages/viewer. The validated 256-UTF-16-unit message plus
bounded username generates a packet under 1 KiB, so this queue has a count and
byte bound; accepted packets are Arc-shared. There is no unbounded movement,
chat or subscription mailbox. Profiles remain bounded by the 64 KiB auth response
and existing per-field/count limits, not arbitrary client-supplied player info.

Generation tokens prevent stale updates/leaves or a rejected duplicate join from
removing another member. Reserved permits make leave reliable even when normal
ingress is full or a Join/session future is cancelled. Entity commands may refer
only to the sender's own runtime entity. Input/flags cannot mutate terrain or
another player's state. Attack is parsed and discarded without damage/knockback.

Chat permits a burst of three, replenishing one message/second before ingress;
excess sender traffic closes that connection with a resource-limit rejection.
The directory additionally caps fanout at 8,192 deliveries/second/burst. Reaching
that cap produces bounded sender feedback. A full viewer chat queue marks only
that viewer closed. The directory never awaits a socket. UTF-8 lengths, signature
shape, acknowledgement fields and complete packet consumption are checked.
Control/formatting characters are rejected. Relayed text is literal, not a
client-defined rich component, and no chat content is logged by the server trace.

Chat signatures/timestamps/salts are bounded and consumed but not verified as a
signed chain. Identity comes from the legitimate account connection; text is
explicitly server-labeled System Chat. No Player Chat packet, trusted signature,
remote chat key, authentication fallback or account-credential input was added.
The production binary cannot enable the private synthetic test path.

## Protocol evidence and real-client iterations

[The protocol ledger](M1D_PROTOCOL_EVIDENCE.md) pins all new packet IDs/layouts
and nested codecs against 33 local reference paths. It distinguishes observed
reference behavior, local policy and unverified areas. Important exact-version
details: one-byte zero LPVec3 velocity, PositionPath discriminator, metadata
serializer IDs, fixed chat signature/acknowledgement/checksum fields, fixed
Player Info enum bitset and anonymous NBT **modified UTF-8** System components.
No Mojang implementation bodies or registry assets are bundled. Public packet
wiki access was restricted, so no field-level public cross-check is claimed.

1. First client run visually passed visibility/pose/chat/leave/rejoin, but the
   trace showed one incidental left-click Attack rejection and only one relay.
   The user confirmed the accidental click. The exact Attack layout was inspected,
   then bounded parsing/discarding was added without combat.
2. The next run corroborated two messages/four deliveries and eight chunk
   boundaries. One earlier rejection was independently explained by the user
   pressing **F/off-hand swap**, an unsupported inventory action. This remains a
   known limitation, not a claim of flawless arbitrary gameplay controls.
3. After ingress coalescing and chat admission changes, the final build passed
   the two-client check with four messages/eight deliveries, leave/reconnect and
   **zero protocol failures/timeouts**. Its binary hash matches the final release.

All iterations and resource samples remain in `evidence/`. Emoji was optional
in the final prompt, not explicitly confirmed by the user; the report claims
automated modified UTF-8 coverage, not unobserved visual Unicode verification.

## Build, regression and adversarial results

Final macOS output: m1d-checks.txt (`evidence/m1d-checks.txt`).
All **65 tests** pass, along with `fmt --check`, Clippy with warnings denied,
release build and CLI help. The explicit resource probe is ignored by ordinary
`cargo test` and was run separately through the release test executable.

| Group | Passing tests | Coverage |
|---|---:|---|
| Auth | 5 | Crypto/challenge, identity/profile bounds, failure behavior and resource permits |
| Network unit | 11 | Persistent cancel-safe transforms, frame/compression/byte bounds and new single-peer ingress isolation |
| Network TCP | 11 | Fragmentation, malformed frames, admission/deadlines, task containment and shutdown |
| Protocol | 17 | Existing Login/status/Play; new exact fields, all truncated chat prefixes, signed/unsigned shape, malformed/oversized chat, modified UTF-8 and 20,000 bounded Play inputs |
| Sessions | 15 | Existing identity/Configuration/chunks/keepalive; local chat gate, indexed visibility, pending teleport, coalescing, slow readers, stale generations, queue saturation/cancelled joins and two-peer TCP lifecycle |
| Chunks / world | 3 / 3 | Unchanged geometry/light/heightmaps, reusable scratch, negative coordinates and bounded chunk-view churn |

The encrypted/compressed two-peer TCP test independently decodes player info
before spawn, positions/head/pose/system messages and removal/rejoin. A malicious
own-entity command closes only its sender; the healthy peer continues. The old
M1C encrypted test still checks initial chunks, entering/departing edges,
teleport ACK, a real keepalive/reply and byte-budget cleanup. Weak references and
scoped counters check peer reclamation; cancelled registration and a saturated
normal command queue cannot lose leave cleanup.

**Linux restored:** Alpine arm64, Linux 6.18.58, musl, Rust 1.99.1, OpenSSL 3.5.9,
temporary 2-vCPU/6-GiB VM. Formatting, all 65 tests, Clippy, release build and
descriptor recovery pass. Linux metadata (`evidence/m1d-linux-meta.json`) hashes
all implementation Rust/manifests/data inventories and matches the current host
source. Raw build/test/Clippy/recovery files are `evidence/m1d-linux-*`. The tested
musl flags are `RUSTFLAGS='-C target-feature=-crt-static'` with system OpenSSL.
This is build/conformance/fault validation, not a Linux performance or stock-client
rendering benchmark. The VM and temporary transfer helpers are shut down.

Both macOS listener recovery (`evidence/m1d-listener-recovery.json`) and
Linux recovery (`evidence/m1d-linux-listener-recovery.json`) deliberately lower
the child descriptor limit to 32, observe accept failures, retain an established
status peer, accept a fresh peer after recovery and cleanly release resources.
The new Play input corpus is bounded, not a coverage-guided fuzzer. No 8–24-hour
soak, complete legal-control matrix or Windows/glibc x86_64 validation was run.

## Measured resources and workload definitions

Host: Apple M5 Pro, 18 logical CPUs, macOS 27.0.1 arm64. Rust 1.99.1; locked
Tokio 1.54.1; macOS native OpenSSL **4.0.3** and platform TLS. Production binary:
**3,371,424 bytes**. No new third-party package or workspace crate was added;
the third-party Cargo.lock graph is unchanged from M1C. See
environment and linkage (`evidence/m1d-validation-environment.json`).

CPU/RSS are per-process observations on a shared developer host without dedicated
core affinity. MiB means 1,048,576 bytes. RSS and OS high-water RSS are not allocator
live bytes, and frame leases do not include every runtime/library/kernel cost.
`ps` CPU resolution is 0.01 seconds; averages below are not latency percentiles.

### Active Play scaling — separate server and client test processes

Final active workload (`evidence/m1d-active-complete.json`), generated by
`scripts/measure_m1d_active.py`, uses a private **test-only** release server
process and a separate client-generator process. Only the server PID is sampled.
The production binary has no way to enable this path. Fixtures start at the
internal post-authentication Play boundary with synthetic identities, empty
profile properties, real TCP/AES-CFB8/compression, actual chunk/batch/teleport/
movement/replication/keepalive handlers, and no Mojang/session RPCs. Authentication
cost or real-account capacity is **not** measured. Existing verified-profile
properties/TLS/auth overhead are covered separately by actual client/production
observations. Test-harness main/test threads add overhead; its async executor is
single-threaded and no RSA jobs/blocking workers are exercised in this workload.

All players occupy the same chunk and see every other player: this intentionally
dense case has P*(P-1) directed visibility edges. Each client submits position/yaw
at **20 packets/s**, follows a two-block circular path, changes yaw by three
degrees/update, toggles crouch once/second, and one sender chats every two seconds.
Initial terrain sends 25 chunks/player; clients ACK batches and keepalives.
Clients join in groups of up to 16, then a **20-second measured window begins
after all have spawned**. Initial/churn counters include warmup; CPU percentages
below use sampled active-window deltas. Three seconds after closing all clients,
the same server is sampled and stopped for final counters/OS accounting.

| Active Play fixtures | Directed edges | Maximum sampled RSS KiB (MiB) | Active CPU, % of one core | Sampled threads | Result |
|---:|---:|---:|---:|---:|---|
| 1 | 0 | 11,440 (11.17) | 0.20% | 2 | Pass |
| 32 | 992 | 13,856 (13.53) | 11.15% | 2 | Pass |
| 128 | 16,256 | 25,632 (25.03) | 54.51% | 2 | Pass |
| 256 | 65,280 | 52,576 (51.34) | **99.19%** | 2 | Pass, near CPU saturation |

All four workloads reached the stated concurrency/edges, sent 25 chunks/player,
recorded zero client losses/protocol errors/timeouts/task failures/I/O errors,
and ended with zero connection/replica/edge/frame leases. OS high-water at 256:
52,640 KiB (51.41 MiB). Total server CPU including warmup/cleanup at 256 was
25.216184 seconds, distinct from the active-window percentage. Post-close RSS
was 52,624 KiB: released ownership does not mean pages return to the OS.

**Capacity implication:** this short run establishes bounded behavior at this
workload, not a production promise of 256 players or spare capacity for 512.
At 256 dense moving players the single core is nearly saturated; delivery latency
and intermediate-sample coalescing were not quantified. Admission defaults are
safety bounds, not validated gameplay throughput. More distributed players may
have fewer visibility edges, but no unmeasured linear extrapolation is claimed.
The initial failed 256 result is retained rather than hidden by the passing rerun.

### Repeated active connection churn in one process

Active churn (`evidence/m1d-active-churn.json`) runs **five rounds of 32** fresh
Play fixtures against one long-lived server (160 total), with the same motion,
pose, encryption/compression and one chat sender. Each round remains active for
three seconds after spawn, closes all clients and waits one second. No external
authentication is simulated. All rounds pass; final counters show 4,000 chunks,
peak 32 connections/992 directed edges, zero protocol/timeouts/task failures and
zero active replicas/edges/frame leases. Total server CPU: 2.034845 seconds;
OS peak RSS 15,168 KiB (14.81 MiB), sampled threads two.

Post-cleanup RSS across rounds was **14,112 / 14,256 / 15,040 / 15,040 / 15,136
KiB**. The approximately 1 MiB rise across this short run is reported explicitly.
Scoped peer references and counters are reclaimed, but no allocator live-byte
analysis distinguishes every retained page/cache from every possible leak.
This is short bounded churn evidence, not proof of long-term leak freedom.

### Production idle, pending connections and failure recovery

Production transport profile (`evidence/m1d-connection-resources.json`) uses
the existing M1B-named sampler against the final production binary: five-second
idle, 128/512 partial-frame sockets, five batches of 200 status reconnects,
eight held auth challenges/a rejected ninth, invalid RSA inputs, deadline expiry
and shutdown with open sockets. Global/per-IP admissions are explicitly raised
to 4096/s for a single loopback generator; deployment defaults remain unchanged.
This workload is transport/auth-failure load, **not active world/player capacity**.

| Observation | RSS KiB (MiB) | Cumulative CPU seconds (`ps`) | Threads |
|---|---:|---:|---:|
| Ready / five-second idle | 9,952 (9.72) | 0.01 | 1 |
| 128 pending | 11,280 (11.02) | 0.01 | 1 |
| 512 pending | 15,136 (14.78) | 0.04 | 1 |
| After 512 close | 15,136 (14.78) | 0.05 | 1 |
| Eight pending auth challenges | 15,216 (14.86) | 0.13 | 1 |
| After invalid RSA recovery | 15,456 (15.09) | 0.14 | 4 |
| Final idle / worker retirement | 15,424 (15.06) | 0.14 | 1 |

All 1,000 status reconnects pass. OS peak RSS: 15,472 KiB (15.11 MiB); total
server CPU: **0.137288 seconds**. Median status exchange 0.327ms, p95 0.491ms,
p99 0.619ms, maximum 1.577ms. Startup to ready 9.29ms; shutdown 2.87ms. Final
connection/session/replica/edge/frame counters are zero. Idle RSS remains above
cold startup after churn; no claim of complete heap release is made.

### Two real authenticated clients

Final client sampling (`evidence/m1d-client-complete-resources.json`) contains
867 samples over **187.863 seconds**, approximately every 0.2 seconds, including
initial ready time, legitimate logins, movement/chat and reconnects. Maximum
observed RSS **15,808 KiB (15.44 MiB)**, maximum threads **four**, final threads
two. Cumulative CPU rose 0.04 to 0.40 seconds; approximately 0.36 CPU seconds
over the window is about 0.19% of one core, with coarse sampling and mixed phases.
It is not a steady-state two-player CPU benchmark, exact peak accounting or a
claim that TLS/thread ownership is fully attributed. Final protocol/resource
counters are clean. No comparison claiming superiority over another server was run.

## Reproduction

```sh
scripts/cargo-local.sh fmt --all -- --check
scripts/cargo-local.sh test --workspace --locked
scripts/cargo-local.sh clippy --workspace --all-targets --locked -- -D warnings
scripts/cargo-local.sh build --release --locked
python3 scripts/check_listener_recovery.py --output evidence/m1d-listener-recovery.json
python3 scripts/measure_m1b.py --output evidence/m1d-connection-resources.json
```

The active probe requires a **release test executable**, not the production server:
build it using `scripts/cargo-local.sh test --release -p smolmc-sessions --no-run
--locked --message-format=json` and select the library test artifact's
`executable` path. Then run:

```sh
python3 scripts/measure_m1d_active.py --executable TEST_EXECUTABLE --players 1,32,128,256 --seconds 20 --output evidence/m1d-active-complete.json
python3 scripts/measure_m1d_churn.py --executable TEST_EXECUTABLE --players 32 --rounds 5 --seconds 3
target/release/smolmc --bind 127.0.0.1:25565 --trace-connections
```

For actual acceptance, join with two distinct normally signed-in stock accounts,
move apart, walk/fly/turn/crouch, send messages from both, disconnect/reconnect
one and remain connected at least a minute. Inventory edits/F/drop/commands are
outside scope. Attach `scripts/sample_m1c_client.py --pid SERVER_PID --output
evidence/m1d-client-complete-resources.json` for the retained macOS sampling method.
SIGTERM/Ctrl-C stops the server and lets the sampler save. Keep tracing off for
load tests. Choose new output paths when preserving historical evidence.

## Compatibility, limitations and stop boundary

No persisted state, world data migration or Java/API compatibility change exists.
M1A/B/C normal paths remain; M1D adds nearby presence/visibility-scoped Tab entries
and global system relay within the single bootstrap realm. Terrain remains
read-only. Ordinary left-click attacks have no effect. **F/off-hand swap, item
drop/edit/use/placement, most gameplay interactions and commands remain unsupported
and may disconnect readably.** There is no inventory, combat, health/food,
elytra/swimming simulation, entity AI, mutable world, persistence, multiple
instances, Java bridge, secure player-chat chain or survival mechanics.

Exact `minecraft/core/26.3` known-pack acknowledgement remains required. The M1B
registry fallback limitation and 773 named tags with empty memberships remain;
these are bootstrap data, not faithful vanilla gameplay semantics. No additional
client versions, Windows/glibc x86_64, long soak, coverage-guided fuzzer,
production latency matrix, many real-account authentication capacity or thousands
of instances were validated. CPU saturation and dense membership churn costs
must remain visible engineering limits. No later roadmap work is authorized.

## Rollback

Rollback was **not** performed. Pre-M1D M1C recovery artifacts are local:
`.tools/m1c-rollback.tar.gz` (source/manifests/docs/prior evidence) and
`.tools/smolmc-m1c` (binary SHA-256
`5ea1081f273b0b9ff2aea4e2c0a54f426d6793b0a29a3507d217d2c05d4c258b`).
Archive hash and current binary/native linkage are in the environment JSON.
These development artifacts are excluded from distribution and no Git commit
rollback is available in this workspace.

Stop M1D with SIGTERM/Ctrl-C. Extract the archive into a **new directory** and
build with the pinned compiler/system OpenSSL, or use the preserved binary with
compatible native libraries. Verify status, legitimate login and M1C stone/flight
before deployment replacement. A fresh directory avoids retaining new presence/
replication modules accidentally; do not overlay an archive into a running tree.
Preserve this report and the current user authorization separately: the archived
AGENTS is historical and grants no M2A permission. No saved data needs migration.

Rollback removes multiplayer/system chat and the M1D local ingress safety fix;
M1C's reviewed single-peer shared-budget weakness returns. If rollback is needed,
carry that separately reviewable safety patch into the recovered tree before
exposing it to untrusted traffic, and rerun its targeted regression. All temporary
test listeners, the validation VM and transfer helpers are stopped. **M1D ends here.**
