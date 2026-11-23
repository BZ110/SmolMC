# Focused multiplayer CPU optimization

Original capture filenames refer to the [local validation archive](../evidence/README.md).

Date: 2026-10-29. Completed within explicit replication/CPU authorization.
**No M2C or unrelated milestone work.** Pre-change checkpoint:
`replication-perf-before` → `d046b37` (validated M2B).

## Outcome

The expensive avoidable work was the small-packet outbound path. Short matched
loads show lower CPU with the same movement input rate and more delivered state.
The dense case still uses substantial CPU; required fanout and AES-CFB8 remain.

CPU below is percent of **one core**. RSS is maximum sampled **server PID** RSS;
client-generator CPU/RSS are separate and excluded.

| Players / worlds | Before CPU | After CPU | Relative CPU reduction | Before RSS MiB | After RSS MiB |
|---|---:|---:|---:|---:|---:|
| 256 / 1 | 97.88% | 82.48% | 15.7% | 52.05 | 53.39 |
| 256 / 16 | 36.44% | 22.86% | 37.2% | 28.98 | 29.94 |
| 128 / 1 | 55.60% | 27.69% | 50.2% | 24.97 | 26.06 |

Dense position delivery improves **2.54×** while CPU falls **15.7%**. CPU per
emitted position, including associated head/pose/control work, falls approximately
**67%**; this is throughput-normalized efficiency, not a per-function cycle claim.
The historical M2B 94.58% was a different 20-second run. This report uses new short
baselines for comparison and does not mix that historical figure into the table.

## Architecture inspected → actual bottleneck

- One event-driven directory contains separate instance-scoped `InstanceDir`s.
  Membership is HashMap based; chunk occupant/subscriber indexes and explicit
  viewer sets select recipients. Stable-chunk movement does not recompute global
  visibility or scan unrelated worlds. Dense scenes inherently have P*(P-1)
  directed edges because everyone can see everyone.
- A per-player latest slot and one queued update notice already coalesce ingress.
  Immutable watch snapshots/rosters, sent-chunk readiness and generation-safe
  reserved leave were retained.
- Each viewer previously checked removals against an unchanged sorted roster on
  every flush, built growing Vec encoders, framed/encrypted every small packet
  separately, and issued a socket write per packet. Dirty viewers also kept
  waking before their existing flush deadline; complete chunk views repeatedly
  searched for nonexistent missing chunks.
- Baseline native sample: **47 of 88 worker-thread samples in `sendto`**, versus
  **15 in AES/CFB8 routines**; directory update p95 bucket ≤8 μs. Final sample:
  **10 of 89 in `sendto`, 44 in AES/CFB8 routines**. Sampling is one second at
  10 ms intervals, not precise function CPU attribution. The sleeping test harness
  main thread is excluded from these worker counts. No compression hotspot was
  established for the below-threshold movement packets.

Raw profiles: dense baseline (`evidence/replication-dense-baseline-corrected-raw/p256-w1.sample.txt`),
final dense (`evidence/replication-final-raw/p256-w1.sample.txt`). Each other short
scenario also has a native sample in its raw evidence directory.

## Changes → invariants → trade-offs

[ADR-007](../decisions/ADR-007-bounded-replication-write-batches.md) records the
source evidence, design, limits, tests and rollback.

1. **Bounded write batching:** up to 64 replication packets / 16 KiB aggregate
   plaintext payload. Every packet keeps independent framing/compression; the
   connection cipher processes the same ordered byte stream in one pending write.
   Byte-budget reservations precede transformed buffer allocation. Partial writes
   preserve ciphertext/cursor, including cancellation; no repeated encryption.
   Large cold player-info packets retain their existing standalone send path.
2. **Stable-roster reuse:** removals are checked only when the immutable roster
   identity changes. New membership, leave/rejoin and switch cleanup still run.
3. **Viewer-wake coalescing:** an already-dirty viewer waits for its existing
   deadline rather than consuming repeated Notify wakes. Incoming controls/chat,
   publisher deadlines, chunk ACKs and keepalive remain independently serviced.
4. **Allocation/search reduction:** position/head/pose encoders reserve their
   small known capacity. Complete chunk views return after one 25-slot validity
   pass, avoiding repeated ring searches; moved centers retain correct behavior.

The directory/recipient indexes, entity IDs, wire fields, teleport handling,
instance ownership, 50 ms minimum publish/flush intervals, compression threshold,
AES-CFB8/provider/keys, input protections and five-second write deadlines remain.
No simulation thread, worker pool, dependency, packet layout or auth mode added.

The local staging buffer costs bounded extra memory: ≤16 KiB payload plus 64 Vec
headers per viewer, separate from transformed-frame byte leases. The observed
RSS cost is about 1.0–1.35 MiB. A flush succeeds only after staged frames are
written. Its owner closes on error/cancellation; this does not create an unbounded
movement queue. Encrypted bytes remain connection-local. Ciphertext equivalence
and continuation are tested against the original individual-send path.

## Fanout, processing latency and update delivery

Histograms use powers-of-two microsecond buckets; the table gives **p95 bucket
upper bounds**, not exact percentiles. Flush time includes awaits/yields.
Directory-handler p95 bucket bounds improve from 8→4 μs (256/1), 8→4 μs
(256/16), and 16→4 μs (128/1); this remains a small component.
Publication-to-write age samples one in 64 observed versions, measured from
server snapshot publication through the corresponding write completion. It is
not client rendering latency and excludes input-to-publication pacing.

