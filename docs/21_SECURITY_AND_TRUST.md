# 21 — Threat model: public server, isolated tenants and Java plugins

**M4B implementation update (2026-10-30):** explicitly configured native item
gameplay now adds counted/component inventory, bounded world-local drops,
durability and armor. The existing directory owns item/interaction ordering;
one shared deadline mechanism handles actual active work, with no inactive-world
scan. Predictive clicks reconcile against Rust state; commit receipts preserve
inventory effects across disconnect. See [M4B report](M4B_ENGINEERING_REPORT.md),
[ADR-011](../decisions/ADR-011-m4b-authoritative-items.md) and
[security/budgets](M4B_SECURITY_REVIEW.md). User-accepted and validated at `m4b-validated`; see the report for evidence and limits.


## Security boundaries
**Untrusted:** raw network clients, handshake hostname, player packets, user-imported templates, downloaded plugins, plugin IPC requests, externally accessible management endpoints. **Trusted only by configuration:** online Mojang session responses, explicitly authenticated Velocity proxy, operator-approved plugin signing/capabilities. In-process Rust native plugins share the core process trust boundary.

## Identity and sessions
Use Mojang online authentication for public direct clients. In proxied deployments, enable modern Velocity forwarding **only** with validated secret and a network/firewall boundary preventing backend access from arbitrary clients. A localhost-only service is not automatically safe on multi-tenant hosts. Authentication/compression/encryption negotiation requires strict protocol-state validation.

## Input handling mandatory safeguards
- Bound framing VarInt lengths, decompressed size and compression ratio, NBT nesting, string lengths, array counts and UTF-8 validation according to exact version.
- Timeouts for handshake/login/configuration, keepalive, dormant connections and authentication RPCs.
- Limit new connections and auth attempts per address/subnet, with consideration for trusted proxies and legitimate carrier NAT.
- Never let one large packet or slow client block a reactor/worker.
- Client movement, world edits, teleport, flying state and chat must be validated according to current permissions/profile.
- World-scoped coordinates, entity IDs and template IDs must be authorized; avoid ID reuse without generation stamps.
- Avoid sensitive logs: session credentials, full authentication tokens, personal chat logs without explicit retention policy, private proxy secrets.

## Isolation model
A single multi-world Rust process achieves *logical game-state separation*, **not memory/process isolation from a Rust crash or native plugin**. For hostile tenants requiring hard isolation, support one process per security trust domain and a supervisory multiprocess control plane later. The project's 100-world objective targets **trusted** games in one process.

## Java sidecar threat model
- Authenticated local IPC channel; socket filesystem permissions / equivalent Windows named pipe access control.
- Each Java plugin granted only declared capabilities (e.g. world create, specific world edits, messages, commands).
- Cross-process bridge errors must fail visibly and not poison world state.
- Multiple untrusted Java plugins sharing one JVM are not isolated from each other; optionally separate processes by trust domain.
- Strong content-length bounds, timeouts, nonce/correlation IDs and schema validation on IPC.

## Attack/fault cases to simulate
Malformed VarInt; zero/oversized compression; decompress bomb; invalid registry; forbidden packet transition; attack chat packet; spoofed forwarded UUID; multiple sessions sharing a UUID; flood chunk requests; repeated world create/destroy; stale handle mutating a recycled arena; Java sidecar freeze; malicious plugin sending 100k edits; invalid template/Anvil/NBT.

## Legal and branding
The user-provided archive appears to contain extracted game/server source and assets. It is a private reference; not bundled in this ZIP. Before publishing a Rust implementation, obtain qualified legal guidance on Minecraft EULA, redistributable generated registry metadata, attribution, reverse-engineering rules, project naming and trade dress. This is a risk note, not legal advice. References: https://www.minecraft.net/en-us/eula and https://www.minecraft.net/en-us/usage-guidelines .
