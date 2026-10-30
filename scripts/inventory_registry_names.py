#!/usr/bin/env python3
"""Inventory only registry/entry identifiers from the user-owned reference.

No JSON/NBT asset contents or implementation bodies are copied. Registry IDs are
defined by our sorted entry order and synchronized to clients by name. Omitted
data is legal only after the client acknowledges minecraft:core:26.3.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", default="unz")
    args = parser.parse_args()
    source = Path(args.reference)
    version = json.loads((source / "version.json").read_text())
    assert version["id"] == "26.3" and version["protocol_version"] == 777
    loader = source / "net/minecraft/resources/RegistryDataLoader.java"
    registries = source / "net/minecraft/core/registries/Registries.java"
    line = next(line for line in loader.read_text().splitlines() if "SYNCHRONIZED_REGISTRIES =" in line)
    keys = re.findall(r"Registries\.(\w+)", line)
    names = dict(re.findall(r'\b(\w+)\s*=\s*createRegistryKey\("([^"]+)"\)', registries.read_text()))
    inventory = []
    for key in keys:
        name = names[key]
        folder = source / "data/minecraft" / name
        assert folder.is_dir(), name
        entries = sorted("minecraft:" + p.relative_to(folder).as_posix()[:-5] for p in folder.rglob("*.json"))
        inventory.append({"registry": "minecraft:" + name, "entries": entries})
    output = Path("crates/sessions/data/registry_names.json")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(inventory, indent=2) + "\n")
    # Declare tag identifiers with intentionally empty server-owned membership.
    # M1B has no gameplay. The client still needs named tag bindings while loading
    # its own acknowledged core data. No proprietary membership data is copied.
    tag_root = source / "data/minecraft/tags"
    tag_names = []
    known_registries = sorted(set(names.values()), key=len, reverse=True)
    network_safe = {names[key] for key in keys} | {
        "block", "item", "entity_type", "fluid", "game_event", "point_of_interest_type", "potion"
    }
    grouped = {}
    for path in sorted(tag_root.rglob("*.json")):
        relative = path.relative_to(tag_root).as_posix()[:-5]
        registry = next((key for key in known_registries if relative.startswith(key + "/")), None)
        assert registry is not None, relative
        if registry not in network_safe:
            continue
        grouped.setdefault("minecraft:" + registry, []).append("minecraft:" + relative[len(registry) + 1:])
    for registry, entries in sorted(grouped.items()):
        tag_names.append({"registry": registry, "tags": sorted(entries)})
    tags_output = output.with_name("tag_names.json")
    tags_output.write_text(json.dumps(tag_names, indent=2) + "\n")
    evidence = {"version": "26.3", "protocol": 777, "registry_count": len(inventory),
                "entry_count": sum(len(r["entries"]) for r in inventory),
                "names_sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
                "tag_count": sum(len(r["tags"]) for r in tag_names),
                "tag_names_sha256": hashlib.sha256(tags_output.read_bytes()).hexdigest(),
                "tag_membership": "Empty by explicit M1B bootstrap policy. No vanilla gameplay or tag-membership parity claimed.",
                "method": "Path/name inventory only; no asset contents copied. Entry IDs follow sorted names transmitted to client.",
                "source_files": [{"path": str(p), "sha256": hashlib.sha256(p.read_bytes()).hexdigest()} for p in [loader, registries]]}
    Path("evidence/m1b-registry-inventory.json").write_text(json.dumps(evidence, indent=2) + "\n")
    print(json.dumps(evidence, indent=2))


if __name__ == "__main__":
    main()
