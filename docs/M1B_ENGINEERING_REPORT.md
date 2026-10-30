# M1B engineering report

Date: 2026-10-29, America/Toronto  
Target: Minecraft Java Edition 26.3, protocol 777  
Status: **Implemented and validated through M1B only. Stop before M1C.**

## Goal and acceptance boundary

The user approved the M1B plan after the M1A architecture review. Work first
resolved the reproducible listener failure, then established persistent session
transport, legitimate online authentication, encryption/compression, Configuration
and the minimum valid Play entry. The governing gate is `13_ROADMAP.md`:
“real client joins play (even if empty) without invalid registry errors.”

**The final signed-in stock 26.3 client passed that wire-state gate.** The server
recorded verified identity, Login ACK, Configuration ACK, Play Login and 403
decoded client Play messages. The user confirmed the completion message again
on the finished binary. Evidence: [client observation](../evidence/m1b-vanilla-client-check.json)
and [final server trace](../evidence/m1b-vanilla-server.log).

The client remained on Loading Terrain during the empty 20-second probe. It
then received “M1B Play entry complete. Terrain and gameplay require M1C.” This
does **not** claim a rendered world or playable server. There are no chunks,
movement/flight, gameplay keepalives, multiplayer/chat, world instances, survival
systems, Java API, sidecar, storage or persistence. M1C and later remain unauthorized.

## Architecture and invariants

| Owner | Implemented responsibility |
|---|---|
| `protocol-777` | Bounded field parsing and version-specific packet encoding; M1A fixture/compatibility helpers retained |
| `network` | Socket admission, listener recovery, task supervision, persistent framing, independent stream transforms, budgets |
| `auth` | Challenge provider, bounded RSA work, fixed-origin HTTPS session verification and validated profile extraction |
| `sessions` | Login/Configuration/Play-entry phases, verified identity leases, shared immutable registry/tag-name packets |
| `server` | CLI, one async runtime, blocking-worker ceiling, dependency assembly and shutdown |

Networking accepts a session handler; it has no dependency on authentication,
session state or worlds. Sessions do not own world data. The bootstrap dimension
identifier is protocol metadata, not a WorldId or an instantiated simulation.
Connection IDs, account UUIDs, protocol session UUID and transient entity IDs
remain distinct. Entity IDs are monotonic and reject overflow rather than reuse.

Each socket has one owned reader and one owned writer. Prefix/body progress,
raw read-ahead, cipher state and write cursor survive cancellation. Queued bytes
are encoded/encrypted once. One pending frame per writer gives bounded backpressure;
there is no unbounded channel. Frames carry shared-budget leases until dropped.
RSA jobs retain their admission permit inside the started operation, even if the
awaiting session is cancelled. Shutdown waits/checks these resources explicitly.

The identity table uses a brief mutex only for join/leave duplicate checks. It
is never held across awaits, normal packet processing, HTTP, crypto, serialization
or world operations. No global hot-path mutex, per-world thread/runtime, per-world
tick, speculative ECS or Java/native-pointer interface is introduced.

Trade-offs, dependencies and future ownership boundaries are recorded in
[ADR-002](../decisions/ADR-002-m1b-transport-authentication.md).

## Listener recovery and failure isolation

The review reproduced M1A process exit after 15 accepted peers with child
`RLIMIT_NOFILE=32`: one `accept()` resource error cancelled every connection.
The initial recovery fix was tested before networking expansion.

The new supervisor distinguishes recoverable interrupted/aborted/reset/resource
errors from fatal listener errors. Resource pressure backs off from 25ms to at
most 1s, while existing tasks and shutdown continue. Healthy accepts do not
create timers. A load run exposed an unnecessary healthy-path timer during
development; it was removed before the recorded final benchmarks.

The regression retains a working status socket during descriptor exhaustion,
verifies its pong, frees descriptors, then verifies a new status connection.
Final macOS and Linux runs both passed, each observing three recoverable accept
errors with clean process exit. See [macOS regression](../evidence/m1b-listener-recovery-final.json)
and [Linux regression](../evidence/m1b-linux-listener-recovery.json).

