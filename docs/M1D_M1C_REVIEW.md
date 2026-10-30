# M1C review before M1D

Date: 2026-10-29. User provisionally accepted M1C and authorized this review,
fixes for high-severity correctness/resource issues, then M1D only.

## Goal and invariants

Check the M1C report against implementation ownership, adversarial input,
backpressure and actual evidence before adding cross-player fanout. Retain
legitimate online identity, shared immutable terrain, persistent cancel-safe
transport, bounded storage and no world simulation workers.

## High-severity finding: one peer can consume the shared ingress allowance

M1C has a shared 8 MiB/second wire+decoded work allowance, but no per-connection
byte allowance. Play's 256-packet/second check occurs after transport decoding.
One authenticated peer can send 128 legal-size 64 KiB control frames in a second
and exhaust the shared allowance, after which an unrelated peer is rejected.
The M1C transport-only load did not exercise this successful-session attack.

The targeted regression injected into the preserved M1C source fails before the
fix: its third full-size frame still succeeds. See
`evidence/m1d-m1c-resource-reproduction.txt`. This is a deliberate expected failure,
not a passing M1D test. It establishes the missing local gate without requiring
account fabrication or calling the external session service.

**Resolution before replication expansion:** add a connection-local 128 KiB/s
wire+decoded work allowance, charged before the shared allowance and before
allocation/decompression. It needs no worker or periodic reset task. The third
64 KiB frame is rejected locally while an independent peer still reads normally;
renewal and resource cleanup are checked. Fixed one-second windows may permit
two windows' traffic near a boundary; this is resource policy, not precise
bandwidth shaping. Multiple malicious accounts can still reach the aggregate
ceiling; the global cap remains a process protection, not complete fairness.
Trusted library clients may explicitly raise their own receiver allowance up to
the shared ceiling to decode terrain in tests; production server sessions retain
the default. Existing socket, admission, frame and session budgets are preserved.

## Other findings and recommendations

- Corrected M1C light bitset and full teleport ACK agree with the pinned nested
  codecs and exercised stock client. Keep the failed evidence and exact fixtures.
- Immutable terrain/scratch ownership is bounded. 512 pending sockets do not
  measure 512 authenticated Play sessions. Add active Play replication workloads
  and label internal synthetic probes separately from legitimate-account tests.
- The single I/O executor is suitable only with bounded work and cooperative
  yields. M1D must coalesce movement, limit chat fanout, and avoid socket awaits
  in the replication directory. Dense nearby players inherently create many
  directed visibility edges; measure that case separately from separated players.
- Reliable leave cleanup must survive cancelled tasks and a full command queue;
  an ordinary best-effort `try_send(Leave)` in Drop is insufficient.
- M1C Linux results are absent, not passing by inheritance from M1B. Attempt a
  fresh Linux build/test and preserve source hashes if feasible.
- No additional high-severity M1C defect was established in the reviewed paths.
  This is a focused review, not a complete security audit or exhaustive Play fuzz.

## M1D design contract

Goal: two signed-in stock clients see movement/orientation/pose, exchange clearly
labeled system messages, and remove peer resources on leave/rejoin. Use an
event-driven server-scoped replication directory with bounded ingress, shared
latest peer snapshots and chunk-indexed subscriptions. Sessions retain socket
and authoritative control ownership; the directory owns replication membership
only. It is not an M2A world manager. No global hot-path lock, per-world worker,
simulation loop, Java/API or survival machinery. Reuse existing dependencies.

Exact protocol-777 new packet bodies and nested codecs must be pinned before
implementation. Tests must cover malformed chat, spoofed/stale references,
coalescing, visibility edges, cancellation/queue saturation, slow peers, and
cleanup. Report workload sizes, movement/chat rates, visibility density, RSS,
CPU, threads and retained resources, without extrapolating to untested accounts
or thousands of worlds. Record final evidence, limitations and M1C rollback.

## Final verification and load-discovered correction

The M1C ingress regression and all 65 final tests pass on macOS/Alpine Linux.
The final stock-client M1D check passed with two legitimate accounts and clean
protocol/resource counters. The initial M1D 256-player fixture run exposed stale
snapshot accumulation in its shared command queue; this was fixed with one
latest pending state/notice per player and bounded command batching. The final
256-player fixture run reaches all 65,280 directed edges without connection loss
or retained resources, but nearly saturates one core. This correction and the
failed evidence are documented in `M1D_ENGINEERING_REPORT.md`; the passing run
is not a production capacity or many-real-account authentication claim.
