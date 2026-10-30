# ADR-002: Persistent transport and isolated M1B session ownership

Status: Accepted within the user's explicit M1B authorization  
Date: 2026-10-29  
Scope: M1B only; no M1C or later authorization

## Goal

Resolve the reproduced M1A listener failure before extending networking, then
establish legitimate online Login, Configuration and minimum Play entry for an
unmodified 26.3 client. Preserve future world isolation by keeping world state
out of connection/session ownership.

## Invariants

- One reader and one writer own each connection's stream/cipher state.
- Cancellation preserves framing and write progress; it never re-encrypts a
  partially sent frame. Read-ahead remains raw until consumed at the current
  encryption boundary.
- Frame allocations/pending writes have bounded owners and shared byte leases.
  No unbounded channel or outbound packet queue exists.
- Authenticated identity comes only from the fixed HTTPS session service after
  the protocol encryption challenge. Claimed UUIDs/hosts are untrusted.
- No world mutation, per-world worker, world tick, persistence, JVM or plugin API.
- Started RSA jobs retain their permits even if the session disappears. Shutdown
  accounts for their completion.
- No unsafe Rust in project crates or bespoke cryptographic primitives.

## Source evidence

The M1A review reproduced server-wide exit from descriptor exhaustion. The first
fix was checked before transport expansion. Final macOS and Linux regressions
retain an established peer during the same fault and accept a fresh status
connection after recovery.

Exact registration order, fields and registry behavior were inspected in the
supplied protocol-777 reference. See `docs/M1B_PROTOCOL_EVIDENCE.md` and
`evidence/m1b-protocol-sources.json`. The stock-client check independently validates
the resulting exchange through Configuration ACK and valid inbound Play packets.
No reference implementation bodies or registry assets are copied into deliverables.

## Design and trade-offs

**Listener supervision.** Recoverable peer/resource errors back off from 25ms to
1s; shutdown and task completion remain serviced. Healthy accepts create no
backoff timer. Unexpected connection-task unwind is counted and contained to its
owned resources. Fatal listener errors still terminate explicitly. This does not
isolate the process against abort, OOM or native-library faults.

**Transport.** Owned TCP halves retain prefix, partial body, raw read-ahead,
transforms, encoded pending bytes and write cursor. `read`/`write` progress is
committed in the owner rather than discarded `read_exact`/`write_all` futures.
Transitions require packet boundaries and an empty pending write. Cancellation
resumes `flush`, never requeues/re-encrypts. Bodies yield after 8 KiB and registry
transmission yields between packets, without adding a world scheduler or pool.

**Budgets.** Shared frame leases, phase sizes, decoded length/ratio checks, an
atomic inbound work budget and session/auth admissions limit exposure. The 4 KiB
read-ahead per connection, library workspaces, HTTP/TLS allocations and runtime
overhead are outside the frame reservation counter; they have separate bounded
owners/capacity limits and OS RSS observations. Reservation peak is not heap usage.

**Session ownership.** Networking accepts an injected handler and has no dependency
on sessions/auth. Sessions use transport, protocol and auth. Verified UUIDs have
scoped leases. A short mutex protects duplicate-identity insertion/removal only;
no awaits, HTTP, crypto, serialization or normal packet processing occur under
it. An additional actor/task just for this cold admission table was unnecessary.
Future world/entity/Java ownership remains separate; no public extension ABI is
promised. M1A's status-only compatibility driver remains available for regressions.

**Crypto and HTTPS.** OpenSSL bindings use the installed provider for the
protocol-required RSA-1024/PKCS#1, AES-128-CFB8, SHA-1, constant-time token comparison
and secure randomness. Java signed-hex formatting is tested protocol encoding.
These legacy algorithms are dictated by the target wire protocol, not a new
crypto scheme. `zeroize` clears Rust shared secrets/decryption scratch. RSA runs
under two permits; the runtime blocking pool is capped at four threads.

One shared async reqwest client uses platform TLS, certificate validation, a
fixed HTTPS origin, no redirects/proxy routing, bounded concurrency/bodies and
deadlines. The Minecraft client owns access-token/session-join work; the server
never reads those credentials. There is no offline fallback. HTTP/2, HTTP/3,
cookies, automatic HTTP decompression, Rustls/AWS-LC and vendored OpenSSL are
disabled. Maintained HTTPS avoids a bespoke HTTP/TLS implementation.

**Compression.** Bounded control-packet zlib remains on the I/O executor with
size/ratio/work limits and cooperative yields; there is no worker task per
packet. Large chunk compression and worker/cache design remain later measured
work. This is a deliberate M1B refinement of the general offload recommendation,
not a scaling claim for gameplay workloads.

**Registry policy.** Acknowledged core-pack contents stay on the client. Only
deterministic identifier inventories are transmitted. Named tags have explicitly
empty membership for this non-gameplay bootstrap. Declined/mismatched packs are
rejected rather than silently assuming data or bundling unlicensed assets. Full
NBT fallback and faithful gameplay tags remain unimplemented.

**Play boundary.** Bootstrap Login supplies a dimension key and adventure-mode
metadata, without creating a running world. No chunks, movement, flight or
gameplay keepalives are added. The client can remain on Loading Terrain during
the 20-second conformance probe, then receives a readable Play disconnect.
Chat-session metadata is bounded/discarded and never trusted for identity or
secure-chat claims. This proves protocol entry without claiming a playable world.

## Dependency impact

Five scoped workspace crates. New direct runtime dependencies: system OpenSSL
bindings, reqwest with `native-tls-no-alpn` only, flate2/miniz_oxide, zeroize and an
explicit Unix libc dependency already present transitively. HTTP is test-only.
Exact versions/transitives are locked. Measured macOS binary: about 3.07 MB;
resident provider/runtime memory is reported separately. No JVM, ECS, storage,
NBT framework, plugin runtime or world engine is added.

## Measurable acceptance tests

42 tests pass on macOS and Alpine Linux, as do formatting, Clippy and release
builds. Descriptor recovery passes on both. The final signed-in stock client
completes auth/config/Play and sends decoded Play traffic. Synthetic profiling
reaches 512 pending sockets, 1000 status reconnects, bounded auth failures/RSA work
and clean shutdown. Final connections, sessions and frame leases are zero.
Raw evidence and detailed results are in `docs/M1B_ENGINEERING_REPORT.md`.

## Compatibility and rollback

M1A status/ping remains supported; description/capacity now describe M1B.
Legitimate 777 Login works; unsupported versions/transfers reject. No saved
worlds, database changes, Java APIs or migrations exist. Stop and restore the
M1A workspace/binary to roll back; the report describes the pre-change archive.

## Open questions and limitations

Before separately authorized M1C: replace the probe close with the specified
game bootstrap/keepalive/chunk lifecycle; select faithful redistributable tag/data
sources as needed; profile chunk CPU/backpressure. Full registry fallback,
Linux/glibc x86_64 and Windows validation, long soak, coverage-guided fuzzing and
platform TLS thread attribution remain open. Thousands of worlds are not
instantiated or benchmarked. No later milestone is authorized.