Connection-task unwind is counted and contained to its owned socket/resources;
the listener remains available. A deterministic injected-panic test verifies
this. Fatal listener/invariant failures still fail explicitly. Process abort,
OOM, native-library faults and future untrusted native code remain shared-process
failure risks, as required by the architecture's isolation distinction.

## Protocol and security evidence

[The packet ledger](M1B_PROTOCOL_EVIDENCE.md) records every implemented state,
direction, ID, layout and reference path. [The source inventory](../evidence/m1b-protocol-sources.json)
pins 54 inspected paths. IDs are separately inventoried and reproducible by
`scripts/inventory_protocol_ids.py`. No packet bodies were guessed from an older
protocol. Exact-version stock-client interoperability independently validates
the implemented flow. Public wiki field-level pages were inaccessible; that
evidence gap is recorded rather than presented as a successful cross-check.

Online login uses the protocol-required provider-backed challenge, continuous
AES/CFB8 stream, correct signed SHA digest, and a certificate-validated response
from the fixed Mojang session endpoint. A claimed UUID is never authoritative.
There is no offline fallback, proxy forwarding, caller-controlled auth URL,
credential/token collection, or bypass for the synthetic load generator.

Profiles/properties and HTTP bodies are bounded. Redirects, environment proxy
routing and automatic HTTP content decompression are disabled. Invalid RSA input
closes the connection without inventing the client's cipher state. Once encrypted
transport is established, account-service/duplicate/capacity failures have readable
rejections before Login Success. No player-chat signatures are forged or claimed.

System OpenSSL supplies primitives; no unsafe Rust or bespoke cryptographic
primitive is introduced. Secret Rust buffers use zeroize. RSA's legacy size and
the stream/hash algorithms are target-protocol requirements, not recommendations
for a new security scheme or a claim of independent cryptographic audit.

### Registry provenance and actual client failures

Registry contents come from the client's acknowledged vanilla `minecraft:core:26.3`
pack. The repository contains names only: 32 registries, 432 entries and 773 named
tags. Entry IDs follow transmitted deterministic ordering. Named tag memberships
are deliberately empty server-owned bootstrap definitions. No Mojang registry/tag
JSON, NBT assets or reference implementation bodies are copied or required at run
time. The private `unz/` is needed only to reproduce development inventories.

The first real attempt authenticated but failed registry loading: an empty tag
map left required tag names unbound. The client error report identified those
bindings; sending named tags corrected the failure. The second attempt completed
Configuration and reached Play but rejected an unsupported packet. Its exact
rejected ID was not captured. Bounded handling of automatic chat-session metadata
resolved the next check, which recorded 0x0a as the first decoded Play packet.
Both failures are retained in `evidence/m1b-client-attempt*.json` and their server
logs. They are not counted as passes.

Declined or mismatched known packs are explicitly unsupported, with a tested
readable Configuration rejection. Full registry NBT fallback and faithful vanilla
tag membership remain absent. No client-supplied NBT is accepted in this slice.

## Resource policy

| Resource | Default / enforced M1B bound |
|---|---|
| TCP connections | 512; operator configurable up to 4096 |
| Authenticated session leases | 512, with duplicate UUID rejection |
| Accept rate | 256/s global, 32/s/IP; fixed windows; 1024 tracked IP keys |
| Shared frame reservations | 16 MiB, no eager allocation of that whole budget |
| Read-ahead | 4 KiB per accepted connection |
| Inbound packet size | 1 KiB before auth; 64 KiB control/decompressed data |
| Outbound payload / queue | 256 KiB / one pending frame per connection |
| Inbound compute exposure | 8 MiB/s aggregate wire bodies + decompressed output; maximum ratio 128 |
| Authentication admissions / RSA jobs | 8 / 2; no unbounded auth queue |
| Runtime blocking threads | Maximum 4; configured 2s idle retirement |
| Handshake/status / Login / Configuration | 5s / 15s / 20s absolute phase deadlines |
| Play conformance session | 20s, then readable close |
| Session HTTP | 3s connect, 8s request, 64 KiB body, at most two idle pooled connections/origin |

