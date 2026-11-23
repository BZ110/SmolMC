# SmolMC roadmap

SmolMC aims to run many small, isolated Minecraft Java Edition arenas in one
Rust process on modest hardware. Worlds share immutable terrain and store their
own edits in sparse copy-on-write overlays. Parked worlds have no world-owned
simulation loop. The Rust server runs independently of the optional Java SDK.
The supported wire target is **Java Edition 26.3, protocol 777**.

## Completed

| Milestone | Delivered |
| --- | --- |
| M0 | [Research, scope, and architecture](docs/00_PRODUCT_THESIS.md) |
| M1A–M1D | Online login, registry negotiation, stone terrain, creative flight, player presence and system chat. [M1D report](docs/M1D_ENGINEERING_REPORT.md) |
| M2A–M2B | Isolated instances, shared templates, bounded overlays, clone/reset and event-driven idle lifecycle. [M2B report](docs/M2B_ENGINEERING_REPORT.md) |
| Replication | Bounded write batching and less wake and allocation overhead. [Measurements](docs/REPLICATION_CPU_REPORT.md) |
| M2C | Optional local IPC bridge and Java SDK, with a runnable minigame example. [M2C report](docs/M2C_ENGINEERING_REPORT.md) |
| M3 | Authoritative arena PvP, arena kit, health, knockback, death and respawn. [M3 report](docs/M3_ENGINEERING_REPORT.md) |
| M4A | Bounded read-only Anvil import, collisions, supported placement and held mining. [M4A report](docs/M4A_ENGINEERING_REPORT.md) |
| M4B | Counted inventories, world-local dropped items, supported loot, equipment wear, armor and inventory interactions. [M4B report](docs/M4B_ENGINEERING_REPORT.md) |

**Current checkpoint** · `m4b-validated`. The [testing record](docs/TESTED.md)
covers stock-client observations, automated checks, reference checks and short
resource measurements.

## Open work

- Broaden verification. Make stock-client checks easier to repeat, add portable
  synthetic terrain fixtures where useful, and measure longer mixed workloads
  on modest Linux hosts. Capture resource use and cleanup after repeated
  reconnects and instance churn. [Validation plan](docs/12_VALIDATION.md)
- Improve developer and operator docs. Document supported maps and authoring
  limits, expand Java examples, and make resource and feature settings easier
  to discover. [Operations design](docs/26_CONFIG_OPERATIONS_AND_RESOURCE_BUDGETS.md)
- Resolve the project license before claiming open-source reuse rights or
  distributing release artifacts.

## Deferred proposals

The maintainer approves the next implementation milestone. These proposals
have no date or implementation commitment.

| Area | What a proposal needs to resolve |
| --- | --- |
| Commands, permissions, and plugin lifecycle | Define the supported Java API, access boundaries, failure handling and disabled-mode cost. |
| Broader gameplay and persistence | Scope crafting, containers, enchanting, mobs, fluids and redstone separately. Define a save format and a failure-recovery strategy before persistence work. |
| Scaling and compatibility | Use comparable workload measurements to justify parallel work. Treat additional client versions and any Bukkit/Paper adapter as separate compatibility projects. |

The [engineering roadmap](docs/13_ROADMAP.md) records the full stage order,
acceptance gates and rollback checkpoints. See [Contributing](CONTRIBUTING.md)
to propose a change.
