#!/usr/bin/env python3
"""Unix subprocess regression: descriptor pressure must not evict live sockets."""
import argparse
import json
from pathlib import Path
import resource
import signal
import socket
import subprocess
import time
from measure_stage1 import frame, packet, ping


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="target/release/smolmc")
    parser.add_argument("--output", default="evidence/m1b-listener-recovery.json")
    args = parser.parse_args()

    def restrict_child():
        resource.setrlimit(resource.RLIMIT_NOFILE, (32, 32))

    server = subprocess.Popen([args.binary, "--bind", "127.0.0.1:0"],
                              stderr=subprocess.PIPE, text=True, preexec_fn=restrict_child)
    sockets = []
    try:
        ready = server.stderr.readline().strip()
        host, port = ready.split("address=")[1].split()[0].rsplit(":", 1)
        address = host, int(port)
        existing = socket.create_connection(address, timeout=2)
        sockets.append(existing)
        existing.sendall(frame(b"\x00\x89\x06\x09localhost\x63\xdd\x01") + frame(b"\x00"))
        assert packet(existing)[0] == 0
        for _ in range(25):
            sockets.append(socket.create_connection(address, timeout=0.2))
        time.sleep(0.15)
        assert server.poll() is None, "accept error terminated server"
        nonce = b"\x01\x12\x34\x56\x78\x9a\xbc\xde\xf0"
        existing.sendall(frame(nonce))
        assert packet(existing) == nonce, "descriptor pressure evicted established socket"
        for sock in sockets:
            sock.close()
        sockets.clear()
        time.sleep(1.2)
        ping(address)
        server.send_signal(signal.SIGTERM)
        tail = server.communicate(timeout=3)[1]
        assert server.returncode == 0, tail
        assert "active: 0" in tail
        import re
        errors = int(re.search(r"accept_errors: (\d+)", tail).group(1))
        assert errors > 0, "fault was not exercised"
        record = {"result": "PASS", "child_nofile_limit": 32,
                  "accept_errors_observed": errors, "established_connection_survived": True,
                  "new_status_connection_after_recovery": True, "exit_code": server.returncode,
                  "server_log": ready + "\n" + tail}
        output = Path(args.output)
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(record, indent=2) + "\n")
        print(json.dumps(record, indent=2))
    finally:
        for sock in sockets:
            sock.close()
        if server.poll() is None:
            server.kill()
            server.wait()


if __name__ == "__main__":
    main()
