# ADR-007: bounded replication writes and wake coalescing

Date: 2026-10-29. Status: implemented and focused tests/short profiles pass.
Authority: targeted replication/CPU work only; no M2C or unrelated milestone.

## Goal → invariants → source evidence

Reduce repeated work in the existing indexed replication pipeline. Preserve
entity IDs, packet/teleport ordering, sent-chunk visibility, per-instance isolation,
latest-state coalescing, five-second write deadlines, byte budgets and online
security. Checkpoint: `replication-perf-before` at `d046b37`.

`InstanceDir` already uses member HashMaps, chunk occupants/subscribers and viewer
sets; stable-chunk movement visits the selected viewers. The dense baseline's
one-second worker sample has 47/88 leaves in `sendto` and 15/88 in AES routines.
The directory update p95 bucket is ≤8 μs. See
[the report](../docs/REPLICATION_CPU_REPORT.md) and raw profiles for measurements.

## Design trade-offs → dependency impact

- Stage at most 64 ordered replication packets / 16 KiB total plaintext payload.
  Frame and compress each independently, reserve shared transformation bytes,
  then apply the connection-local AES-CFB8 stream and write as one pending batch.
  Partial writes retain their encrypted bytes/cursor; no re-encryption on resume.
  A ciphertext-equivalence test compares concatenated individual sends, batching
  and the following frame, with compression on/off and threshold boundaries.
- Cache the immutable roster identity, checking removals only on roster change.
  Membership changes and world-switch clear still remove old entity/Tab state.
- Coalesce viewer wakes while already dirty, until its existing 50 ms flush due
  time. Inbound controls/chat, publication and other deadlines remain enabled.
- Short-circuit complete chunk views with 25 validity checks rather than repeated
  ring/contains searches. A moved center still detects missing entering chunks.
- Pre-size the existing position/head/pose encoders; serialized fields are unchanged.

The staging buffer adds bounded caller memory, at most 16 KiB payload plus 64 Vec
headers per viewer, separate from leased transformation memory. Actual RSS rises
about 1 MiB in these loads. One pending transformed write remains; recipients,
compression thresholds, crypto/keys and update pacing are unchanged. A successful
View flush writes all staged frames; errors/cancellation close the owning session.
There is no new durable movement queue or encrypted cross-client cache.

No dependency, thread, runtime, protocol layout or authentication change. Profiling
histograms/timestamps/signals exist only in the private test build. The benchmark
client publishes its observation map only on membership changes, avoiding observer
copying that throttled the initial high-delivery dense diagnostic.

## Acceptance tests → open questions → rollback

78 focused network/TCP/session/protocol/world tests pass, including exact
ciphertext/continuation equivalence, cancellation/partial-write resume, invalid
batch limits, visibility/chunk transitions, stale tokens, full-queue cleanup,
world isolation/switching, pose/chat and rejoin. Fmt, workspace all-target Clippy
with warnings denied and release build pass. Three representative short final
loads pass with matched input rates; the report records CPU/RSS/fanout/latency.

AES-CFB8, necessary per-recipient bytes, serialization/allocations and lookups
remain. No Linux, long-soak or new stock-client run was performed for this focus.
Restore `replication-perf-before` into a separate worktree after shutdown to roll
back; there is no persisted-state migration. Stop before M2C.
