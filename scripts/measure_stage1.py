#!/usr/bin/env python3
"""Reproducible macOS Stage 1 smoke/profile; Python stdlib only.

Build release first. Measures a separate Rust process, not this Python client.
No credentials, Minecraft assets, or private reference contents are collected.
"""
import argparse
import datetime
import json
import os
from pathlib import Path
import platform
import re
import resource
import signal
import socket
import struct
import subprocess
import time


def varint(value):
    result = bytearray()
    while True:
        byte = value & 127
        value >>= 7
        result.append(byte | (128 if value else 0))
        if not value:
            return bytes(result)


def frame(payload):
    return varint(len(payload)) + payload


def exact(sock, length):
    result = bytearray()
    while len(result) < length:
        chunk = sock.recv(length - len(result))
        if not chunk:
            raise AssertionError("unexpected EOF")
        result.extend(chunk)
    return bytes(result)


def read_varint(sock):
    value = 0
    for index in range(3):
        byte = exact(sock, 1)[0]
        value |= (byte & 127) << (7 * index)
        if not byte & 128:
            return value
    raise AssertionError("oversized VarInt")


def packet(sock):
    length = read_varint(sock)
    assert 1 <= length <= 4096
    return exact(sock, length)


def ping(address):
    hello = b"\x00\x89\x06\x09localhost\x63\xdd\x01"
    nonce = struct.pack(">q", -1234567890123456789)
    with socket.create_connection(address, timeout=2) as sock:
        sock.sendall(frame(hello) + frame(b"\x00") + frame(b"\x01" + nonce))
        response = packet(sock)
        assert response[0] == 0
        # Decode response string length independently from Rust.
        used, length = 1, 0
        while True:
            byte = response[used]
            length |= (byte & 127) << (7 * (used - 1))
            used += 1
            if not byte & 128:
                break
        assert len(response) == used + length
        status = json.loads(response[used:])
        assert status["version"] == {"name": "26.3", "protocol": 777}
        assert status["players"]["online"] == 0
        assert packet(sock) == b"\x01" + nonce
        assert sock.recv(1) == b""


def cpu_seconds(value):
    parts = value.split(":")
    return sum(float(part) * 60**index for index, part in enumerate(reversed(parts)))


def sample(pid):
    # macOS ps: resident set in KiB; cumulative CPU displayed to 0.01 seconds.
    values = subprocess.check_output(
        ["ps", "-p", str(pid), "-o", "rss=,time="], text=True
    ).split()
    threads = subprocess.check_output(["ps", "-M", "-p", str(pid)], text=True)
    return {
        "rss_kib": int(values[0]),
        "cpu_seconds": cpu_seconds(values[1]),
        "threads": len(threads.strip().splitlines()) - 1,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="target/release/smolmc")
    parser.add_argument("--output", default="evidence/stage1-resources.json")
    args = parser.parse_args()
    if platform.system() != "Darwin":
        parser.error("ps sampling in this script is macOS-specific")
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    binary = Path(args.binary).resolve()
    report = {
        "timestamp_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "host": platform.platform(),
        "machine": platform.machine(),
        "cpu_model": subprocess.check_output(["sysctl", "-n", "machdep.cpu.brand_string"], text=True).strip(),
        "logical_cpus": os.cpu_count(),
        "binary_bytes": binary.stat().st_size,
        "profile": "release, default limits, loopback; no other client on this endpoint",
        "samples": {},
    }
    log_path = output.with_suffix(".log")
    log = open(log_path, "w")
    child_before = resource.getrusage(resource.RUSAGE_CHILDREN)
    server = subprocess.Popen([str(binary), "--bind", "127.0.0.1:0"], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    sockets = []
    try:
        ready = server.stderr.readline().strip()
        assert ready.startswith("ready "), ready
        log.write(ready + "\n")
        host, port = re.search(r"address=(\S+)", ready).group(1).split(":")
        address = host, int(port)
        report["samples"]["ready"] = sample(server.pid)
        start = time.monotonic()
        time.sleep(5)
        report["samples"]["idle_5s"] = sample(server.pid)
        report["idle_elapsed_seconds"] = time.monotonic() - start
        elapsed_start = time.monotonic()
        rtts = []
        for _ in range(100):
            start = time.monotonic()
            ping(address)
            rtts.append((time.monotonic() - start) * 1000)
            time.sleep(0.06)  # Deliberately below default per-IP admission limit.
        report["pings"] = {
            "passed": len(rtts),
            "elapsed_seconds": time.monotonic() - elapsed_start,
            "exchange_ms_min": min(rtts),
            "exchange_ms_median": sorted(rtts)[len(rtts) // 2],
            "exchange_ms_max": max(rtts),
            "note": "TCP connect + status + ping + EOF, Python-driven; not production latency or throughput",
        }
        report["samples"]["after_100_pings"] = sample(server.pid)
        time.sleep(1.1)  # Begin the hold case in a fresh admission window.
        for _ in range(16):
            sockets.append(socket.create_connection(address, timeout=2))
        time.sleep(0.1)
        report["samples"]["16_open_connections"] = sample(server.pid)
        time.sleep(5.2)
        for sock in sockets:
            assert sock.recv(1) == b"", "deadline did not close idle connection"
            sock.close()
        sockets.clear()
        report["samples"]["after_idle_connections_expire"] = sample(server.pid)
        ping(address)
        # Keep sockets open while stopping to verify cancellation at process level.
        time.sleep(1.1)
        for _ in range(4):
            sockets.append(socket.create_connection(address, timeout=2))
        time.sleep(0.05)
        start = time.monotonic()
        server.send_signal(signal.SIGTERM)
        tail = server.communicate(timeout=3)[1]
        log.write(tail)
        report["shutdown_seconds"] = time.monotonic() - start
        assert server.returncode == 0
        assert "active: 0" in tail
        assert "pongs: 101" in tail
        assert "timeouts: 16" in tail
        for sock in sockets:
            assert sock.recv(1) == b""
        report["exit_code"] = server.returncode
        child_after = resource.getrusage(resource.RUSAGE_CHILDREN)
        report["child_cpu_seconds_including_ps_sampling"] = (
            child_after.ru_utime + child_after.ru_stime - child_before.ru_utime - child_before.ru_stime
        )
        report["limitations"] = [
            "RSS snapshots are not heap allocation accounting or peak RSS.",
            "CPU samples are quantized to 0.01s; zero observed delta does not mean zero CPU.",
            "One short run on a shared development host; no baseline comparison, no soak or production claim.",
            "No gameplay, worlds, JVM or authentication workload exists in Stage 1.",
        ]
        output.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))
    finally:
        for sock in sockets:
            sock.close()
        if server.poll() is None:
            server.kill()
            server.wait()
        log.close()


if __name__ == "__main__":
    main()
