# 12 — Verification, conformance, adversarial and fault-injection plan

## Five independent verification gates
1. **Wire correctness:** stable 26.3 client status/login/config/play and protocol 777 packet fixture checks.
2. **World correctness:** isolation, template immutability, spawn, view subscriptions, reset and destroy.
3. **Security:** online identity, bounded inputs, no unauthorized operations, IPC authentication.
4. **Resource correctness:** no leaks, bounded queues, cold/idle work, cleanup under failures.
5. **Compatibility/API:** Java SDK semantics, event deadlines, version error behavior, feature profile checks.

## High-priority test cases
| ID | Case | Assertion |
|---|---|---|
| P-001 | 26.3 status ping | valid response and ping return |
| P-002 | legitimate online login | reaches config without identity spoof |
| P-003 | registry negotiation | client joins play, no missing/invalid registry |
| P-004 | flat stone 3x3+ chunks | valid terrain/light/heightmaps visible |
| P-005 | fly beyond current view | new chunks load, old ones unload |
| P-006 | two players | mutual movement and orientation visibility |
| P-007 | simple message | both see correct system-label relay |
| P-008 | offline/unsupported client | explicit safe failure, no panic |
| W-001 | cloned template A/B | mutable overlay does not cross to sibling |
| W-002 | destroy occupied world | defined relocate/disconnect policy |
| W-003 | clone/reset 100 times | returned to base state and allocations reclaimed |
| W-004 | concurrent edits/destroy | no stale/mismatched handles or corruption |
| F-001 | ultra-pvp disabled mob AI | zero mob AI tasks/subscriptions/allocations |
| F-002 | invalid feature dependency | rejected before partial activation |
| F-003 | disable active feature | drains tasks and closes resources |
| J-001 | Java feature OFF | JVM and sidecar not started |
| J-002 | Java bridge kill | Rust server remains healthy |
| S-001 | malformed VarInt/length | bounded rejection |
| S-002 | compression bomb | bounded rejection, no OOM |
| S-003 | flood chunk/teleport requests | rate-limited and no starvation |

## Tools/procedures to design later
- Golden packet fixtures from a client running **the exact supported version** (sanitized).
- Property-based testing and fuzz targets: VarInt framing, NBT, packets, compressed frame lengths, palette bit packing, async lifecycle transitions.
- Model/property tests for COW overlay: `read(A,x)=base[x]` until local write; no visible sibling change.
- Long soak (8–24 hours) with world churn and reconnects; inspect memory growth.
- Race tests under constrained thread schedules and randomized task delays.
- Crash injection: disk full, server restart during save, Java bridge restart, malformed client disconnect.
- Negative permissions tests: Java plugin cannot access other tenant's world without declared capability.

## CI gate hierarchy (future)
Static lint/license scan → fast unit/property tests → protocol codec fixtures → integration with real vanilla 26.3 client/bot → profile matrix → dedicated soak/performance canary. No performance optimization merges unless baseline correctness still passes.

## Evidence requirements
Every test result includes version, host, exact command/config, output log, timestamps, failure reproduction or pass signal. A task is NOT complete merely because compiled code exists.
