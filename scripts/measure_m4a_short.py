#!/usr/bin/env python3
"""Three short final release scenarios on a bounded imported map, plus PID-sampled idle/profile.
Movement pacing, radii, yaw, pose/chat, compression/encryption match preserved M3.
Only constant coordinate translations change. A shared 7x7 COW platform at y199
keeps the combat path comparable; imported terrain remains loaded underneath.
"""
import argparse,json,subprocess,time,signal,hashlib,os
from pathlib import Path
from measure_stage1 import sample
from measure_replication_short import run
p=argparse.ArgumentParser();p.add_argument('--world',type=Path,required=True);p.add_argument('--executable',type=Path,required=True);p.add_argument('--output',type=Path,default=Path('evidence/m4a-after.json'));a=p.parse_args();raw=a.output.parent/(a.output.stem+'-raw');raw.mkdir(parents=True,exist_ok=True)
h=lambda f:hashlib.sha256(f.read_bytes()).hexdigest()
record=dict(binary_sha256=h(Path('target/release/smolmc')),probe_sha256=h(a.executable),method=__doc__,runs=[])
for imported in [False,True]:
 cmd=['target/release/smolmc','--bind','127.0.0.1:0']
 if imported:cmd+=['--import-world',str(a.world),'--import-bounds','-16,-64,-80,48,320,-16','--import-remove-unsupported','--world-profile','build','--pvp','on']
 with (raw/('idle-imported.log'if imported else'idle-flat.log')).open('w')as log:
  child=subprocess.Popen(cmd,stdout=subprocess.DEVNULL,stderr=log)
  for _ in range(100):
   if 'ready address='in Path(log.name).read_text():break
   assert child.poll()is None;time.sleep(.05)
  time.sleep(2.5);s=sample(child.pid);start=time.monotonic();time.sleep(3);end=sample(child.pid);wall=time.monotonic()-start;child.send_signal(signal.SIGTERM);child.wait(timeout=10);assert child.returncode==0
  record['idle_imported'if imported else'idle_flat']=dict(rss_kib=end['rss_kib'],threads=end['threads'],cpu_percent=(end['cpu_seconds']-s['cpu_seconds'])/wall*100,seconds=wall)
os.environ['SMOLMC_PROBE_IMPORT']=str(a.world.resolve())
for players,worlds,combat in [(128,1,False),(256,16,False),(32,2,True)]:
 os.environ['SMOLMC_PROBE_PVP']='1'if combat else'0';os.environ['SMOLMC_PROBE_COMBAT']='1'if combat else'0';r=run(a.executable.resolve(),raw,players,worlds,8);record['runs'].append(r);a.output.write_text(json.dumps(record,indent=2)+'\n');print(players,worlds,combat,r['result'],round(r['cpu_one_core_percent'],2),'RSS',r['rss_kib'],flush=True);assert r['result']=='PASS'
