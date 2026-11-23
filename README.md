# SmolMC

[![CI](https://github.com/BZ110/SmolMC/actions/workflows/ci.yml/badge.svg)](https://github.com/BZ110/SmolMC/actions/workflows/ci.yml)

SmolMC is an experimental Rust server for **Minecraft Java Edition 26.3
(protocol 777)**. It is designed for minigames and small arenas on modest
hardware. Many isolated worlds can run in one process, share immutable
terrain and keep their own edits. Parked instances have no world-owned
simulation loop.

[Roadmap](ROADMAP.md) · [What has been tested](docs/TESTED.md) ·
[Run the server](docs/RUNNING.md) · [Contribute](CONTRIBUTING.md)

## Current state

**M4B is implemented and user-accepted.** The milestone checkpoint is
`m4b-validated`.

- Online-mode login, creative flight over a stone world, multiplayer
  presence, and world-scoped system chat.
- Isolated arenas with shared templates, bounded edits, clone and reset.
  Imported Anvil terrain supports a bounded set of blocks, collisions,
  placement and held mining. The original save is read-only.
- Opt-in arena PvP with server-authoritative combat, death and respawn.
  Imported maps can opt into counted inventories, dropped items, supported
  loot, durability and armor.
- An optional Java SDK for managing arenas through local IPC. The Rust
  server runs on its own. [Java example](java/examples/TinyArena.java)

The supported subset is aimed at arenas. Crafting, enchanting, mobs,
persistent saves, general redstone and fluids, and Bukkit/Paper plugin
loading are deferred. [M4B engineering report](docs/M4B_ENGINEERING_REPORT.md)

## Quick start

Install [rustup](https://rustup.rs/) and your platform's OpenSSL development
libraries. On Debian or Ubuntu, install `build-essential pkg-config libssl-dev`.
On macOS, install `pkgconf openssl@3` with Homebrew. The repository pins
Rust 1.99.1.

```sh
cargo build --release --locked
target/release/smolmc
```

Add **127.0.0.1:25565** in a signed-in Minecraft Java Edition 26.3 client's
Multiplayer menu. See the [running guide](docs/RUNNING.md) for imported
terrain, PvP, inventory modes, Java and resource defaults.

## Testing and contributions

User testing covered stock-client login, flight, instance isolation, PvP,
imported terrain and the M4B interaction fixes. The M4B record also includes
Rust checks on macOS and Alpine Linux, Java integrations and independent
reference checks. [Testing record and measurement scope](docs/TESTED.md)

Send reproducible bugs and scoped proposals through
[GitHub Issues](https://github.com/BZ110/SmolMC/issues/new/choose). Report
security issues [privately](SECURITY.md). The project license is pending.
