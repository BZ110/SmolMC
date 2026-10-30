# Stage 1 protocol evidence — Java Edition 26.3 / 777

Checked 2026-10-29. This covers only M1A and the terminal login rejection.
No reference source code or game assets are redistributed. File paths below
are relative to the supplied private `unz/`. SHA-256 pins of inspected files
and the installed client's official manifest/JAR SHA-1 are in
[`protocol-sources.json`](../evidence/protocol-sources.json).

## Observed local reference behavior

| Wire detail | Reference paths |
|---|---|
| 26.3; protocol 777; data version 5023 | `version.json` |
| Handshake ID 0; protocol VarInt, hostname String(255), unsigned big-endian port, intent VarInt | `network/protocol/handshake/HandshakeProtocols.java`, `ClientIntentionPacket.java` |
| Intents: status 1, login 2, transfer 3 | `network/protocol/handshake/ClientIntent.java` |
| Status serverbound request ID 0 (no fields), ping ID 1 | `network/protocol/status/StatusProtocols.java`, `ServerboundStatusRequestPacket.java` |
| Status clientbound response ID 0 (JSON string), pong ID 1 | `network/protocol/status/StatusProtocols.java`, `ClientboundStatusResponsePacket.java` |
| Ping/pong payload is one signed big-endian 64-bit value; exact echo | `network/protocol/ping/ServerboundPingRequestPacket.java`, `ClientboundPongResponsePacket.java` |
| Status JSON includes version, players, component description and secure-chat flag | `network/protocol/status/ServerStatus.java` |
| Duplicate status rejected, close after pong | `server/network/ServerStatusPacketListenerImpl.java` |
| Outer length at most 3 bytes, nonzero | `network/Varint21FrameDecoder.java` |
| String bounds use encoded byte count and decoded Java UTF-16 length | `network/Utf8String.java` |
| Packet IDs are registration-order indices, starting at zero | `network/codec/IdDispatchCodec.java`, `network/protocol/ProtocolInfoBuilder.java` |
| Login disconnect is clientbound ID 0, JSON component string | `network/protocol/login/LoginProtocols.java`, `ClientboundLoginDisconnectPacket.java` |

For compactness, `network/` in the table abbreviates `net/minecraft/network/`;
`server/` abbreviates `net/minecraft/server/`. Packet class filenames in the
same row use the directory of the preceding fully qualified class path.

## Independent public cross-checks and gaps

[PrismarineJS's maintained version table](https://github.com/PrismarineJS/minecraft-data/blob/master/data/pc/common/protocolVersions.json)
was fetched independently and confirms 26.3 → protocol 777 / data 5023. The fetched
bytes are hash-pinned in the evidence JSON; only the relevant version metadata
is retained. This corroborates version identity, not every packet field.

The research pack's
[Minecraft Wiki packet URL](https://minecraft.wiki/w/Java_Edition_protocol/Packets)
could not be fetched through the web tool (restricted URL). Guessed `26.3`
PrismarineJS schema URLs returned 404 and were **not** used as wire evidence.
Accordingly, field-level evidence comes from targeted local reference inspection,
independently encoded TCP fixtures and the exact-version stock client check.
There is no claim of two independent field-level specifications or a captured
golden vanilla packet trace.

## Implementation policy decisions

- Outer frames are further capped at 1024 bytes inbound. The legal maximum-size
  M1A handshake fits: hostname at most 765 bytes plus bounded VarInts and port.
- Packet VarInts use at most five bytes, with overflowing high bits rejected.
  Nonminimal encodings are accepted when within bounds.
- Hostnames are valid UTF-8, at most 765 bytes and 255 UTF-16 code units, then
  discarded. The local reference's Java decoding may replace invalid UTF-8;
  this implementation deliberately rejects it under the dossier's input policy.
- All payloads must be consumed exactly; malformed IDs, intent values, string
  sizes and trailing bytes close the connection. No status-state disconnect
  packet is invented.
- Require one status request before ping. The inspected reference also accepts
  ping before a status request; that permissive sequence is outside this slice.
- Keep the eight nonce bytes unchanged, including negative values. No timestamp
  interpretation or server-generated nonce is involved.
- Status accepts mismatched version numbers for discovery and always reports
  777. Login/transfer reject without creating a session; 777 gets a Stage 1 reason,
  other versions get an explicit target-version/Stage 1 reason.
- No Set Compression, encryption, configuration or Play packets are sent.
  Compressed/encrypted input has no negotiated state and cannot be accepted.
- Entire exchange deadline: five seconds. A peer that stops after status can
  close normally or be reclaimed by the deadline. Unknown and legacy formats
  close without entering a guessed legacy protocol.

## Executable evidence

`crates/protocol-777/src/lib.rs` includes a hand-authored handshake fixture and
boundary/corpus tests. `crates/network/tests/status.rs` independently encodes the
loopback client, validates JSON and exact pong bytes, and exercises fragmented and
coalesced delivery. It does not reuse the server codec to build requests.

The user confirmed stock 26.3 server-list display, ping bars and the readable
login rejection. See [`vanilla-client-check.json`](../evidence/vanilla-client-check.json)
and [`vanilla-server.log`](../evidence/vanilla-server.log). The agent could not
inspect Java's window through its UI tool; the observation is explicitly
user-reported. The installed client JAR was checked against its official manifest.

