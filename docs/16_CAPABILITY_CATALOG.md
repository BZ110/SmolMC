# 16 — Feature-capability catalog and disablement matrix

## Interpretation
`Required` means non-optional for the announced **first runnable milestone**, not that every mechanic is permanently baked into the protocol library. `Deferred` means documentation exists but NO feature implementation is authorized. Runtime scope = global/world/connection. Policies: `build`, `restart`, `reload`, `quiesce`, `immediate`. Costs are **hypotheses to benchmark**, not quantitative claims.

| Capability | M1 profile | Scope | Disable / switch policy | What remains when OFF | Performance hypothesis |
|---|---|---|---|---|---|
| TCP framing / bounded codec | Required | connection | build/restart | cannot accept clients without it | packet allocations & copies |
| Login/authentication | Required | connection | restart | status endpoint could remain | crypto/session checks |
| Configuration registries | Required | connection | restart | cannot enter Play without negotiation | immutable cache once per protocol version |
| Player session/movement | Required | connection | disconnect | no players; status optional | active-player updates |
| Chunk streaming | Required | world/connection | reload | no visible world | compressed packet cache |
| Immutable stone generator | Required | world | reload | no world loaded | virtual chunks reuse |
| Creative + flight | Required | player | immediate subject to state | fall back to other gamemode not in M1 | minimal |
| System chat relay | Required | connection | immediate if safe | no messages | modest |
| Two-player visibility | Required | world | quiesce | solo player only | range filtering |
| World registry and isolation | Required | global/world | restart/reload | cannot host separate worlds | ID + subscription bookkeeping |
| COW writable overlays | Deferred M2 | world | reload | template read-only | changed-chunk memory |
| World persistence | Deferred M2+ | world | quiesce/reload | ephemeral state | I/O workers, cache |
| Vanilla terrain generation | Deferred | world | reload | import or simple generator | large CPU/RAM savings |
| Block placement/breaking | Deferred M2 | world | quiesce | player can look/point; edits ignored/rejected | event & mutation costs |
| Neighbor/block physics | Deferred | world | quiesce | edited blocks not physically propagated | huge tick budget depending on workload |
| Redstone | Deferred | world | quiesce | inert redstone states if permitted | scheduled updates |
| Fluid simulation | Deferred | world | quiesce | no flow or propagation | neighbor cascades |
| Random block ticks/crops | Deferred | world | quiesce | no growth / decay | idle periodic work |
| Mob AI/pathfinding | Deferred | world | quiesce | no AI workers/entities of that type | memory and pathfinding CPU |
| Passive spawn/despawn | Deferred | world | quiesce | no mob spawning | spawn checks |
| Weather/daylight simulation | Deferred | world | immediate/quiesce | static time/weather values only | periodic world tasks |
| Survival hunger/health | Deferred | player/world | reload | creative semantics only | player component & ticks |
| Inventory/container logic | Deferred | player/world | quiesce | protocol-visible minimal creative inventory only where needed | state replication/storage |
| Player collision/combat | Deferred | world | quiesce | no PvP hit processing | spatial index / events |
| Java sidecar | Deferred M2+ | global | restart | **no JVM process**, no IPC tasks | large JVM baseline + IPC |
| WASM plugins | Deferred | global/world | restart/quiesce | no runtime/compilation/cache | sandbox memory + fuel |
| Native Rust plugins | Deferred | global | restart | no dynamic loader | ABI/safety complexity |
| Extensive metrics/tracing | Optional | global | immediate | cheap mandatory counters only | cardinality and logging overhead |
| Autosave | Deferred | world | quiesce | no periodic disk work | I/O and dirty tracking |

## How to implement an 'OFF' contract (for future design, not executable code)
- Resolve a **declared dependency graph before initialization**. Failure gives descriptive error, not silent fallback.
- Allocate feature resources only through a scoped lifecycle owner. Each owner exposes task/callback/cache counters.
- On shutdown, first stop ingress, quiesce inflight work, revoke event subscriptions, flush if needed, then free memory.
- `UnsupportedFeature` should be explicit in both Rust and Java APIs. Client-required response paths must still behave correctly.
- A world parked with no players/timers should not have 20Hz world simulation wakeups, but a listening server still runs shared event loops/timers.
- Give compile-time stripping only to truly rare or costly dependencies; excessive Cargo cfg variants slow builds and testing.
- Do not allow Java plugins to load arbitrary heavy capabilities by side effect; require operator permission and explicit reconciliation.

## Profile truth table
- `minimal-creative`: always-on required items only; editable blocks OFF, persistence OFF, Java bridge OFF.
- `pvp-static`: minimal-creative, with combat optional later; no mob AI, random ticks, redstone or weather.
- `creative-builder`: block edits + optional storage + undo hooks; still no required survival simulation.
- `survival-experimental`: gated explicitly as NOT VANILLA PARITY until each mechanic is correctly implemented.

## Important semantics for client interaction
Disabling block mutation should produce **defined player-visible behavior** (interact denied, ghost block corrected, or instance read-only), not silently accept client assumptions. Disabling night progression still requires valid world-time packets. Creative clients still expect ability/teleport/position state. `Minecraft enabled features` registry negotiation is unrelated to this engine's optional gameplay modules.
