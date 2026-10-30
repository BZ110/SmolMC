# M2B protocol and client regression evidence

Date: 2026-10-29. Target remains Minecraft 26.3 / protocol 777.
M2A is user-accepted; specific real-client observations are recorded in
[the acceptance record](../evidence/m2a-stock-client-user-verification.json).
No new account credentials, stock-client trace or visual recording was collected
for M2B. No new wire layout is introduced or inferred.

[Preservation hashes](../evidence/m2b-preservation.json) verify that auth, network,
protocol codecs, chunk encoding and registry/tag inventories are byte-identical
to Git tag `m2a-accepted`. Existing field/path evidence remains in the
[M1C](M1C_PROTOCOL_EVIDENCE.md), [M1D](M1D_PROTOCOL_EVIDENCE.md) and
[M2A](M2A_PROTOCOL_EVIDENCE.md) ledgers. This ledger describes changed data ownership
and observed fixtures, not new packet IDs or vanilla gameplay claims.

The new four-client test
`m1d_probe::m2b_cloned_worlds_replay_shared_edits_then_diverge_on_the_wire`
clones an already-edited world before joining two clients to each instance.
All four independently decode the initial Air correction at (8,62,8), while the
retained overlay charge remains shared. A two-position batch in A restores that
block to Stone and edits (-1,62,-17) to Air. Only A's two clients receive those
changes; B's two clients retain the original Air state and receive no A-only
negative-coordinate override. Closing clients and destroying both worlds returns
snapshot charges, connections, replicas, visible edges and frame leases to zero
without protocol errors, timeouts or task failures.

[Release wire output](../evidence/m2b-cloned-wire.txt) and complete
[macOS](../evidence/m2b-tests.txt) / [Linux](../evidence/m2b-linux-tests.txt)
regressions record passing results. The retained M2A fixture additionally verifies
live switching, unique teleport acknowledgements, removal of old peers/Tab/chunks,
25 target chunks, scoped system chat and reset/destroy/stale-handle rejection.

Fixtures use real TCP, AES-CFB8, compression, packet readers and actual Play
handlers, beginning at a private **post-authentication** test boundary with
synthetic identities. They do not simulate successful Mojang authentication,
Handshake/Configuration, stock rendering or real-account capacity. That path is
compiled only for tests and cannot be enabled by the production executable.
No authentication fallback, credential input, token logging or crypto changes.

The shared virtual stone body is still sent first, with only that world's sparse
corrections before batch completion. COW ownership and chunk-index lookup do not
change block update/batch/teleport formats. Buried Air/Stone bounds and the intact
y=63 roof preserve prior heightmap/light assumptions. No surface edits, lighting
simulation, inventory, survival or general template import is added.

Engine `--world-profile` selects trusted world capabilities; it does not alter
Minecraft Feature Flags or allow authentication to be disabled. A read-only
profile still sends all protocol-required terrain/control packets and corrects
unsupported client block actions. [Production CLI/status checks](../evidence/m2b-cli-check.json)
verify two read-only worlds, unsupported-profile rejection and clean shutdown.
