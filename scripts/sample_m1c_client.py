#!/usr/bin/env python3
"""Attach macOS RSS/CPU/thread sampling to an already running client-test server.

Start the server separately, pass its PID, then perform the legitimate stock
client test. Stop the server to finish sampling. This omits any earlier peak.
"""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import time
from measure_stage1 import sample


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--binary", default="target/release/smolmc")
    parser.add_argument("--output", default="evidence/m1c-client-resources.json")
    args = parser.parse_args()
    if platform.system() != "Darwin" or args.pid <= 0:
        parser.error("requires macOS and a positive server PID")
    command = subprocess.check_output(
        ["ps", "-p", str(args.pid), "-o", "comm="], text=True).strip()
    if not command.endswith("/smolmc"):
        parser.error("PID must identify the SmolMC server")
    digest = hashlib.sha256(Path(args.binary).read_bytes()).hexdigest()
    started = time.monotonic()
    samples = []
    try:
        while True:
            try:
                samples.append({"elapsed_seconds": round(time.monotonic() - started, 3),
                                **sample(args.pid)})
            except (subprocess.CalledProcessError, IndexError):
                break
            time.sleep(0.2)
    except KeyboardInterrupt:
        pass
    record = {"timestamp_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "binary_sha256": digest,
              "sampling": "macOS ps approx every 0.2 seconds, attached to live client test; no heap or exact peak accounting",
              "samples": samples}
    if samples:
        record.update({"max_observed_rss_kib": max(s["rss_kib"] for s in samples),
                       "max_observed_threads": max(s["threads"] for s in samples)})
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(record, indent=2) + "\n")
    print(f"Saved {len(samples)} samples to {output}")


if __name__ == "__main__":
    main()
