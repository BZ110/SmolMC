#!/usr/bin/env python3
"""macOS separate-process, private-test-binary active Play workload.

Requires a release smolmc-sessions test executable, never the production server.
Synthetic post-auth fixtures exercise encryption, chunks, visibility, pose,
movement and system chat. Authentication cost/capacity is NOT measured.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import time
from measure_stage1 import sample


def wait_file(path, process, timeout):
    end = time.monotonic() + timeout
    while not path.exists():
        if process.poll() is not None:
            raise RuntimeError(f"probe exited before {path}; see its log")
        if time.monotonic() >= end:
            raise TimeoutError(str(path))
        time.sleep(0.05)


def run(executable, root, count, seconds):
    prefix = root / f"players-{count}"
    ready = prefix.with_suffix(".ready")
    client_ready = ready.with_suffix(".clients")
    for path in [ready, client_ready]:
        path.unlink(missing_ok=True)
    env = dict(os.environ, SMOLMC_PROBE_READY=str(ready),
               SMOLMC_PROBE_REPORT=str(prefix.with_suffix(".server.json")),
               SMOLMC_PROBE_ROLE="server")
    command = [str(executable), "m1d_probe::active_play_resource_probe", "--ignored", "--exact", "--nocapture"]
    with prefix.with_suffix(".server.log").open("w") as server_log, prefix.with_suffix(".client.log").open("w") as client_log:
        server = subprocess.Popen(command, env=env, stdout=server_log, stderr=subprocess.STDOUT)
        client = None
        snapshots = []
        try:
            wait_file(ready, server, 30)
            cold = sample(server.pid)
            env.update(SMOLMC_PROBE_ROLE="client", SMOLMC_PROBE_PLAYERS=str(count),
                       SMOLMC_PROBE_SECONDS=str(seconds),
                       SMOLMC_PROBE_REPORT=str(prefix.with_suffix(".client.json")))
            client = subprocess.Popen(command, env=env, stdout=client_log, stderr=subprocess.STDOUT)
            wait_file(client_ready, client, 180)
            start = time.monotonic()
            while client.poll() is None:
                snapshots.append({"elapsed_seconds": round(time.monotonic() - start, 3), **sample(server.pid)})
                time.sleep(0.2)
            time.sleep(3)  # observe leave cleanup; no RSA jobs were started
            recovered = sample(server.pid)
            server.send_signal(signal.SIGTERM)
            stopped, status, usage = os.wait4(server.pid, 0)
            assert stopped == server.pid
            server.returncode = os.waitstatus_to_exitcode(status)
            assert server.returncode == 0
            counters = json.loads(prefix.with_suffix(".server.json").read_text())
            assert "active_replicas: 0" in counters["sessions"] and "visible_edges: 0" in counters["sessions"]
            assert "frame_bytes_retained: 0" in counters["network"]
            assert "task_failures: 0" in counters["network"]
            client_record = json.loads(prefix.with_suffix(".client.json").read_text())
            def number(text, key):
                return int(re.search(r"\b" + key + r": (\d+)", text).group(1))
            all_healthy = (client.returncode == 0 and client_record.get("closed_clients", 0) == 0
                           and number(counters["network"], "peak_active") == count
                           and number(counters["sessions"], "peak_visible_edges") == count * (count - 1)
                           and all(number(counters["network"], key) == 0 for key in ["protocol_errors", "timeouts", "task_failures", "io_errors"]))
            result = {"result": "PASS" if all_healthy else "FAIL", "players": count, "seconds_after_all_spawned": seconds,
                      "cold": cold, "samples": snapshots, "recovered": recovered,
                      "process_usage": {"peak_rss_kib": usage.ru_maxrss / 1024,
                                        "user_cpu_seconds": usage.ru_utime,
                                        "system_cpu_seconds": usage.ru_stime},
                      "server": counters,
                      "client": client_record}
            if len(snapshots) >= 2:
                wall = snapshots[-1]["elapsed_seconds"] - snapshots[0]["elapsed_seconds"]
                cpu = snapshots[-1]["cpu_seconds"] - snapshots[0]["cpu_seconds"]
                result["active_cpu_one_core_percent"] = cpu / wall * 100
                result["max_sampled_rss_kib"] = max(s["rss_kib"] for s in snapshots)
                result["max_sampled_threads"] = max(s["threads"] for s in snapshots)
            return result
        finally:
            for process in [client, server]:
                if process and process.poll() is None:
                    process.kill()
                    process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", required=True)
    parser.add_argument("--players", default="1,32,128,256")
    parser.add_argument("--seconds", type=int, default=20)
    parser.add_argument("--output", default="evidence/m1d-active-play-resources.json")
    args = parser.parse_args()
    if platform.system() != "Darwin":
        parser.error("uses macOS ps and wait4 RSS units")
    executable = Path(args.executable).resolve()
    output = Path(args.output)
    raw = output.parent / (output.stem + "-raw")
    raw.mkdir(parents=True, exist_ok=True)
    record = {"timestamp_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "host": platform.platform(), "raw_directory":str(raw), "test_executable": str(executable),
              "test_executable_sha256": hashlib.sha256(executable.read_bytes()).hexdigest(),
              "workload": {"players": "synthetic post-authentication Play fixtures, NOT real-account login capacity",
                           "topology": "dense: all players in the same chunk, every other player visible",
                           "movement": "20 packets/s/client, 2-block circular path; yaw 3 degrees/update",
                           "pose": "crouch toggles once/second",
                           "chat": "one sender every two seconds, server-labeled relay to all",
                           "transport": "TCP loopback, AES-CFB8, compression threshold 256; real chunk/batch/keepalive handlers",
                           "processes": "separate Rust server/client test processes; sample only server PID",
                           "server_runtime": "same single-thread async runtime; test harness overhead included"},
              "limitations": ["No external authentication load or production capacity claim.",
                              "Single shared macOS host; ps CPU resolution 0.01 seconds.",
                              "No long soak, heterogeneous terrain or world-instance workload."],
              "runs": []}
    for count in map(int, args.players.split(",")):
        print(f"Starting {count} active Play fixtures", flush=True)
        record["runs"].append(run(executable, raw, count, args.seconds))
        output.write_text(json.dumps(record, indent=2) + "\n")
        print(f"Completed {count}: CPU {record['runs'][-1].get('active_cpu_one_core_percent', 0):.1f}%", flush=True)


if __name__ == "__main__":
    main()
