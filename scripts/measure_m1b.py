#!/usr/bin/env python3
"""macOS M1B process profile: status churn, partial frames, bounded auth work.

No legitimate-account simulation: synthetic logins stop at the encryption
challenge or submit invalid ciphertext. No session-service requests are sent.
The real-client authenticated Play check is recorded separately.
"""
import argparse
import datetime
import json
import os
from pathlib import Path
import platform
import re
import signal
import socket
import subprocess
import time
from measure_stage1 import frame, packet, ping, sample, varint


def challenge(address):
    sock = socket.create_connection(address, timeout=3)
    hello = b"\x00\x89\x06\x09localhost\x63\xdd\x02"
    start = b"\x00\x09LoadProbe" + bytes(16)
    sock.sendall(frame(hello) + frame(start))
    response = packet(sock)
    assert response[0] == 1, "expected encryption request"
    assert response[-1] == 1, "server must require legitimate authentication"
    return sock


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="target/release/smolmc")
    parser.add_argument("--output", default="evidence/m1b-resources.json")
    args = parser.parse_args()
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    if platform.system() != "Darwin":
        parser.error("this sampler uses macOS ps; run listener regression/tests on Linux")
    binary = Path(args.binary).resolve()
    command = [str(binary), "--bind", "127.0.0.1:0", "--max-connections", "512",
               "--connection-rate", "4096", "--per-ip-rate", "4096"]
    report = {"timestamp_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "host": platform.platform(), "machine": platform.machine(),
              "cpu_model": subprocess.check_output(["sysctl", "-n", "machdep.cpu.brand_string"], text=True).strip(),
              "logical_cpus": os.cpu_count(), "binary_bytes": binary.stat().st_size,
              "command": command, "samples": {}, "reconnect_batches": [],
              "note": "Release; loopback; admission rates raised explicitly for a single-IP load generator. No world or gameplay load."}
    server = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    sockets = []
    try:
        started = time.monotonic()
        ready = server.stderr.readline().strip()
        assert ready.startswith("ready "), ready
        report["startup_to_ready_seconds"] = time.monotonic() - started
        host, port = re.search(r"address=(\S+)", ready).group(1).rsplit(":", 1)
        address = host, int(port)
        report["samples"]["ready"] = sample(server.pid)
        time.sleep(5)
        report["samples"]["idle_5s"] = sample(server.pid)
        for count in [128, 512]:
            for _ in range(count):
                sock = socket.create_connection(address, timeout=3)
                sock.sendall(b"\x80\x08\x00")  # partial 1024-byte handshake body
                sockets.append(sock)
                time.sleep(0.001)  # Pace establishment; measure live sockets, not backlog overflow.
            time.sleep(0.05)
            report["samples"][f"{count}_partial_connections"] = sample(server.pid)
            for sock in sockets:
                sock.close()
            sockets.clear()
            time.sleep(0.2)
            ping(address)
            report["samples"][f"after_{count}_disconnects"] = sample(server.pid)
        all_times = []
        for batch in range(5):
            before = sample(server.pid)
            times = []
            for _ in range(200):
                start = time.monotonic()
                ping(address)
                times.append((time.monotonic() - start) * 1000)
                time.sleep(0.002)
            all_times.extend(times)
            report["reconnect_batches"].append({"batch": batch + 1, "passed": len(times),
                                               "before": before, "after": sample(server.pid)})
        # Hold every authentication admission, then prove the ninth is rejected.
        for _ in range(8):
            sockets.append(challenge(address))
        report["samples"]["8_pending_auth_challenges"] = sample(server.pid)
        ninth = socket.create_connection(address, timeout=2)
        ninth.sendall(frame(b"\x00\x89\x06\x09localhost\x63\xdd\x02") + frame(b"\x00\x09LoadProbe" + bytes(16)))
        try:
            assert ninth.recv(1) == b"", "auth admission limit not enforced"
        except ConnectionResetError:
            pass
        ninth.close()
        # Invalid ciphertext reaches the real bounded RSA worker path, not HTTPS.
        invalid_key = b"\x01" + varint(128) + bytes(128) + varint(128) + bytes(128)
        for sock in sockets:
            sock.sendall(frame(invalid_key))
        worker_samples = []
        for _ in range(10):
            worker_samples.append(sample(server.pid))
            time.sleep(0.01)
        report["rsa_worker_samples"] = worker_samples
        for sock in sockets:
            try:
                assert sock.recv(1) == b""
            except ConnectionResetError:
                pass
            sock.close()
        sockets.clear()
        time.sleep(0.2)
        ping(address)
        report["samples"]["after_invalid_auth_recovery"] = sample(server.pid)
        time.sleep(5)
        report["samples"]["final_idle_after_worker_retirement"] = sample(server.pid)
        # Include a true deadline case and shutdown with unfinished connections.
        idle = socket.create_connection(address, timeout=7)
        assert idle.recv(1) == b""
        idle.close()
        for _ in range(4):
            sockets.append(socket.create_connection(address, timeout=2))
        time.sleep(0.05)
        start = time.monotonic()
        server.send_signal(signal.SIGTERM)
        # wait4 attributes peak RSS and exact user/system CPU to this server PID,
        # excluding the Python load generator and ps sampling subprocesses.
        stop_deadline = time.monotonic() + 3
        while True:
            stopped, status, usage = os.wait4(server.pid, os.WNOHANG)
            if stopped:
                break
            if time.monotonic() >= stop_deadline:
                raise TimeoutError("server did not stop within three seconds")
            time.sleep(0.001)
        server.returncode = os.waitstatus_to_exitcode(status)
        tail = server.stderr.read()
        report["shutdown_seconds"] = time.monotonic() - start
        report["process_usage"] = {"peak_rss_kib": usage.ru_maxrss / 1024,
                                   "user_cpu_seconds": usage.ru_utime,
                                   "system_cpu_seconds": usage.ru_stime,
                                   "voluntary_context_switches": usage.ru_nvcsw,
                                   "involuntary_context_switches": usage.ru_nivcsw,
                                   "note": "macOS wait4 for server PID; ru_maxrss is bytes on macOS. Snapshot RSS and OS high-water RSS are different accounting views."}
        assert server.returncode == 0, tail
        assert "active: 0" in tail and "active_sessions: 0" in tail and "frame_bytes_retained: 0" in tail
        assert "peak_active: 512" in tail, "connection load did not reach claimed concurrency"
        assert "authenticated: 0" in tail and "play_entered: 0" in tail, "synthetic traffic must not authenticate"
        ordered = sorted(all_times)
        report["status_exchanges"] = {"passed": len(ordered), "min_ms": ordered[0],
                                      "median_ms": ordered[len(ordered)//2], "p95_ms": ordered[int(len(ordered)*.95)],
                                      "p99_ms": ordered[int(len(ordered)*.99)], "max_ms": ordered[-1]}
        report["exit_code"] = server.returncode
        report["limitations"] = ["Single shared-host run, not production capacity or world scalability.",
                                 "RSS snapshots, not allocator or peak-heap accounting; ps CPU resolution 0.01 seconds.",
                                 "Synthetic pending connections/auth failures; no fake authenticated players and no external auth-service load.",
                                 "Raised admission rates are measurement configuration, not default deployment limits."]
        output.write_text(json.dumps(report, indent=2) + "\n")
        output.with_suffix(".log").write_text(ready + "\n" + tail)
        print(json.dumps(report, indent=2))
    finally:
        for sock in sockets:
            sock.close()
        if server.poll() is None:
            server.kill()
            server.wait()


if __name__ == "__main__":
    main()
