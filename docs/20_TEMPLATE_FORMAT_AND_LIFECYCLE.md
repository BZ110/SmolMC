# 20 — Template file design, migrations, snapshots and world lifecycle

## Template boundary
`Template` = immutable logical map + dimension/biome identifiers + spawn + generator recipe + optional asset/registry references. `WorldInstance` = independent players/entities/events, overlay mutations, time settings, module profile and lifecycle. `WorldStorage` = optional persistence of overlay or snapshot. Never alias their identities.

## Template flavors
- **VirtualFlat:** parametric stone floor, configured height/span, air above; nothing stored per unmodified chunk; ideal M1.
- **ImportedVanilla:** region/level dat/structures recognized by a future importer after security/path validation; not M1.
- **CompiledTemplate:** preprocessed index of block section palettes, light/biome/heightmap data, hash/digest, per-chunk stream cache; future.
- **DerivedTemplate:** immutable snapshot of an edited instance promoted after validation. It becomes a new revision, never mutates the original template.

## Proposed metadata manifest fields (schema discussion, not file format commitment)
`formatVersion`, `templateId`, `revision`, `humanName`, `dimensionType`, `minY`, `height`, `spawn`, `registryVersion`, `sourceMinecraftVersion`, `generatorKind`, `defaultCapabilityProfile`, `chunkIndex`, `contentDigests`, `sizeLimitHints`, `bounds`.

Always define: compression; path-traversal protection; trusted vs untrusted files; block state registry compatibility; missing optional fields; strict version migration; checksum policy; deterministic ordering; max file size and max section count; how imports are cancelled; what happens to already-running arenas after template update.

## World state machine
`Declared` → `LoadingTemplate` → `Ready` → `Active` ↔ `Parked` → `Resetting` → `Ready` OR `Destroying` → `Destroyed`.

Safety transitions:
- Create allocates metadata and immutable refs; never mutate shared template content.
- First player joins: subscribe chunks then network spawn. Handle async chunk delivery.
- Last player leaves: drain or retain scheduled tasks based on profile. Park only if no due work.
- Reset: increment generation, evict/redirect players according to policy, cancel pending jobs, discard overlays, rebind template, clear scoped plugin data and visibility, send world reload/resync as required.
- Destroy: reject new API work, handle connections/teleports, wait for authorized I/O, drop refs and emit final event; stale handles error explicitly.
- Template updated: new instance gets latest revision only by explicit policy; existing ones remain pinned to old immutable revision.

## COW design candidate
Read order: instance overlay section → template immutable section → procedural default. Writes to unchanged shared section should allocate independent mutable copy or sparse delta, with explicit version counter. On threshold of modifications, promote sparse delta to dense palette. On reset, discard overlay by ownership scope, not by iterating and reverting every changed block. **Editable template data itself must never be changed**.

## World identity vs dimension identity
The Minecraft client sees a dimension type/key and world/level identifier in protocol packets. These **must not** be confused with server-internal arena identity. Multiple isolated arenas can be in the same dimension type. Teleport between instances may require respawn/dimension transition or carefully orchestrated unload/re-subscribe even when dimension type is unchanged; test with real client, don't assume sending one position packet is sufficient.

## Isolation checklist
No cross-instance visibility (player list/entity spawn), chunk subscriptions, chat broadcast except explicitly shared channels, mutable state, weather/time, tasks, plugin metadata, permission scopes or storage namespace. Shared template **data** is fine; shared mutable simulation state is not. Changing a shared template manifest while 100 worlds run must never change existing world revisions without policy.

## Template source safety and legal
Do not bundle proprietary decompiled Minecraft code/assets in a public template format implementation. Imported world files belong to users; validate sizes, filenames, compression bombs, malformed NBT, out-of-range block IDs and integrity. Document whether a build-time proprietary data generator is needed or external data can be freely redistributable.
