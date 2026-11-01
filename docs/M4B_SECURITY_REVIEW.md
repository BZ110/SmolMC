# M4B security and failure review

2026-10-30. Read with ADR-011, the protocol ledger and the engineering report.

## Authority and identity

Client item predictions never supply server counts, components, equipment or
durability. Slot/action/window bounds are decoded exactly; valid stale-revision
clicks execute against current Rust state and receive a full resync. This follows
the actual vanilla handler and does not trust its hashed predicted stacks.
Invalid slots, crafting/unsupported drag actions and inappropriate armor placements
remain rejected. Existing ingress/admission/action budgets remain.

Directory membership/token/world ownership scopes actions and passive pickup.
Item IDs use a separate monotonic range; player allocation stops below it. UUIDs
derive from unique item identities. There is no client-provided pickup authority:
pickup requires a ready, living, active same-world member and exact local geometry.
Competing collectors serialize through one owner; the remaining count is the next
collector's input. Item count arithmetic is bounded by catalog maximums.

## Atomicity and reconnect

Reservations precede item-consuming drops/loot-producing terrain submissions.
Failed admission/lighting/generation/expected-state checks cannot consume an item,
wear a tool or create loot. Successful commits deliver an exactly-once receipt
through reserved directory capacity, independently of socket backpressure.
Placement consumption, tool wear and world drops follow that receipt. Deferred
operations are bounded and retain server-created mining facts; no client-supplied
elapsed time is accepted.

Disconnect marks the old game inactive before caching its inventory. Pending real
commits finish against that old state and publish the final inventory into its
bounded identity entry; reconnect cannot restore it while its receipt is pending.
Unsubmitted retries are canceled. Old lease tokens cannot create drops in a new
session/world. There is no eviction that grants a repaired kit. The 1025th distinct
item identity fails admission instead of growing the cache or evicting old state.

## Budgets and failure behavior

- Default 1024 item entities/world and 8192 shared; CLI ranges are 1..4096/world,
  up to 32768 shared, with shared at least the per-world limit. Pending loot counts.
- At most 256 item visibility edges/viewer; hidden candidates refill as edges free.
- Reliable payload/header charges are capped at 128 KiB/viewer and 8 MiB shared; leases release on completed writes, failed sends and disconnect. Scoped assembly scratch/allocator overhead are measured separately from these logical limits.
- Existing critical queue remains 32 groups. A combat group has at most 128 entries
  and 64 KiB; item delivery has at most 1024 packets and 64 KiB. Existing transport
  byte budgets, 64-packet/16-KiB writes and deadlines remain.
- Per-player pending interaction queue: 16. Delayed block records: 8. Lighting busy
  retries: at most 16 at 20ms spacing. One shared deadline mechanism; no idle sweep.
- Merge comparisons are bounded and spatial. Maximum stack sizes and compatible
  components govern merges; mass piles cannot allocate an unbounded map or queue.
- Item lifetime is five minutes while active; last departure drops the ephemeral
  item directory and reservations. Occupied reset/destroy remain rejected; empty
  reset increments generation and old pending terrain results cannot commit.
- Saturated viewers fail closed; reliable state is not silently dropped to hide
  resource exhaustion. Full inventories keep uncollected items, including partial
  remainders. Invalid interactions reconcile the authoritative state.

## Scope and remaining risks

Java snapshots are read-only, authenticated, schema-gated and identity-checked.
Old schema subsets remain negotiated; no Java veto or synchronous combat/mining
callback is introduced. Existing authentication, crypto and transport policies are
preserved. No unsafe Rust, JNI, bespoke cryptography or new external dependency.

This remains an ephemeral arena engine, without persistence, arbitrary item
components, enchantments, crafting/containers or full movement/survival parity.
Primitive oracle tests and short load cases are not a hostile-internet certification
or long soak. Exact final check results and user client acceptance belong in the
engineering report, rather than being inferred from this review.

Final audit (2026-10-31) also serialized disconnect's snapshot storage with pending
receipt finalization under the consistent game→cache lock order. This prevents a
departing session from overwriting newer committed inventory or restoring a stale
pending flag. Concurrent successful/failed receipt regressions exercise relog.
The shared identity cache is not acquired on normal live gameplay paths.
