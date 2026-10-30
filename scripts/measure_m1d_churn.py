#!/usr/bin/env python3
"""Repeated active Play batches against ONE private test-server process on macOS.

Uses the same post-auth fixtures as measure_m1d_active.py, not account auth.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import time
from measure_stage1 import sample
from measure_m1d_active import wait_file


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", required=True)
    parser.add_argument("--players", type=int, default=32)
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--seconds", type=int, default=3)
    parser.add_argument("--output", default="evidence/m1d-active-churn.json")
    args = parser.parse_args()
    if platform.system() != "Darwin":
        parser.error("macOS ps and wait4 accounting required")
    exe = Path(args.executable).resolve()
    output = Path(args.output)
    root = output.parent / (output.stem + "-raw")
    root.mkdir(parents=True, exist_ok=True)
    ready = root / "server.ready"
    ready.unlink(missing_ok=True)
    command = [str(exe), "m1d_probe::active_play_resource_probe", "--ignored", "--exact", "--nocapture"]
    env = dict(os.environ, SMOLMC_PROBE_READY=str(ready), SMOLMC_PROBE_ROLE="server",
               SMOLMC_PROBE_REPORT=str(root / "server.json"))
    report = {"timestamp_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "host": platform.platform(), "test_executable_sha256": hashlib.sha256(exe.read_bytes()).hexdigest(),
              "workload": {"players_per_round": args.players, "rounds": args.rounds,
                           "seconds_after_spawn_per_round": args.seconds,
                           "server": "one long-lived private test process; no Login/authentication calls",
                           "activity": "20 Hz circular movement/yaw, 1 Hz crouch, one chat sender every 2 seconds; dense mutual visibility",
                           "cleanup": "all clients close, then 1 second before next batch"}, "rounds": []}
    server = None
    try:
        with (root / "server.log").open("w") as log:
            server = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT)
            wait_file(ready, server, 30)
            report["cold"] = sample(server.pid)
            for index in range(args.rounds):
                client_ready = ready.with_suffix(".clients")
                client_ready.unlink(missing_ok=True)
                client_report = root / f"round-{index+1}.json"
                env.update(SMOLMC_PROBE_ROLE="client", SMOLMC_PROBE_PLAYERS=str(args.players),
                           SMOLMC_PROBE_SECONDS=str(args.seconds), SMOLMC_PROBE_REPORT=str(client_report))
                before = sample(server.pid)
                with (root / f"round-{index+1}.log").open("w") as client_log:
                    client = subprocess.Popen(command, env=env, stdout=client_log, stderr=subprocess.STDOUT)
                    samples = []
                    try:
                        wait_file(client_ready, client, 60)
                        while client.poll() is None:
                            samples.append(sample(server.pid))
                            time.sleep(0.2)
                        assert client.returncode == 0
                    finally:
                        if client.poll() is None:
                            client.kill()
                            client.wait()
                result = json.loads(client_report.read_text())
                assert result["closed_clients"] == 0
                time.sleep(1)
                report["rounds"].append({"round": index+1, "before": before,
                                         "active_samples": samples, "after_cleanup": sample(server.pid),
                                         "client": result})
                output.write_text(json.dumps(report, indent=2) + "\n")
                print(f"Completed active churn round {index+1}", flush=True)
            server.send_signal(signal.SIGTERM)
            _, status, usage = os.wait4(server.pid, 0)
            server.returncode = os.waitstatus_to_exitcode(status)
            assert server.returncode == 0
            report["server"] = json.loads((root / "server.json").read_text())
            for text in ["active_replicas: 0", "visible_edges: 0"]:
                assert text in report["server"]["sessions"]
            for text in ["active: 0", "frame_bytes_retained: 0", "protocol_errors: 0", "timeouts: 0", "task_failures: 0"]:
                assert text in report["server"]["network"]
            report["process_usage"] = {"peak_rss_kib": usage.ru_maxrss / 1024,
                                       "user_cpu_seconds": usage.ru_utime, "system_cpu_seconds": usage.ru_stime}
            report["limitations"] = ["Synthetic Play fixtures, not repeated legitimate account authentication.",
                                     "Separate client/server processes; only server sampled. Test-harness overhead included.",
                                     "Short shared-host churn run; no allocator live-byte accounting or long-soak claim."]
            report["result"] = "PASS"
            output.write_text(json.dumps(report, indent=2) + "\n")
    finally:
        if server and server.poll() is None:
            server.kill()
            server.wait()


if __name__ == "__main__":
    main()
