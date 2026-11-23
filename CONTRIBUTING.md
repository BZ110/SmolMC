# Contributing to SmolMC

Good first contributions include documentation fixes, reproducible
compatibility reports and small regressions covered by portable tests. Open a
[proposal](https://github.com/BZ110/SmolMC/issues/new?template=proposal.yml)
before implementing a protocol, gameplay, SDK or architecture change. The
[roadmap](ROADMAP.md) gives the current scope and deferred proposals. The
project license is pending.

## Build and check your change

Install Rust via [rustup](https://rustup.rs/) and the system OpenSSL
development libraries. The repository pins the Rust version.
[Platform setup](docs/RUNNING.md)

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

For Java bridge changes, install JDK 17 or newer, build the server with
`cargo build --release --locked` and run `scripts/java-check.sh`. The
[extended workflow](.github/workflows/extended.yml) lists the additional Java
and Unix recovery checks. Default checks run without a Minecraft install or
account.

## Open a pull request

1. Fork the repository and branch from `main`. Keep the change focused.
2. Include the issue number, the goal and invariants, source evidence, relevant
   trade-offs, dependency and resource impact, measurable acceptance tests
   and open questions. For a milestone, include backward compatibility and
   rollback information.
3. Report which checks ran and what they showed. For gameplay and wire changes,
   target protocol 777 and distinguish stock-client observations, reference
   behavior and automated fixtures. For performance, publish the workload,
   hardware, timings, units and a comparable baseline.
4. Keep credentials, account details, private saves, raw logs and proprietary
   code or assets out of commits and issues. Send vulnerability reports
   [privately](SECURITY.md).

[Engineering guidelines](AGENTS.md) · [Testing record](docs/TESTED.md)
