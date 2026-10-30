# 23 — Test stories / expected behavior (future, not executed)

## P0 (must pass before claiming server MVP)
**S01 Status:** clean stock 26.3 client sees status ping; mismatched version handled clearly; malicious length doesn't crash.

**S02 Real join:** online-mode verified legitimate stock 26.3 client reaches Play after complete configuration; wrong auth rejected; no insecure fallback.

**S03 World render:** client spawns above stone in an expected dimension, sees flat chunks all around, no void, wrong lighting or endless chunk load; movement between chunk boundaries correct.

**S04 Flight:** player has creative gamemode and correct flight capabilities; vertical and horizontal movement accepted within safety bounds; teleport confirmations handled.

**S05 Two-player presence:** two real clients in same world see accurate positions, rotations, names and disconnect cleanup; they don't see players isolated to another world.

**S06 Chat:** both can exchange human-readable system-relayed text, no fake cryptographic signing and no packet errors; limits and sanitization apply.

**S07 Two worlds:** independent world identifiers and subscriptions; player teleports/switches between worlds, client scene clears/resyncs; viewers in world A never see world B.

## P1 (first optimization differentiation)
**S08 Clone:** 100 identical empty/stone arenas share template allocations; record incremental memory and clone time; zero world simulation callback when truly parked.

**S09 Isolated modification:** edit one arena (future capability enabled); others remain unchanged. Reset revokes old generation, cancels work and discards overlays.

**S10 Churn:** continuously create/park/destroy instances and measure retained resident bytes and task count after quiescence. Every resource category has owner.

**S11 Feature OFF:** after disabling mob AI and Java bridge, no mob tick or Java process exists; capability inspection shows proper resolved dependencies.

**S12 Java control:** from documented Java SDK, launch/connect, import virtual template, create world and teleport player. Core still works when sidecar is killed.

## Negative/abuse stories
**S13** malformed compression or registry payload; **S14** fast reconnect/disconnect spam; **S15** spoofed proxy forwarding; **S16** client sends fly state without authorization; **S17** stale world ID after reset; **S18** 100k plugin edits in one request; **S19** arena destroyed during async chunk stream; **S20** disk quota or plugin restart halfway through persistence.

## How to report results
For each Sxx: date, commit hash, build flags, machine, Java client version, packet artifacts, expected observation, actual observation, failure details, attached profiler output and reviewer sign-off. All results currently **NOT RUN**.
