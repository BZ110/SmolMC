# What has been tested

The accepted gameplay checkpoint is `m4b-validated`. The stock-client entries
below come from user reports in the original project chats and the corresponding
engineering records. Automated and reference tests were recorded separately.

## Stock Minecraft Java Edition 26.3

| Milestone | User-reported observations |
| --- | --- |
| M1A–M1C | Server-list status worked, a signed-in client joined, stone rendered, and creative flight remained connected across more than 50 blocks. [M1C report](M1C_ENGINEERING_REPORT.md) |
| M1D | Two clients saw movement and crouching, exchanged system chat, and observed disconnect/reconnect cleanup. [M1D report](M1D_ENGINEERING_REPORT.md) |
| M2A | Two-world separation, relog behavior and the requested isolation checks passed. [Acceptance record](../evidence/m2a-stock-client-user-verification.json) |
| M3 | Two clients verified arena combat and accepted the classic knockback behavior, including consecutive hits. [Acceptance record](../evidence/m3-vanilla-client-acceptance.json) |
| M4A | The imported map rendered after a full-height correction. The user confirmed held pickaxe mining and the requested follow-up behavior. [Acceptance record](../evidence/m4a-vanilla-client-acceptance.json) |
| M4B | The user accepted the interaction retest after grass replacement, Netherite resistance, rapid building/mining and armor menu corrections. The final audit added a disconnect/inventory regression after the GUI retest. [Acceptance record](../evidence/m4b-vanilla-client-acceptance.json) |

## Automated and reference checks at M4B

- macOS and Alpine arm64 recorded **147 passing Rust test executions** in each
  workspace run, covering 141 distinct test bodies. Six opt-in tests were
  excluded from the default run. Both hosts also passed formatting, strict
  Clippy and release builds. [macOS manifest](../evidence/m4b-macos-meta.json),
  [Linux manifest](../evidence/m4b-linux-meta.json)
- Separate checks covered the Java SDK, four encrypted Play integrations and
  bridge/listener recovery. The final server also passed compatibility checks
  with SDK versions 0.1.0 through 0.3.0. [M4B report](M4B_ENGINEERING_REPORT.md)
- Independent checks against installed reference codecs and mechanics exercised
  item packets, 72 armor outputs, 12 knockback-resistance vectors, 16 terrain
  chunks, 48 mining cases and 72 collision cases. [Item reference](../evidence/m4b-reference.json),
  [terrain reference](../evidence/m4b-terrain-reference.json),
  [terrain mechanics](../evidence/m4b-terrain-mechanics.json)

## Short resource samples

The M4B report records one-thread idle samples of **10,688 KiB RSS** for the
flat world and **17,120 KiB RSS** for imported terrain. The synthetic
32-player, two-world armored item workload sampled **6.32% of one CPU core**
and **20,112 KiB server RSS**. The matched M4A-to-M4B samples saw higher sampled
CPU on several workloads. Active samples ran for roughly eight seconds. They
provide local regression evidence. Sustained capacity, long-soak behavior
and performance on low-cost hosted instances still need measurement.
[Methods and results](M4B_ENGINEERING_REPORT.md#short-resource-measurements)

## GitHub checks

[CI](../.github/workflows/ci.yml) runs the locked Rust workspace checks on
Ubuntu. [Extended checks](../.github/workflows/extended.yml) can run the release
build, SDK, encrypted Play fixtures and recovery checks on demand. Stock-client
testing, private reference inputs and capacity measurements require a separate
run. [GitHub Actions runs](https://github.com/BZ110/SmolMC/actions)
