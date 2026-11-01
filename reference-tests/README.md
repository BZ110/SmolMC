# Optional reference interoperability checks

These original adapters call operator-owned reference JARs. This repository
contains no Mojang/Paper JAR, implementation bodies or game assets. They are not
dependencies of Rust or the standalone Java SDK.

```sh
python3 scripts/check_protocol777_ids.py --reference unz
python3 scripts/check_reference_packets.py --minecraft-dir '/path/to/minecraft'
python3 scripts/check_classic1194_reference.py
python3 scripts/check_items777_reference.py
```

The first check pins identifier facts from the private source inventory, including
the clientbound bundle slot at ID 0. The second initializes the installed 26.3
client's actual codecs and registries in a separate JVM capped at 512 MiB. Original
Rust vectors must decode to expected packet classes, consume exactly and re-encode
byte-for-byte. Actual item/attribute IDs, F's changed action enum and semantic LP
velocity are checked. No account files, access tokens or launch credentials are read.

The classic check compares 16 yaw vectors and six velocity quantization vectors
with actual Paper 1.19.4 build 550 classes. It requires the private patched JAR and
libraries described in the script and `evidence/m3-classic1194-sources.json`.
Only patch-only mode was used; no Paper server/plugin host was launched. No Purpur.
These narrow gates do not establish full Paper physics or competitive-server parity.

`diagnostics/modern-26-3/` retains the historical flat-world motion investigation.
It executes actual primitive methods with explicit accepted-position restoration;
it does not instantiate the full ServerPlayer/network/render path. Its paired
landing observations explain a rejected intermediate profile. It is **not** the
current classic gameplay acceptance gate, and earlier incomplete 180-vector
comparisons are not claimed as current evidence.

The user separately tested two real clients and accepted the final profile. See
the M3 engineering/protocol reports for the deliberate ground-contact policy,
reference scope, failure history and validation limits.

The M4B item check invokes the installed 26.3 client's actual codecs, registries,
CombatRules, durability transitions and primitive knockback method. It compares
Rust vectors for component inventories, item spawn/metadata, equipment/attributes,
collection, movement/removal, 72 armor outputs and 12 resistance vectors. Original
`Items777Catalog.java` and the generators produce the bounded item/loot catalog
from private operator inputs. See `docs/M4B_PROTOCOL_EVIDENCE.md` for provenance,
commands and limits. This does not execute the full loot engine, server tick,
authentication or GUI; actual user client observations are separate evidence.
