# 04 — Minimum vertical slice: make it real, not feature-rich

## Definition of 'working'
A stock **26.3 Java Edition client** adds the server to its server list, connects with a legitimate account in online mode, loads a flat stone landscape, moves/flys in creative, sees another connected player move, sends a chat message visible to others, and switches to a separate world without data leakage. This is a *major* protocol milestone even though gameplay is tiny.

## Explicitly first-class
- One listening endpoint, bounded network event loop, status ping and graceful disconnect.
- Protocol-777 handshake, login, configuration and play state transitions.
- Registry/dimension data sufficient for correct unmodified client bootstrap.
- Minimal **static stone** chunk producer: e.g., uniform stone below a fixed height, air above, correct biome/light representation as required by protocol; no procedural terrain generator.
- Players spawn at safe above-ground coordinates; creative mode and flight granted.
- Keepalive, position sync/confirmation, movement to nearby observers.
- Simple chat relay using unambiguous system messages; no counterfeit secure chat.
- Chunk view radius small by default; send only newly subscribed chunks and unload unsubscribed chunks.
- At least 2 distinct world instances (same template allowed); changing a block in A never changes B.
- Server shutdown or world destroy releases all player references and caches.

## Deliberately **not** first-class
Block damage physics, inventory mechanics, creative item menus, mob AI, weather, block light propagation updates, recipes, crafting, combat calculations, redstone, advancements, survival hunger, loaded-world serialization, Java plugin execution. **Block placement** can be a dedicated next acceptance item rather than a hidden prerequisite to first connection.

## M1 demo scenario (manual)
1. Start fresh server, observe ready state and idle metric snapshot.
2. Vanilla client A pings, logs in and spawns on stone.
3. A double-jumps and flies freely, including crossing two chunk boundaries; no ghost chunks.
4. Vanilla client B logs in; A and B see each other, including turning and movement.
5. A sends a message; B sees a server-labeled relay; no fabricated trusted signature.
6. Operator moves A to instance `arena-b`; A sees new world/chunks and B does not follow.
7. Destroy `arena-b`; operator or lifecycle policy relocates/disconnects A cleanly, buffers freed.
8. Send 100 malformed/fuzzed packet cases in test harness; no panic or unbounded allocation.

## Technical acceptance gates
- Correct 26.3 protocol state transition, without custom client mods.
- Online mode supported and on by default for public-facing deployment; offline only for restricted development with a visible warning.
- No client soft-lock, invalid registry disconnect, or permanent void during expected login.
- Per-world isolation tested at API and network observation layers.
- Resource ownership tested: no persistent per-player entity after disconnect, no abandoned world jobs after destroy.
- Log output records session transitions, world creation/destruction and violations with correlation IDs.

## Why not implement more now?
Every added gameplay feature expands protocol surface, cache invalidation, state ownership, concurrency edge cases, and API semantics. Starting with the smallest vertical slice validates assumptions **before** building a large library atop them.
