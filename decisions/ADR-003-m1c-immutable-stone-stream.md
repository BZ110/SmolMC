# ADR-003: Immutable M1C content and bounded player interest

Status: Implemented and validated within explicit M1C authorization  
Date: 2026-10-29  
Scope: M1C only. M1D and later milestones require explicit authorization.

## Goal

Render the roadmap's minimum stone world in an unmodified, normally signed-in
26.3 client and sustain creative flight across chunk boundaries. Replace M1B's
20-second empty probe without adding a simulation engine or instance manager.

## Invariants

- Preserve ADR-002's persistent transport, cancellation safety, frame leases,
  admission/deadline bounds and legitimate account authentication.
- Protocol serialization does not own player state; networking does not own
  terrain; sessions do not mutate world content.
- One immutable coordinate-independent chunk body and shared registry metadata
  serve all connections. No dense block arrays or resident per-coordinate chunks.
- Per-session interest has 25 fixed slots, one batch in flight, at most four
  chunks per batch and one reusable chunk serialization buffer. No growing queue.
- No world timer, per-world task/thread, global movement lock, background terrain
  generation, persistence, JVM, player replication or chat implementation.

## Source evidence

See [protocol ledger](../docs/M1C_PROTOCOL_EVIDENCE.md), the 57-file SHA-256
reference inventory and real-client records in `evidence/`. The first two client
attempts exposed incorrect nested bitset and teleport-ACK assumptions; corrected
code has exact fixtures and a successful client rendering/flight check. Local
reference facts and client observations are distinguished from design policies.
No proprietary implementation bodies or game data assets are redistributed.

## Design and trade-offs

`world` is a dependency-free immutable material/coordinate/view model. It does
not define a live WorldId, manager, overlay, entity owner or extension API.
`chunks` converts that model into protocol-777 terrain bytes. `protocol-777`
contains pure Play codecs. `sessions::player` owns only that connection's
position, controls, deadlines and sent subscriptions; the session driver performs
bounded I/O. Existing `network`, `auth` and runtime ownership remain intact.

The virtual world is stone at -64 <= y < 64, air elsewhere, overworld height
384, plains biome, frozen noon and creative spawn (8,65,8). A shared 35,956-byte
suffix contains 24 singleton sections, typed heightmaps and complete sky/block
light masks. Coordinate headers are prepended to a reused 35,965-byte scratch
buffer. Each send still copies the suffix and invokes bounded zlib framing;
this is not a zero-allocation transport. Repeated compression of this tiny
uniform body measured about 10.4–10.5 microseconds/chunk on the development host,
so no worker pool or compressed cache is introduced. This observation does not
justify arbitrary larger chunks on the single I/O executor.

Interest is a square radius two with near-first sends. Changing centers removes
departures, preserves overlap and computes missing chunks from the current view;
there is no stale work mailbox to drain. Batch acknowledgements pace transmission
within fixed limits. Each batch or unload reconciliation has a five-second
write deadline; batch/teleport acknowledgements have ten seconds. Keepalives
are issued after twelve seconds, with a twenty-second response deadline.
The loop sleeps until a connection deadline or inbound packet; a complete view
has no periodic chunk work. A small fixed-array scan is preferable to indexing
infrastructure at this radius; larger radii require measurement and redesign.

Movement is finite and bounded, and initial movement cannot move interest until
the full teleport acknowledgement matches. A session-local travel budget
(32-block burst, 64 blocks/second refill), horizontal bound 100,000 and y range
63.99–512 constrain induced work. These are M1C policies, not vanilla physics or
anti-cheat parity. Read-only block actions acknowledge and restore original
terrain within subscribed/reachable coordinates. Inventory editing and chat
remain unsupported. Opening/closing the inventory screen is tolerated.

Future world ownership must become an explicit separately authorized layer.
These private session structures are not a public API or a claim that an
authenticated session will own a future world. Geometry, packet conversion and
transport can remain separate when subscriptions gain a world identity.

## Dependency impact

Two small workspace crates; no new third-party runtime dependency or worker.
`flate2` is reused only as a new dev dependency for the chunk microbenchmark.
The locked third-party package graph is unchanged from M1B. Project Rust still
forbids unsafe code. Native TLS/OpenSSL and bounded RSA workers are unchanged.

## Measurable acceptance tests

53 tests, formatting, Clippy with warnings denied and a release build pass on
macOS arm64. Tests include complete geometry/light decoding, a literal bitset
fixture, truncation/nonfinite input, ACK matching/deadlines, negative coordinates,
fixed-capacity churn, encrypted/compressed Play streaming and real keepalive
response/cleanup. The stock client rendered stone, flew 50+ blocks and stayed
connected beyond a minute. Server counters independently show eight boundaries,
65 chunk sends, ten keepalive responses and no protocol failures/timeouts.

The existing descriptor-exhaustion regression passes on M1C. The resource run
handles 512 pending connections, 1,000 status reconnects, bounded authentication
failures and shutdown with zero connection/session/frame leases retained.
Measurements and limits are in [the engineering report](../docs/M1C_ENGINEERING_REPORT.md).

## Compatibility and rollback

M1A status/ping and M1B legitimate login/configuration remain available. Play now
uses creative mode and persistent read-only terrain rather than adventure-mode
empty probe/20-second close. No stored data, database schema or Java API changes.
Stop the process and restore the pre-M1C workspace/binary as described in the
report; there is no state migration.

## Open questions

No many-authenticated-player capacity, thousands-of-instances, long soak,
coverage-guided fuzzing or M1C Linux/Windows validation is claimed. Faithful tag
memberships/full registry fallback remain M1B limitations. Future multiplayer
visibility, world ownership and editing each need their own authorized design;
this milestone provides no implicit permission to implement them.