| Players / worlds | Directed edges (unchanged) | Position packets/s before → after | Flush p95 bound ms before → after | Publish-to-write p95 bound ms before → after |
|---|---:|---:|---:|---:|
| 256 / 1 | 65,280 | 455,036 → 1,157,963 | 131.072 → 16.384 | 131.072 → 65.536 |
| 256 / 16 | 3,840 | 73,721 → 75,106 | 4.096 → 1.024 | 65.536 → 65.536 |
| 128 / 1 | 16,256 | 272,339 → 303,556 | 16.384 → 8.192 | 65.536 → 65.536 |

Input movement rates remain comparable (nominal 5120/s for 256, 2560/s for 128;
small discrepancies reflect asynchronous snapshot/window boundaries):

| Players / worlds | Before movements/s | After movements/s |
|---|---:|---:|
| 256 / 1 | 5162 | 5155 |
| 256 / 16 | 5155 | 5157 |
| 128 / 1 | 2578 | 2578 |

There was no lower sender rate, smaller visibility radius, different recipient
selection, disabled encryption/compression, or longer flush period for a CPU win.
Faster drains reduce existing latest-snapshot coalescing losses. Intermediate
movement remains coalescible under the established contract; 20 Hz delivery to
every observer is not guaranteed. Separate pose, control/ACK, membership and
teleport ordering paths retain their regression checks.

## Short benchmark method and diagnostic correction

Only the requested three scenario types were used: 256/1, 256/16 (16 per world),
128/1. Each run warms for one second after all clients load, then measures roughly
nine seconds. Existing private release fixtures use TCP/AES-CFB8/compression,
20 Hz circular movement/yaw, 1 Hz crouch and one chat sender per two seconds.
Only server PID CPU/RSS are sampled on the shared macOS M5 Pro host. Process CPU
resolution is 0.01 s. No affinity/statistical confidence or capacity guarantee.

The first optimized dense diagnostic doubled delivery enough to expose a client
observer flaw: it cloned its entity HashMap on every packet. Input fell to ~4369
movements/s. That diagnostic is **not used for the final CPU comparison**. The
observer now copies membership only on spawn/remove; gameplay generation is
unchanged. The dense baseline was rerun with the corrected client; its input is
~5162/s versus final ~5155/s. The original 16-world/128 baselines were unthrottled
and match final input rates, so were retained instead of repeating those baselines.

There were ten short windows total (initial three baseline, initial three candidate,
one necessary corrected dense baseline, final three): approximately 90 seconds
of active measurement, not another scaling campaign. No Linux/soak/large sweep.
The before/after common test-only timestamps/histograms/signals are absent from
the production binary. Baseline harness patch (`evidence/replication-baseline-harness.patch`)
reconstructs instrumentation and the corrected observer over the checkpoint.

Evidence: original baselines (`evidence/replication-before.json`),
initial diagnostic (`evidence/replication-after.json`),
corrected dense baseline (`evidence/replication-dense-baseline-corrected.json`),
final three cases (`evidence/replication-final.json`),
environment/binary hashes (`evidence/replication-environment.json`) and
source hashes (`evidence/replication-source-sha256.txt`).
All selected runs reach expected visible edges, lose zero clients and finish with
zero connections/replicas/frame leases and zero protocol/timeouts/task/I/O errors.

## Validation and remaining bottlenecks

**78 focused tests pass**, 0 failures; the opt-in load probe is separately invoked.
Groups: network unit 13 + TCP 11, sessions 23, protocol 17, world 14. This includes
new batch ciphertext equivalence across compression boundaries and subsequent
cipher continuation, partial-write cancellation/resume, invalid count/byte/empty/
overlap rejection and lease cleanup. Existing visibility/negative chunk movement,
pending teleport, generation guards, 10,000-state ingress coalescing, slow readers,
queue saturation/cancelled join, world switch/isolation, chat/pose and leave/rejoin
fixtures pass. Focused output (`evidence/replication-tests.txt`),
workspace all-target Clippy (`evidence/replication-clippy.txt`),
format check (`evidence/replication-fmt.txt`) and
release build (`evidence/replication-release.txt`) pass.

Dense 256 still consumes ~82.5% of one core. With ~1.16 million position packets/s
plus head/pose messages, required per-recipient AES-CFB8 bytes now dominate sampled
work. Serialization/allocations, lookups, and unavoidable fanout remain. No shared
ciphertext, altered cipher, relative-movement protocol redesign or plaintext
publication cache was added. Short shared-host measurements do not establish
512-player capacity, long-term latency, Linux behavior or stock-client rendering
for this change; those were not retested in this focused scope.

## Reproduction and rollback

```sh
scripts/cargo-local.sh test -p smolmc-network -p smolmc-sessions -p smolmc-world -p smolmc-protocol-777 --locked
scripts/cargo-local.sh fmt --all -- --check
scripts/cargo-local.sh clippy --workspace --all-targets --locked -- -D warnings
scripts/cargo-local.sh build --release --locked
scripts/cargo-local.sh test --release -p smolmc-sessions --no-run --locked
python3 scripts/measure_replication_short.py --executable TEST_EXECUTABLE --seconds 9 --output evidence/replication-repeat.json
```

Select the release test executable printed by Cargo. For a baseline use a separate
worktree at `replication-perf-before`, apply the saved harness patch and build into
a **separate target directory**. The `--dense-only` option permits the single
conditional diagnostic case. These clients are private post-auth fixtures, not
Mojang authentication or real-account capacity tests; production cannot enable them.

Rollback was not performed. Stop the process and create a separate checkout with
`git worktree add --detach ../SmolMC-pre-replication replication-perf-before`.
Rebuild with pinned Rust/OpenSSL and run focused checks before replacement. No
saved-state migration; ephemeral edits/connections do not survive restart.
**Focused work stops here; M2C and unrelated milestones remain unauthorized.**
