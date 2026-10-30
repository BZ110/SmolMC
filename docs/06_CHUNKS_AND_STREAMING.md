# 06 — Chunks, palettes, network streaming and light

## A stone world is not a 'single packet'
The client expects valid world bootstrap and chunk encoding. Chunk sections contain block-state storage and biomes; chunk output may require heightmaps, lighting masks/arrays and block entity data structures, depending on the exact 26.3 specification. Incorrect registry IDs or bit-packed palettes cause disconnects or visual corruption. The primary deliverable is a well-tested encoder **not** a general-purpose generator.

## Stone generator (design)
A virtual template returning all-air above level `H`, stone below `H`, with one configurable spawn platform and expected surface height. Generate only chunks in view radius. For uniformly filled sections, a single palette value plus compact bit storage may be far cheaper than a dense 4096-block array, **as allowed by the version-specific format**. Light and heightmaps must remain consistent with material geometry.

## Data representations
- **Authoritative block state IDs**: registry-bound, versioned, not Java object references per block.
- **Sections**: palette + bit-packed indices, optional uniform singleton states.
- **Sparse edits**: per-instance diff (see world doc).
- **Chunk visibility**: each player has a set/ring of subscribed chunk coordinates.
- **Outbound chunk buffer**: immutable reference-counted bytes where legal to share.
- **Dirty update accumulator**: coalesce many changes into a minimum appropriate set of block/section/chunk updates.

## Outbound interest management
At login, establish view-center and bounded set. When crossing border, send only entering edge and unload leaving edge; avoid resend of whole render distance. Prioritize near/view-facing chunks if compatible with protocol. Apply per-player byte/packet budget and backpressure. Do not queue infinite payloads for slow clients. Keep compatibility with chunk batch acknowledgements and pacing/feedback protocol.

## Compression, encryption, caching
- Shared **uncompressed** chunk payload may be amortized across players in the same world and registry, subject to mutable state.
- Shared **compressed pre-encryption** data can save CPU if stream framing and compression thresholds are compatible; measure memory trade-offs.
- Session encryption, sequence/framing and per-player packet fields must be applied appropriately; never cache/send identical encrypted bytes across sessions.
- Prewarming all chunks on every instance creation defeats the memory goal. Prefer lazy materialization.

## Chunk edge / locality
If view-distance radius is R, naive full square has `(2R+1)^2` chunks. Increasing R grows bandwidth and working set roughly quadratically, not linearly, before protocol/client-specific distance shapes and optimizations. Eg R=4 → 81 positions; R=8 → 289; treat as geometric upper-square illustration, not observed client behavior.

## Spawn and lighting pitfalls
- Spawn must be a valid safe location; air and stone blocks encoded consistently.
- Explicitly test underground/above-ground lighting, day/night and neighboring chunk borders.
- Missing light data or wrong section range can yield black or flickering terrain.
- Heightmaps/biomes must match valid dimension configuration.
- World-switch must invalidate previous view state and never reuse stale client chunk subscriptions across worlds.

## Future heavy world concerns
Entities, block entities, scheduled ticks, redstone, pathfinding and fluids can create periodic CPU cost; each needs separate capability ownership. Saving/importing Anvil demands data-version handling and corruption recovery. Static templates do not imply correct survival semantics.

## Benchmark proposals
Chunk encode ns/chunk; bytes/chunk; compressed bytes/chunk; allocations/chunk; cache hit rate; peak pending outbound bytes; login first-chunk latency; viewer-move delta bytes; 100-instance shared base resident bytes; overlay promotion cost; memory after cleanup. Compare cold vs warm.
