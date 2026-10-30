# 14 — Risk register and mitigation order

| Severity | Risk | Why it matters | Mitigation/evidence |
|---|---|---|---|
| Critical | Protocol 777 config/registry invalid | clients cannot join | golden trace + real client test |
| Critical | Security identity bypass | impersonation / risk to public servers | real online mode, no trusted proxy mode without signed config |
| Critical | Cross-instance contamination | match/grief/security breach | immutable templates and world-scoped IDs, property tests |
| Critical | Unbounded client/plugin buffers | memory exhaustion | size limits, timeouts, quotas, backpressure |
| High | Misadvertising Bukkit support | false adoption expectations | explicit compatibility matrix |
| High | IPC per-block event tax | defeats Rust latency gains | batch and snapshots, selective cancellable hooks |
| High | Global lock/hot scheduler | serial bottleneck | ownership model, profiling |
| High | COW overlay heavy write slowdown | poor survival performance | threshold promote and baseline tests |
| High | Chat authenticity mistakes | client trust/security warnings | signatures handled correctly or clearly system messages |
| High | Incorrect chunk encoding/light | void, ghost blocks, disconnects | fixture corpus and real-client test |
| High | Resource reclamation leaks | 1000-world workload fails | churn soak, quota/ownership metrics |
| High | JNI unsafe lifetimes | crash/data races | sidecar default, stable proxy handles |
| High | Mojang redistribution/licensing | legal/release blocker | do not embed supplied code or assets in pack |
| Moderate | Feature combinatorial explosion | unmaintainable testing | profiles and dependency solver |
| Moderate | Per-feature runtime toggles | half-transition state corruption | transactional lifecycle policies |
| Moderate | Incompatible version evolution | breaks registries or wire | protocol adapters, pin version |
| Moderate | SIMD/unsafe premature optimization | complexity with little gain | benchmark-first policy |
| Moderate | Template memory cache retention | memory creep | pinned/refcounted cache + budget eviction |

## Decision forcing questions before implementation
Is M1 online-mode required (recommended yes)? Are NBT/registry assets generated at build time or loaded from an external installed compatible data pack? Which plugin trust model is required? Must world instances survive process crashes? Will multiple CPU/NUMA profiles matter? How long should API promises remain stable? Set defaults if no answer: online mode on; plugin sidecar opt-in; ephemeral by default; single supported protocol version; minimal immutable stone chunks; stable-versioned API.
