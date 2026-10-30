# 18 — Research bibliography, provenance and verification status

**Checked:** 2026-10-29. Citations are URLs to materials used for architectural reasoning; this is not permission to redistribute their code or data. Public claims may change. The uploaded archive is the authoritative *local reference for what the user provided*, not an automatically trusted implementation blueprint.

| ID | Source | What it substantiates | Limitations |
|---|---|---|---|
| SRC-01 | [Minecraft Wiki Java Edition protocol packets](https://minecraft.wiki/w/Java_Edition_protocol/Packets) | protocol phases, packet ID/names/layout current stable | wiki evolves; pin copy/version when implementing; CC BY-SA terms |
| SRC-02 | [Minecraft Wiki protocol versions](https://minecraft.wiki/w/Protocol_version) | current 26.3 / protocol 777 | snapshot IDs differ; may change on future release |
| SRC-03 | [PrismarineJS protocolVersions](https://github.com/PrismarineJS/minecraft-data/blob/master/data/pc/common/protocolVersions.json) | cross-check version 26.3, 777, data 5023 | community maintained; protocol details may lag |
| SRC-04 | [PrismarineJS Minecraft-data](https://github.com/PrismarineJS/minecraft-data) | cross-language registry/data extraction precedents | check licensing and coverage by release |
| SRC-05 | [Pumpkin repository](https://github.com/Pumpkin-MC/Pumpkin) | Rust vanilla-oriented server, optional features goals | under development |
| SRC-06 | [Pumpkin plugin architecture docs](https://github.com/Pumpkin-MC/Pumpkin-Docs/blob/master/docs/en/developer/plugins/index.md) | native and WASM plugin model | evolving design |
| SRC-07 | [FerrumC repository](https://github.com/ferrumc-rs/ferrumc) | Rust server competitor, creative alpha, rewrite | not a performance proof |
| SRC-08 | [Valence repository](https://github.com/valence-rs/valence) | Rust/Bevy framework; modular design | incomplete / version support must be checked |
| SRC-09 | [Minestom SharedInstance class](https://github.com/Minestom/Minestom/blob/master/src/main/java/net/minestom/server/instance/SharedInstance.java) | chunks shared with container, entities separate | this is not automatic isolated COW world semantics |
| SRC-10 | [Folia independent region overview](https://docs.papermc.io/folia/reference/overview/) | ownership and independent scheduling | complex vanilla mechanics not necessary for M1 |
| SRC-11 | [Paper Folia support docs](https://docs.papermc.io/paper/dev/folia-support/) | region/entity/global/async scheduler APIs | not a direct Rust implementation recipe |
| SRC-12 | [Paper plugin.yml guide](https://docs.papermc.io/paper/dev/plugin-yml/) | Bukkit-style plugin metadata and API-versioning | Bukkit plugin binary compatibility very different from SDK design |
| SRC-13 | [Paper plugin operation](https://docs.papermc.io/paper/dev/how-do-plugins-work/) | JVM plugin lifecycle and class loading | does not give Rust bridge |
| SRC-14 | [Velocity modern forwarding](https://docs.papermc.io/velocity/player-information-forwarding/) | proxy authentication forwarding and MAC | requires trust boundary/firewall |
| SRC-15 | [Velocity security](https://docs.papermc.io/velocity/security/) | forwarding does not replace backend isolation | threat-model guide |
| SRC-16 | [Tokio runtime docs](https://docs.rs/tokio/latest/tokio/runtime/) | fairness/bounded work and task scheduler behavior | no measured claim for this app |
| SRC-17 | [Minecraft EULA](https://www.minecraft.net/en-us/eula) | restrictions on redistributing Minecraft game/software content | legal review required, not legal advice |
| SRC-18 | [Minecraft usage guidelines](https://www.minecraft.net/en-us/usage-guidelines) | name, brand, assets and server commerce restrictions | legal review required |
| SRC-19 | [Paper adding plugins](https://docs.papermc.io/paper/adding-plugins/) | paper JAR packaging and plugin trust warning | per-plugin JVM isolation not promised |

## Source priority
1. User archive's `unz/version.json` for archived version identity only.
2. Exact released protocol source + verified live client traces for wire behavior.
3. Official Paper/Velocity docs for reference platform semantics.
4. Competitor source files pinned at a Git revision for comparison.
5. Community benchmarks for hypotheses **only**; do not present numbers as experimentally confirmed.

## Archive restrictions and verification
User-supplied `/mnt/data/unz.zip` was inventoried **locally**. The research pack includes **metadata and file paths only**, no Mojang Java source, JSON registry dump or binary NBT copied from the archive. Paths and descriptions are a navigational index for a later authorized developer. Do not commit the archive or generated extracted source to a public repository by default. Independent protocol specifications and clean-room implementations are preferred.
