# 11 — Performance measurement plan: prove, do not advertise

## Competitors and fairness
Compare **same host/CPU, client version, view distance, active feature set, worlds/chunks, plugin load, compression and encryption**: proposed RustMC (when implemented) vs Pumpkin, Minestom, Paper and, where relevant, Folia/FerrumC. For 100 separate Paper instances compare **functionally equivalent** services. Clearly disclose different survival mechanics and features rather than declaring total superiority.

## Five workload families
A. **Cold idle**: server ready, 0 worlds / 1 / 10 / 100 / 1000 parked stone arenas, no players. B. **PvP instances**: 1-4 flying players per arena, many independent arenas, chat/movement. C. **Many viewers**: 100-500 players in a single lobby, same chunks, high replication pressure. D. **World churn**: create/destroy/reset 1000 match instances; test leaks and tail latencies. E. **Mutable worlds**: 0/100/5000 edits per arena; compare COW to full copies; include a survival-heavy workload eventually.

## Metrics (all with units and provenance)
- RSS, PSS (where available), allocator resident vs allocated, peak RSS, retained bytes after unload.
- Incremental memory per empty arena = slope of heap/RSS as N increases, not RAM/N alone.
- Incremental CPU per parked arena = slope of CPU seconds/wall second as N increases.
- Startup to **client joinable** (not merely process spawned).
- Player join p50/p95/p99, login-to-world p50/p95/p99.
- Tick simulation p50/p95/p99 **only for worlds that tick**; scheduler wakeups/sec.
- Network send bytes/player/s, pending buffer bytes, packets/s, compressed chunk cache hit ratio.
- World clone/reset p50/p95/p99, overlay bytes per modification count.
- CPU cycles/player movement, allocation/player movement, memory released after disconnect.
- Sidecar RPC p50/p95/p99 and events dropped/timeouts when enabled.
- Failure rate and correctness; passing protocol tests is not optional for speed scores.

## Concrete experiment design
Run each scenario in 5+ replicates after warmup, use an isolated host and scripted compatible clients, record CPU model/core count, RAM, OS, runtime versions, process settings, compilation optimizations, user sessions, packet/compression settings. Separate cold cache and warm cache. Report medians/distributions and variance, and save test scripts/config (future implementation) with raw CSV. Monitor network bandwidth; it can dominate chunk-heavy runs.

## Do not fabricate numbers
The project has **no observed benchmarks**. Example goals like '<1 MB per empty arena' or '100 instances per GB' are product **hypotheses**, not measurements. Define targets after a baseline measurement and after deciding what an 'empty instance' contains.

## Target hypotheses to test, not commitments
H1: an unloaded/parked instance adds **no per-instance periodic timer**. H2: the incremental resident memory cost of an unmodified shared arena is much lower than making a standalone copy. H3: instance reset scales with overlay size, not full template size. H4: Java-off deployment uses no JVM and fewer resources than Java-on mode. H5: first-login protocol correctness survives chunk optimizations.

## Instrumentation requirements
Per-world/feature live counters, allocation tracing on representative workloads, profiling hooks, heap/RSS metrics, structured tracing with world and player IDs, queue depth histograms, tokio task count (if used), cache hit/miss. Instrument overhead itself; use sampling to avoid instrumentation distorting results.

## Failure threshold
If memory grows monotonically after instance churn with no plateau, if perf varies wildly, or if a model cannot reproduce correctness at scale, stop adding features and fix resource lifetime/state ownership first.

## Benchmark report format
`Date, commit, target_protocol, hardware, OS, build_flags, scenario, feature_profile, players, worlds, view_distance, compression, trial, CPU_seconds, PSS_MB, RSS_MB, p99_join_ms, p99_sched_ms, errs, notes`.
