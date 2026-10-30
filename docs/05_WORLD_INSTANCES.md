# 05 — World templates, instances, overlays, and isolation: main differentiator

**Current implementation:** M2B implements shared immutable virtual-stone descriptors, sparse chunk COW snapshots, clone/reset/reuse/park, scoped ownership and bounded profiles. Arbitrary imported/template files and general lighting remain future design. See [M2B evidence](M2B_ENGINEERING_REPORT.md). Root AGENTS.md controls authorization.

## Three distinct nouns
**Template**: content-addressed, logically immutable world content (chunks, metadata, reference palette). Loaded once or lazily into a shared cache. **Instance**: independently named live world with its own players, entity set, rules, subscriptions, time/config and overlay; can be unloaded without affecting any template or other instance. **Storage**: optional durability policy/backing store, not automatically implied by having an instance.

## World lifecycle state machine
`Declared → LoadingTemplate → Ready → Active ↔ Parked → Resetting/Destroying → Ready/Destroyed` with a failure path and cleanup on unsuccessful transitions. Transitions must be idempotent, observable, cancellation-aware. Players cannot enter not-ready instances. Destroy policy: relocate, disconnect, or reject if occupied. Registry of world IDs must not recycle IDs while outstanding commands exist.

## Snapshot / copy-on-write design
Let a base chunk be a shareable immutable object. New instance initially references it, storing only *diffs* after modifications. Reads consult overlay then base. Writes update a per-instance overlay; a chunk may be promoted to a private materialized copy if edit density crosses a measured threshold. **Never mutate the base**; no cross-instance contamination. A 'reset match' can discard the overlay rather than re-copy world files.

**Candidate overlay choices**:
1. Sparse map `(block_pos → BlockState)` — cheap for very few edits, random lookup/hash overhead for dense edits.
2. Per-chunk/per-section bitset + side array — more compact, complexity higher.
3. Full copied palette/section after threshold — better for heavy edit density, costs allocation.
4. Persistent chunk tree / immutable structure — elegant sharing, potentially high pointer overhead and copying cost.

Default design: base immutable, sparse section edits, promote after benchmarks. Keep overlay representation private so it can change without breaking SDKs. A 100-instance arena with 100 edited blocks each should not allocate 100 entire worlds; measure actual retained memory.

## Important distinction from Minestom's SharedInstance
Minestom documents shared instances whose writes can affect the parent and siblings. **Our proposed independent arena instances must isolate writes by default**. Sharing immutable template bytes is different from sharing mutable world state; make it impossible to conflate these API modes.

## Immutable template caching
- Key by `content_hash + registry_version + dimension_profile + encoding_variant`.
- Memory quotas on compressed and decoded template chunks; lazy loading.
- Reference-count pinning by active instances/players; unpinned caches evictable.
- Share **precompressed chunk packet representations** only when all per-player fields and world state allow identical bytes; differentiate by protocol version, registry and lighting state.
- Precompute only frequently used variants. Caching every compression threshold, height/lights combination can cost more RAM than it saves.
- Editable source world and imported Anvil are separate input pipelines; MVP stone template can be virtual (mathematical), avoiding chunk buffers for unloaded areas.

## World coordinate and geometry policies
- Per-world dimension settings: bounds, min/max Y, skylight, logical height, dimension ID.
- Chunk coordinates are signed; negative X/Z use **floor division**, not truncation.
- Unloaded chunk reads are explicit async/blocking-policy decisions; never synchronously disk-fetch on network threads.
- Entering an instance requires spawn chunk ready and players' interest subscriptions updated atomically.
- World switching must ensure chunk unload/respawn and dimension semantics are protocol-correct, even when two worlds share an identical biome/dimension configuration.

## Multi-tenancy and isolation policy
Each world has: world ID, template ID, lifecycle, player set, overlay, feature profile, quotas, scheduler handle, event namespace, optional storage handle, owner scope. **Resource quotas** include: max chunk interests, max overlay changes, max bytes retained, max entities, max scheduled jobs, max events/s, and per-player outbound queue budget. On violation degrade gracefully or evict; never take the entire process down through one malicious world command.

## Potential gains vs costs
**Likely gains**: fewer duplicate chunk allocations, cheap resets and cloning, no per-world OS thread, idle worlds do little work. **Costs**: overlay lookup indirection, cache accounting, world-switch complexity, memory reclamation and worse performance on heavily modified survival maps. **Conclusion**: do not claim superiority over ordinary mutable chunk containers without workload-specific benchmarking.

## Example lifecycle contracts (illustrative language only)
- `TemplateStore.import(source, options) -> TemplateId`
- `WorldManager.create(templateId, featureProfile, persistenceMode) -> WorldHandle`
- `WorldHandle.teleport(player, spawn)`
- `WorldHandle.reset()`, `WorldHandle.snapshot()`, `WorldHandle.destroy(policy)`
- `WorldMetrics.sharedBytes`, `overlayBytes`, `residentChunks`, `subscribers`, `idleWork`
These are **API sketches, not implemented methods**.

## Deep tests
A and B share initial content; change block in A, check B unchanged; destroy A, check B intact; spawn players at edge in A/B and ensure packets never cross; reset A, verify original base returned; compare overlay memory after 1, 100, 5,000 edits; multithread races under concurrent editing, cloning and destruction.

## Further references
Minestom: https://github.com/Minestom/Minestom/blob/master/src/main/java/net/minestom/server/instance/SharedInstance.java
Pumpkin world: https://github.com/Pumpkin-MC/Pumpkin
