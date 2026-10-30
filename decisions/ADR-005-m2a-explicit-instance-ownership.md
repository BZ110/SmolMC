# ADR-005: explicit instance ownership and bounded isolated edits

Date: 2026-10-29. Status: accepted by user following stock-client verification, supplementing automated gates.
Authority update: M2B only is now authorized; M2C and later remain unauthorized.
See `evidence/m2a-stock-client-user-verification.json` for exact user-reported scope.

## Goal

Complete M2A identities, lifecycle ownership, isolated edits, scoped subscriptions/
chat and world changes without replacing M1D transport or replication. The
[engineering report](../docs/M2A_ENGINEERING_REPORT.md) records acceptance,
measurements, limitations and rollback.

## Invariants

- Templates and the shared encoded stone body remain immutable.
- A production session owns one world membership and replication lease.
- World IDs never recycle in-process. Reset generations reject stale world
  operations; replication generations guard update/chat/leave.
- Occupied reset/destroy return `Occupied`; relocate or disconnect first. Pending
  switch requests do not pin destinations.
- No world task, OS thread, timer, simulation pass, persistence or JVM.
- M1D networking/auth bounds, dirty pacing, coalescing, reserved leave and bounded
  chat remain; aggregate fanout is bounded across worlds.
- Lifecycle locks never cover socket awaits, chunk encoding or ordinary movement.

## Source evidence

The unchanged roadmap gate is “two identical arenas diverge safely with edits.”
Ownership/lifecycle sources are `docs/02_ARCHITECTURE.md`, `05_WORLD_INSTANCES.md`,
`12_VALIDATION.md`, `20_TEMPLATE_FORMAT_AND_LIFECYCLE.md` and
`21_SECURITY_AND_TRUST.md`. [Protocol evidence](../docs/M2A_PROTOCOL_EVIDENCE.md)
separates existing wire layouts, local policies and observed fixtures. No packet
fields or performance figures are inferred from proposals.

## Design trade-offs

The pure manager owns immutable template descriptors and bounded sparse edits.
Synchronous virtual-template binding creates Ready instances without loading jobs.
A session RAII guard drives enter/leave counts, including cancellation. Reset
drops edits and advances generation; destroy retires identity and drops state.

A trusted control catalog serializes lifecycle/admin operations with one mutex.
Per-world immutable watch snapshots keep that global mutex off the Play hot path.
Each edit currently clones the bounded sparse map, and old readers may temporarily
retain prior snapshots. New subscriptions send shared base bytes then local
overrides. Live readers compare latest maps and emit subscribed changes/removals.
Buried-only edits preserve the static roof/heightmap/lighting assumptions. M2B
cache/overlay-promotion/feature-profile mechanisms are deferred.

One event-driven directory holds per-world membership/chunk indexes. Empty world
directories are removed after their own commands; movement does not scan unrelated
worlds. Ingress has one latest slot and one identity/world/generation notice per
lease. Chat also checks generation; a shared allowance prevents multiplying the
M1D aggregate fanout ceiling by instance count. No cross-index visibility/chat.

Trusted switch requests coalesce per session. After old ACKs drain, membership
transfers, the old replication lease drops, old peers/Tab/chunks clear, interest
resets and the target is joined as not-ready before position sync. Socket failure
closes the session and releases ownership. A target reset/destroyed before commit
causes safe closure; request acceptance does not promise switch completion.

Production `--instances N` creates up to 4096 identical arenas and routes joins
round-robin over live worlds. Trusted Rust Service controls provide edits/switch/
reset/destroy; there is no network management listener or new player command.
Same-dimension unload/re-subscribe passes the headless decoder; stock-client acceptance was subsequently supplied by the user.

## Dependency impact

No new crate, third-party dependency, Cargo feature, lockfile change, wire layout,
unsafe code, JVM or worker pool. Auth/network/protocol/chunk sources and workspace
manifests match final M1D hashes. The lifecycle benchmark is a separate example,
not a production control endpoint. Synthetic clients remain under `cfg(test)`.

## Measurable acceptance tests

Tests cover isolation/immutability, overwrite quota behavior, lifecycle/stale
handles, cancellation/snapshot reclamation, pending teleport/unsent chunks, slow
viewers, full-queue reserved leave, duplicate generations, 10,000-update coalescing,
four-client wire isolation/switch and all retained M1D regressions. The report
records checks and measurements through 4096 empty instances, lifecycle churn
and dense active Play. Limits are safety bounds, not capacity promises.

## Compatibility, rollback and open questions

Default one-world behavior retains M1D controls/terrain/chat; multiple worlds scope
chat and Tab/presence. Rust control signatures are internal M2A APIs, not a stable
SDK or promised Java examples. No saved-data migration.

After stopping this server, restore the verified M1D archive into a new directory;
use the report's hash, source inventory and check commands. No rollback was
performed. Ephemeral edits disappear on restart/rollback.

User-reported stock-client acceptance is recorded separately. Current Linux/
Windows builds, long soak, maximum edited-world retention, arbitrary templates/
surface edits and latency percentiles remain unverified. M2B only is now authorized; do not proceed to M2C or later.
