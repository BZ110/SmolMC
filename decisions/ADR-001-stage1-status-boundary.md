# ADR-001: Stage 1 is M1A; isolate protocol and connection ownership

Status: Accepted for the authorized Stage 1 implementation  
Date: 2026-10-29  
Scope: M1A — skeleton and status endpoint; no later-stage authorization

## Goal and context

Build the smallest functional server consistent with `docs/13_ROADMAP.md`.
The roadmap does not literally number “Stage 1”; its first implementation
stage is **M1A**, whose gate is stock-client server-list status and malformed
packet resilience, explicitly with no worlds. M1B is a separate login stage.
The implementation uses this narrow interpretation, stated to the user before
changes. The user subsequently confirmed status/ping and login rejection in
the client; no broader stage was requested.

## Invariants

- Network clients never acquire an authenticated identity or enter Play here.
- Protocol parsing has no sockets, world mutations or extension callbacks.
- One task owns one connection; one supervisor owns admission and task lifetime.
- Memory is bounded before frame allocation. Reads and writes share an absolute
  deadline; shutdown leaves no detached connection tasks.
- No unsafe Rust, global mutex, world tick, crypto implementation or JVM host.

## Source evidence

The roadmap and mandatory documents were read before changes. Exact wire fields
and registration order were inspected in the supplied reference, with hashes
recorded in `evidence/protocol-sources.json`. The independently fetched
PrismarineJS version table confirms 26.3 / 777 / data 5023. The stock-client gate
is user-observed; it is recorded separately from automated tests.
See `docs/STAGE1_PROTOCOL_EVIDENCE.md` for packet-by-packet evidence.

## Chosen design and trade-offs

Use three workspace crates: pure `smolmc-protocol-777`, connection runtime
`smolmc-network`, and thin executable `smolmc-server`. A single-thread Tokio
runtime runs bounded connection tasks. Each task awaits writes directly, so
there is no unbounded outbound channel. A small supervisor-owned fixed-window
limiter bounds accepted rates and IP bookkeeping without idle timers.

Alternatives considered: blocking thread per connection adds stacks and lifecycle
cost; a bespoke reactor adds unnecessary transport complexity; a single crate
blurs protocol/ownership boundaries; creating empty world/bridge crates or IPC
schemas now prematurely freezes future contracts. Those are unnecessary for M1A.
The three crates preserve places to attach an authenticated session layer, world
owner routing and typed Java sidecar commands later, without implementing them.
Connection IDs are local diagnostic IDs, never player UUIDs or world identities.

Reject Login/Transfer immediately with a protocol-correct login disconnect.
Do not parse usernames, attempt authentication or create dev-only player sessions.
Report zero player capacity honestly. Accept other versions for status discovery
so clients can see the target version; this is not multiversion gameplay support.

The 1 KiB inbound cap covers every valid Stage 1 packet including a maximum-size
hostname; it is lower than the wire ceiling and must be reconsidered before M1B.
Strict UTF-8, complete payload consumption and request-before-ping are deliberate
policies, not claims of byte-for-byte reference parity.

## Dependency impact

Runtime dependencies are Tokio (locked to 1.54.1) and serde_json (1.0.154), plus
their locked transitives. Only current-thread runtime, TCP/IO, timers, signals,
macros and sync are enabled; no runtime-multi-thread feature, crypto, HTTP, JVM,
world storage or game-data library. All project crates forbid unsafe Rust;
dependencies may use upstream unsafe internals. `Cargo.lock` pins the
resolved graph. The optional resource harness uses Python's standard library.

## Measurable acceptance tests

See `docs/STAGE1_REPORT.md`: 19 automated tests; 140 malformed network cases;
20,000 deterministic codec inputs; release build and warning-free Clippy;
real-client status/ping and readable rejection confirmed by the user; resource
run with 101 correct exchanges, 16 idle timeouts and cancellation at SIGTERM.
Every tested final snapshot has zero active connections.

## Compatibility and rollback

This is the initial implementation, so there are no prior binaries, persistence
formats or public extension APIs to migrate. The wire target is exclusively 777;
the status discovery exception does not accept other clients into gameplay.
Internal Rust interfaces are not a stable SDK promise. No data is persisted.

Rollback: stop the process with Ctrl-C/SIGTERM; remove or revert the new workspace
crates, Cargo files and test scripts. Keep the supplied reference and original
research intact. No world data, accounts, schema migrations or external service
configuration need recovery. Authorization remains limited to M1A even after a
rollback. Do not advance to M1B without the user's explicit authorization.

## Open questions

No blocker remains for M1A. Before a future authorized M1B, pin a redistributable
protocol/registry source, design maintained online-mode crypto/session verification,
and revise state-specific frame, authentication and configuration budgets. Before
production scaling, validate on Linux and conduct sustained admission/fault tests.