Frame leases cover bounded application packet/pending-output allocations, not
total heap, read-ahead, HTTP/TLS/provider internals, compressor workspaces or kernel
socket buffers. Those have separate ownership/capacity bounds and measured RSS.
Control decompression stays on the reactor under these bounds; bodies yield
after 8 KiB and registry sends yield between packets. Large chunk compression
and broader worker/cache design are later profiling work, not implemented here.

## Automated and manual verification

**42 tests pass on macOS and Linux:** 12 protocol tests, 10 network unit/transport
tests, 11 TCP integration tests, five auth tests, four session/configuration tests.
Formatting, Clippy with `-D warnings`, and release builds also pass on both.

| Area | Evidence / assertions |
|---|---|
| M1A regression | Status JSON, exact nonce pong, malformed sequences, fragmentation/coalescing, deadlines, cleanup |
| Persistent transport | Deterministic partial-prefix/body cancellation, partial-write resumption, no requeue, exact encryption boundary with read-ahead |
| Transform bounds | Encrypted/compressed roundtrip; bad lengths/thresholds/zlib/trailing bytes; allocation and work-budget rejection |
| Input resilience | Retained 140 malformed TCP cases; deterministic 20,000-input codec smoke corpus; field truncations/array/enum limits |
| Authentication | Real challenge/provider roundtrip, bad token/key size, permit recovery, signed-digest vectors, profile identity/property limits |
| Account-service failures | Mock HTTP 204/401/429/500/503, malformed JSON, oversized declared/actual bodies; no fallback |
| Configuration | Known-pack positive/decline/version bounds, 32 registries, named-tag regression, finish ACK and shared metadata |
| Identity/resource lifetime | Duplicate/capacity lease recovery, task-unwind containment, socket cancellation, listener rebind, final zero frame leases |
| Real client | Normally signed-in stock 26.3; final account/config/Play success, 403 decoded Play packets and user-confirmed completion message |

Mock HTTP tests exercise response handling without contacting Mojang. Live
authentication-service outage/blackhole fault injection and comprehensive TLS
negative fixtures were not performed. Configured library/phase deadlines and
bounded concurrency are verified by code inspection and the exercised timeout
paths; this is not an exhaustive fault campaign or coverage-guided fuzz run.

Raw macOS commands/results: [m1b-checks.txt](../evidence/m1b-checks.txt).
Linux: [tests](../evidence/m1b-linux-checks.txt), [Clippy](../evidence/m1b-linux-clippy.txt),
[release build](../evidence/m1b-linux-build.txt), [host/toolchain/source pins](../evidence/m1b-linux-meta.json).
The Rust source/manifests tested on Linux match the final macOS workspace hashes.

Linux environment: temporary Alpine 3.24.3 / Linux 6.18.58 aarch64 VM using the
already-installed UTM/QEMU components and macOS HVF; two vCPUs, 6 GiB RAM; Rust
1.99.1 musl target, system OpenSSL 3.5.9, `RUSTFLAGS=-C target-feature=-crt-static`.
It was shut down after testing. The real Minecraft client/server exchange was
on macOS, not inside that guest. No Linux performance capacity is inferred from
the VM, and no VM tooling becomes a server dependency.

## Measured resources — final macOS release run

Machine: Apple M5 Pro, 18 logical CPUs, macOS 27.0.1 arm64. Rust 1.99.1; OpenSSL
3.6.5; release binary **3,074,832 bytes**. Git revision: N/A, workspace supplied
without Git. Source hashes are in [m1b-implementation-sha256.txt](../evidence/m1b-implementation-sha256.txt).

