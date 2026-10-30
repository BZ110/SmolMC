#!/usr/bin/env python3
"""Short import/COW resource profile; metrics are measured separately from logical charges."""
import argparse,json,subprocess,hashlib
from pathlib import Path
from measure_stage1 import sample
p=argparse.ArgumentParser();p.add_argument('--world',type=Path,required=True);p.add_argument('--output',type=Path,default=Path('evidence/m4a-resources.json'));a=p.parse_args();binary=Path('target/release/examples/profile_m4a');raw=a.output.with_suffix('.log')
with raw.open('w')as log:
 child=subprocess.Popen([str(binary),str(a.world)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=log,text=True);phases=[]
 for line in child.stdout:
  row=json.loads(line);row['server_process']=sample(child.pid);phases.append(row);child.stdin.write('\n');child.stdin.flush()
 child.wait(timeout=10);assert child.returncode==0
record=dict(result='PASS',profile_binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),method='One 16-chunk full-height imported template; 1/128/256 COW instances; 128 edits in 8 instances; reset/destroy; 10000 local collision queries. RSS/CPU are separate ps PID samples. Logical retained charges exclude allocator/OS metadata. No players or JVM.',phases=phases)
a.output.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record,indent=2))
