#!/usr/bin/env python3
"""Inventory protocol registration names/IDs; never copy implementation bodies."""
import argparse
import json
from pathlib import Path
import re


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", default="unz")
    args = parser.parse_args()
    root = Path(args.reference)
    version = json.loads((root / "version.json").read_text())
    assert version["id"] == "26.3" and version["protocol_version"] == 777
    result = {}
    for phase, stem in [("login", "Login"), ("configuration", "Configuration"), ("game", "Game")]:
        source = (root / "net/minecraft/network/protocol" / phase / (stem + "Protocols.java")).read_text()
        for direction in ["SERVERBOUND", "CLIENTBOUND"]:
            start = source.index(direction + "_TEMPLATE =")
            section = source[start:source.index("\n    });", start)]
            names = re.findall(r"\.(?:addPacket|withBundlePacket)\(\w+PacketTypes\.(\w+)", section)
            assert names and len(set(names)) == len(names)
            result[phase + "/" + direction.lower()] = {name: index for index, name in enumerate(names)}
    Path("evidence/m1b-packet-ids.json").write_text(json.dumps(result, indent=2) + "\n")
    print("Wrote registration-order identifier inventory for protocol 777.")


if __name__ == "__main__":
    main()
