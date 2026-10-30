# 19 — Optimization hypothesis bank (ranked by strategic value)

**All proposed, unimplemented and unmeasured.** Priority `P0` = essential architecture; `P1` = likely useful for target workload; `P2` = investigate only after profiling. "Savings" are qualitative hypotheses, not observed results.

| Rank | Area | Potential technique | Why it might help | What can go wrong | Prove/disprove |
|---|---|---|---|---|---|
| P0 | Idle worlds | event-driven parked lifecycle; no unconditional world ticks | eliminate per-instance wakeups | timers silently fail or work stalls | CPU/wakeup delta for 0..1000 idle instances |
| P0 | World cloning | immutable template chunks + copy-on-write overlays | share identical terrain across arenas | overlays grow; reference leaks; cleanup complexity | memory per clone, edit-heavy crossover |
| P0 | Ownership | owner-based serialized mutation queues | avoids coarse global world locks | hot-owner saturation | throughput/p99 with independent worlds |
| P0 | Bounded queues | byte/entry backpressure for sockets/IPC/storage | prevents burst OOM | disconnects under load | packet floods + realistic bursts |
| P0 | Feature residency | don't start disabled workers, caches, JVM | remove overhead from irrelevant mechanics | accidental hard dependencies | per-feature RSS/wakeup counters |
| P0 | Minimal chunk generation | constant virtual flat stone recipe | avoid terrain generator and per-world NBT | wrong biome/light/heightmap | vanilla client sees world w/o corruption |
| P1 | Shared chunk payload | precompute immutable chunk sections / light payloads | less repeated serialization | wrong chunk coords, compression state or block updates | cache hit %, bytes/client, correctness |
| P1 | Batch sends | grouped chunk streaming + priorities | improve login/movement consistency | latency/queue spikes | join p95, bandwidth |
| P1 | Palette representation | uniform section → sparse → dense palette evolution | small memory for homogeneous stone/air | transition complexity and cache churn | bytes/chunk by edit count |
| P1 | Compression | cache **pre-encryption** compressed immutable payloads by version/settings | saves CPU for repeated views | packet container and length mismatch | compression CPU & packet correctness |
| P1 | Spatial visibility | chunk-grid subscriptions for players/entities | avoid O(players²) broadcast | ghosts at border transitions | broadcast work per motion |
| P1 | Arena teardown | generation counters, reference tracking, bulk drop | quick safe recycling | stale tasks/handles after reset | destroy/reset churn, no leaks |
| P1 | Java bridge | batch block edits/reads + event coalescing | avoids IPC per block/player packet | stale cache or semantic drift | operations/sec, latency p99 |
| P1 | Disk writes | optional journal/dirty chunks; asynchronous persistence | avoid blocking game loop | crash recovery semantics | fsync and recovery under faults |
| P1 | Runtime | shared I/O executors and bounded world owners | no thread per world | fairness and tail latency | trace ready queue and wakeups |
| P2 | SIMD varints/palettes | vectorized codecs where profiled hot | lower codec cycles | portability/unsafe correctness | fuzz + cycle profiles |
| P2 | Per-region parallelism | actor ownership split within busy world | better many-core scaling | cross-boundary complexity | only after a single busy world saturates |
| P2 | Slab allocations | pools for sessions/packets/entities | reduces allocator cost | retention & fragmentation | RSS high-water + alloc profiles |
| P2 | Mmap templates | map large immutable region content | possible sharing across process mapping | random I/O, OS page cache, copy-on-write semantics | cold vs warm RSS/IO |
| P2 | ECS | dense entity storage | potential benefit for thousands of entities | needless M1 code complexity | entity-heavy micro + macro benchmarks |
| P2 | Alternate network stacks | io_uring/other specialized I/O | lower syscalls | complexity and OS restrictions | p99 & CPU under real packet mixes |

## Critical architectural distinction: payload sharing
Store one immutable *uncompressed content representation* keyed by `(protocol, dimension, template revision, chunk coordinate + relevant section/light version)`; optionally cache compressed forms keyed by compression threshold and dictionary/settings. **Never share connection encryption state or ciphertext**, since cipher streams are stateful per connection. The outer packet framing and packet type correctness must be maintained. Position-dependent values prevent naive sharing of one coordinate-dependent chunk packet across arbitrary locations.

## Memory model (conceptual only)
`RSS ≈ core + connection_state×P + shared_templates×T + instance_metadata×N + changed_sections×S + stream_buffers + optional_feature_residency + allocator/OS overhead`.

`P` players; `T` distinct template data; `N` instances; `S` modified sections. This is an accounting model **not a claim of linear accuracy**. Linux/macOS shared pages, page cache, Arc refcount slabs, fragmentation and compression affect observed RSS. Use retained bytes by component and proportional set size (PSS) where supported.

## Concrete benchmark matrix to falsify hypotheses
1. **Cold** 0/1/10/100/1000 worlds with zero players: count wakeups/sec + RSS + time to create/destroy.
2. **Static arenas** one 16×16 chunk template cloned 1/10/100/1000 times, no edits; scan sharing/range cache.
3. **Sparse edits** 1/100/10,000 edits/arena; memory growth and dirty-overlay promotion.
4. **Heavy write** fully modify 10/50/100% sections; compare COW with per-instance dense arrays.
5. **Players** 1/10/100/500 connected across few or many instances; p99 update delay, bandwidth, scheduler fairness.
6. **Java sidecar** off/on/kill-under-load, bulk vs N one-block RPCs; Java RSS counted separately.
7. **Churn** 10000 create→join→edit→reset→destroy operations; scan retained refs and bytes.
8. **Mixed heavy survival** deferred until feature exists, and should not be included in early performance claims.

## Golden rule
Do not add `unsafe`, lock-free algorithms, custom allocators or bespoke codec implementations without a measured bottleneck and correctness test. Rust eliminates many memory-safety risks; it does not automatically make algorithms/cache layout fast.