The final synthetic run used loopback with `--max-connections 512
--connection-rate 4096 --per-ip-rate 4096`. Raised rates allow a single-IP load
generator and are not deployment defaults. Connections are paced during opening
to measure live socket occupancy rather than kernel backlog overflow. No account
authentication is simulated: synthetic logins stop at challenge or send invalid
ciphertext, producing **zero authenticated sessions / zero Play entries**.

| Point | RSS KiB | RSS MiB | Cumulative CPU seconds | Threads |
|---|---:|---:|---:|---:|
| Ready | 9808 | 9.58 | 0.01 | 1 |
| After 5s idle | 9808 | 9.58 | 0.01 | 1 |
| 128 concurrent partial frames | 11072 | 10.81 | 0.01 | 1 |
| 512 concurrent partial frames | 14896 | 14.55 | 0.04 | 1 |
| After first 200 reconnects | 15008 | 14.66 | 0.06 | 1 |
| After 1000 reconnects | 15008 | 14.66 | 0.12 | 1 |
| Failed RSA work / recovery sample | 15184 | 14.83 | 0.12 | 4 |
| Final idle after worker retirement | 15152 | 14.80 | 0.12 | 1 |

All five batches of 200 status reconnects completed; RSS after batches two
through five was unchanged at 15008 KiB. Eight pending challenge admissions were
held; the ninth was rejected; invalid RSA responses released all admissions and
the listener still answered a subsequent status ping. Worker threads retired to
the observed single-thread baseline. A true idle timeout and shutdown with four
unfinished connections were also exercised.

Final counters: accepted 1657, peak active **512**, pongs/status responses **1003**
each, login rejections **9** (expected negative workload), idle timeout **1**,
active connections **0**, active sessions **0**, retained frame bytes **0**.
Peak frame reservation was **532480 bytes**. The generic network `io_errors`
counter includes the nine expected auth/capacity failures; it is not exclusively
an operating-system socket-error count. There were no synthetic successful logins.

The 1000 local connect/status/ping/EOF exchanges had median **0.299 ms**, p95
**0.419 ms**, p99 **0.565 ms**, maximum **1.687 ms**. Startup-to-ready in this warm
development-host run was **8.99 ms**; SIGTERM completion **1.44 ms**. These are
Python-driven local smoke observations, not production latency/throughput/SLA
or cold-start guarantees. CPU from `ps` is quantized to 0.01s: unchanged idle
samples do not mean literally zero CPU. The server PID's `wait4` totals were **0.047280s user CPU + 0.072059s system CPU** (0.119339s total), with **15200 KiB OS peak RSS**. Those process totals exclude the Python generator and `ps` subprocesses; OS peak RSS remains distinct from heap-allocation accounting.

Raw data and final counters: [m1b-resources.json](../evidence/m1b-resources.json),
[m1b-resources.log](../evidence/m1b-resources.log). Reproduce with
`python3 scripts/measure_m1b.py` after a release build.

### Separate event-loop responsiveness probe

A diagnostic binary used the unchanged M1B services/runtime limits and a 10ms
timer, with 128 initial partial-frame connections and 1000 paced status exchanges.
All 1000 status exchanges passed. Across 1000 timer samples, lateness was median
**1.497ms**, p95 **2.201ms**, p99 **2.350ms**, maximum **2.432ms**. Shutdown again
left zero connections and frame leases. See [raw results](../evidence/m1b-event-loop-delay.json)
and [probe source](../evidence/m1b-loop-probe.rs).

The probe was compiled from a separate scratch Cargo workspace depending on the
same `smolmc-network`/`smolmc-sessions` paths, Tokio 1.54.1 and serde_json 1.0.154.
It is not part of the deployed executable and starts no gameplay tick. These
timers measure cooperative executor scheduling plus OS timer resolution under
this synthetic workload; there is no idle-control comparison or production
latency guarantee. They do not measure movement, chunks or authenticated load.

### Separate real-account resource observation

