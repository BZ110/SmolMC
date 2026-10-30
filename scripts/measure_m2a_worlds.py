#!/usr/bin/env python3
"""macOS separate-process M2A lifecycle/idle measurements; real Service, no clients.

ps samples the child only. CPU is CPU-time delta / wall-time, one core = 100%.
RSS is resident process memory, including allocator retention, not live heap bytes.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time
from measure_stage1 import sample


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', default='target/release/examples/profile_worlds')
    parser.add_argument('--seconds', type=float, default=3)
    parser.add_argument('--output', default='evidence/m2a-world-resources.json')
    args = parser.parse_args()
    assert platform.system() == 'Darwin', 'macOS ps/wait4 units required'
    binary = Path(args.binary).resolve()
    output = Path(args.output)
    log_path = output.with_suffix('.log')
    record = dict(timestamp_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  host=platform.platform(), machine=platform.machine(), binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                  command=[str(binary)], interval_seconds=args.seconds,
                  workload='Real Service; one shared stone/registry baseline; 0,1,10,100,1000,4096 empty worlds; no clients or directory task. Three 10,000-world create/edit/reset/destroy cycles in one process.',
                  limitations=['Short idle windows; ps CPU resolution 0.01s; RSS includes allocator high-water memory.', 'No live heap allocator accounting, peak transient snapshots, world edits under maximum capacity or long soak.', 'Separate PID sampled; shared host, no host-isolation claim.'], phases=[])
    with log_path.open('w') as log:
        child = subprocess.Popen([str(binary)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=log, text=True, bufsize=1)
        try:
            record['ready'] = json.loads(child.stdout.readline())
            base = None
            for command in ['hold 0','hold 1','hold 10','hold 100','hold 1000','hold 4096','hold 0', 'churn 100 100','churn 100 100','churn 100 100','hold 0']:
                child.stdin.write(command+'\n'); child.stdin.flush()
                phase = json.loads(child.stdout.readline())
                before = sample(child.pid)
                started = time.monotonic()
                time.sleep(args.seconds)
                after = sample(child.pid)
                wall = time.monotonic()-started
                if base is None:
                    base = before['rss_kib']
                phase.update(command=command, before=before, after=after, sampled_wall_seconds=wall,
                             idle_cpu_one_core_percent=100*(after['cpu_seconds']-before['cpu_seconds'])/wall,
                             incremental_rss_kib=after['rss_kib']-base)
                record['phases'].append(phase)
                output.write_text(json.dumps(record, indent=2)+'\n')
                print(f"{command}: RSS {after['rss_kib']} KiB, idle CPU {phase['idle_cpu_one_core_percent']:.3f}%", flush=True)
            child.stdin.write('quit\n'); child.stdin.flush()
            _, status, usage = os.wait4(child.pid, 0)
            child.returncode = os.waitstatus_to_exitcode(status)
            assert child.returncode == 0
            record.update(result='PASS', exit_code=child.returncode,
                          process_usage=dict(peak_rss_kib=usage.ru_maxrss/1024, user_cpu_seconds=usage.ru_utime, system_cpu_seconds=usage.ru_stime))
            output.write_text(json.dumps(record, indent=2)+'\n')
        finally:
            if child.poll() is None:
                child.kill(); child.wait()


if __name__ == '__main__':
    main()
