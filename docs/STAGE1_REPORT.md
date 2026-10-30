# Stage 1 implementation and verification report

Date: 2026-10-29 (America/Toronto)  
Scope: **M1A only**, the first implementation stage of `13_ROADMAP.md`  
Result: **Implemented and validated. Stop before M1B.**

## Goal

Run a bounded Rust TCP endpoint that a stock 26.3 client can discover and ping.
Do not expand this task to the full M1 creative-world milestone. Root `AGENTS.md`,
the roadmap and the handoff now record the user's Stage 1-only authorization.
The project initially contained only the private extracted reference; research
documentation was copied from the supplied research pack into this workspace.
The original Downloads pack was left unchanged.

## Invariants and architectural decisions

Pure protocol codec → connection ownership/runtime → thin assembly root. No
network handler accesses a world or Java object. No authentication, configuration,
Play, worlds, survival systems, extension hosting or speculative public APIs.
Login is a terminal, readable rejection. There is one shared async runtime and
no detached connection task, per-world thread or tick loop. See
[ADR-001](../decisions/ADR-001-stage1-status-boundary.md) for alternatives,
dependency impact, compatibility and rollback.

## Source evidence

See [packet provenance](STAGE1_PROTOCOL_EVIDENCE.md) and the hash inventory in
[`evidence/protocol-sources.json`](../evidence/protocol-sources.json). Fields were
verified against targeted supplied reference classes, then exercised with
independent TCP fixtures and the stock 26.3 client. Public version metadata was
cross-checked; unavailable wiki/schema sources are explicitly recorded as gaps.
No proprietary source or assets were translated or copied into deliverables.

## Measurable acceptance and results

| Acceptance item | Evidence/result |
|---|---|
| Stock 26.3 server-list status and ping | **PASS, user-observed**: correct description, 0/0 players, ping bars |
| Readable unsupported login | **PASS, user-observed**, plus login/transfer and wrong-version TCP tests |
| Correct JSON/target version and exact 64-bit pong | **PASS**, independently encoded TCP integration tests |
| Fragmented and coalesced TCP frames | **PASS**, including all nonce sign/boundary values |
| No panic or listener poisoning on malformed input | **PASS**, 140 malformed TCP cases and 20,000 deterministic bounded codec inputs |
| Bounded allocation and protocol sequencing | **PASS**, length rejected before body allocation/read; UTF-8/UTF-16 limits, trailing bytes, bad IDs/intents, duplicate request, malformed ping |
| Connection/rate bounds and timeout recovery | **PASS**, cap, per-IP/global admission and absolute exchange deadlines |
| Cleanup after partial body, peer EOF and shutdown | **PASS**, zero active counters after drain; listener rebinds |
| Build and static checks | **PASS**, release build, formatting and Clippy with `-D warnings` |
| Resource/lifecycle smoke run | **PASS**, 101 successful status/ping exchanges, 16 timed-out idle connections, 4 open sockets cancelled at SIGTERM, zero active at exit |

There are **19 automated tests**: six codec tests, three runtime unit tests and
ten loopback integration tests. The malformed-input corpus is deterministic
smoke coverage, not a coverage-guided fuzz campaign or proof of DoS resistance.
Raw command output is in [`stage1-checks.txt`](../evidence/stage1-checks.txt).
Manual observations and their provenance are in
[`vanilla-client-check.json`](../evidence/vanilla-client-check.json).

Commands (normal Cargo can replace the project-local helper):

```sh
scripts/cargo-local.sh fmt --all -- --check
scripts/cargo-local.sh test --workspace --locked
scripts/cargo-local.sh clippy --workspace --all-targets --locked -- -D warnings
scripts/cargo-local.sh build --release --locked
python3 scripts/measure_stage1.py
```

The test client used installed, unmodified 26.3 binaries/libraries and an isolated
test game directory. A placeholder test profile sufficed for the server-list and
rejection check; no account tokens were read and no authenticated login was tested.
The Rust executable itself never starts a JVM. The Java process existed only as
a manually observed conformance client.

## Resource measurements

One short run on Apple M5 Pro, 18 logical CPUs, macOS 27.0.1 arm64; Rust 1.99.1;
release profile, default server limits, loopback, no gameplay. Git revision: N/A,
workspace was supplied without a Git repository. The 913,936-byte release binary
was measured by `scripts/measure_stage1.py`. Full values and methodology are in
[`stage1-resources.json`](../evidence/stage1-resources.json), with final counters
in [`stage1-resources.log`](../evidence/stage1-resources.log).

| Point | Resident set (KiB) | Cumulative process CPU (s) | OS threads |
|---|---:|---:|---:|
| Ready | 2368 | 0.00 | 1 |
| After 5.01 seconds idle | 2368 | 0.00 | 1 |
| After 100 paced exchanges | 2640 | 0.01 | 1 |
| 16 concurrent idle connections | 2640 | 0.01 | 1 |
| After those connections expired | 2640 | 0.01 | 1 |

The 100 paced exchanges took 6.44 seconds including deliberate pacing. Measured
end-to-end local exchange time (TCP connect, status, ping, EOF), driven by Python:
minimum 0.344 ms, median 0.471 ms, maximum 4.942 ms. These are smoke measurements,
not a production latency or throughput claim. SIGTERM cleanup took 1.21 ms in
this run; final counters: accepted 121, rejected 0, peak active 16, status/pong
101 each, idle timeouts 16, active 0, protocol/IO errors 0.

RSS snapshots are not peak RSS or heap accounting. The retained 272 KiB increase
does not prove either a leak or leak-freedom. CPU from `ps` is quantized to 0.01 s;
zero measured idle delta does not mean literally zero CPU. No statistical repeats,
comparative baseline, allocator analysis, wakeup counters, network-byte accounting,
8–24 hour soak, Linux run or gameplay resource measurement was performed. There
is no performance superiority claim and no numerical resource target asserted.

## Compatibility and rollback

Initial implementation: no prior public APIs, saved worlds or migrations.
Only protocol 777 is a supported target; status discovery for other versions is
an intentional diagnostic path. Internal Rust interfaces may evolve in future
authorized stages. Future worlds retain separate ownership/identity boundaries;
the Java sidecar remains optional and unimplemented.

Stop with Ctrl-C/SIGTERM. Revert/remove the workspace code, manifests and scripts
to roll back; there is no persistent state or external configuration to restore.
Keep the private reference and research documentation. Scope authorization
continues to prohibit M1B and later work without explicit user instruction.

## Known limitations and open questions

- **Status only.** No player can authenticate or enter the game. The advertised
  zero player capacity is deliberate. M1B–M5 remain unimplemented.
- Hard limits are Stage 1 defaults; gameplay needs a separately reviewed budget.
  Fixed one-second admission windows permit bursts near window boundaries and
  group clients behind a shared IP. This is bounded admission, not DDoS mitigation.
- IPv4 loopback is tested on macOS; IPv6 address parsing is available, but IPv6
  transport, Linux and Windows conformance have not been validated.
- No real golden packet trace or coverage-guided fuzzing; manual client evidence
  is user-reported rather than an agent-captured screenshot.
- Detailed connection logs are opt-in. Aggregate counters always appear at stop;
  there is no metrics service, persistent log collector or stable extension API.
- Before a future M1B: resolve online-authentication design, maintained crypto,
  redistributable registry provenance and broader state-specific input budgets.
  These are later-stage design work, not blockers for the completed M1A gate.