The final signed-in client used the unmodified release binary with default
admission settings and opt-in phase tracing. `ps` snapshots every approximately
0.1s observed peak RSS **14656 KiB (14.31 MiB)** and at most **four threads**.
The last pre-stop sample was 14608 KiB, two threads and cumulative CPU 0.05s.
Shorter peaks may be missed. The extra surviving observed thread was not
attributed to a specific platform TLS/runtime component; no thread-leak-freedom
claim is made. The process subsequently exited cleanly with zero active sessions,
connections and frame reservations. See [real-login samples](../evidence/m1b-real-login-resources.json).

RSS retained after churn is not back to startup. Stable short-run samples and
released logical leases do not prove leak-freedom or identify allocator/provider
retention. No allocator accounting, statistical performance campaign, long soak,
packet-byte/wakeup accounting, hundreds of legitimately authenticated players,
or thousands of worlds was measured. These remain explicit limits of the evidence.

## Compatibility, operation and rollback

Protocol target remains 777 only. Status discovery still answers mismatched
versions with target metadata; successful Login requires exactly 777 and verified
identity. M1A status/ping behavior remains covered, while description/capacity
now describe M1B. Internal Rust APIs and metadata inventories are not a stable
Java SDK or persistence format. No existing world/account data was migrated.

Default listener remains loopback. No daemon/service, OS login item, cloud host,
proxy, account setting or external application configuration was changed. The
temporary Linux VM and task-local transfer services were stopped. The user's
Minecraft client was not modified or replaced with a custom client.

Rollback procedure:

1. Stop the running server with Ctrl-C/SIGTERM and verify its stopped counters.
2. Restore the pre-M1B workspace or known-good M1A binary. A private archive was
   saved before edits at `.tools/m1a-rollback.tar.gz`; extract it into a separate
   directory first for inspection. It contains the prior workspace/docs/evidence,
   not Mojang source/assets or the installed toolchain.
3. Restore the archived Cargo files, original three crates and instructions/docs.
   Remove or leave unreferenced the M1B-only auth/sessions crates and new scripts;
   retain the current evidence separately if desired. Build/run the M1A status
   endpoint and repeat status/ping/rejection tests.
4. There are no persistent worlds, database schemas, stored credentials, Java
   hosts or external configuration changes to reverse. The user authorization
   boundary remains M1B maximum unless explicitly changed; rollback is not
   permission to start M1C.

The native OpenSSL dependency is dynamically linked and must be available on a
deployment host. The Linux build tested aarch64/musl; Linux/glibc x86_64, Windows,
IPv6 transport conformance and proxy deployments have not been validated.

## Remaining limitations and next authorized boundary

- Play is a bounded conformance session, not a playable world. M1C must receive
  separate authorization before chunks, spawn/movement/flight/keepalive work.
- Empty tag memberships and core-pack-only synchronization are deliberate M1B
  limitations. Full registry fallback and vanilla tag semantics are absent.
- Chat-session data is discarded and untrusted; secure chat and relay are absent.
- Fixed-window IP admission permits boundary bursts and groups clients behind
  shared IPs; there is no subnet-aware/proxy policy or DDoS guarantee.
- Aggregate frame leases do not account for every allocator/library/kernel byte.
  Expensive RSA work is bounded; started blocking jobs cannot be forcibly aborted.
  Cleanup reports its deadline failure, but runtime teardown may still await a
  provider operation. The exercised fixed-size operations completed normally.
- Opt-in synchronous diagnostic logging is for conformance/debugging. Normal
  packet processing emits no per-tick logs; a production telemetry backend and
  richer failure taxonomy remain future work.
- No world scheduler, instance isolation, COW overlays, Java contracts or claimed
  multi-world performance was implemented. The module/ownership boundaries leave
  that work available at its roadmap stages without embedding worlds in sockets.

**M1B acceptance is complete within this documented scope. No M1C, M1D or later
milestone work was started.**
