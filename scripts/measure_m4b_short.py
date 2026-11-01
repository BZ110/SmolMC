#!/usr/bin/env python3
"""Short M4A checkpoint comparison, then explicit item/iron-armor combat.
Same 20Hz inputs, coordinate translation and shared platform as M4A; Rust/JVM separate.
"""
import argparse,hashlib,json,os,subprocess,signal,time
from pathlib import Path
from measure_replication_short import run
from measure_stage1 import sample
p=argparse.ArgumentParser();p.add_argument('--after-only',action='store_true');p.add_argument('--world',type=Path,required=True);p.add_argument('--before-probe',type=Path,required=True);p.add_argument('--after-probe',type=Path,required=True);p.add_argument('--output',type=Path,default=Path('evidence/m4b-performance.json'));a=p.parse_args()
raw=a.output.parent/(a.output.stem+'-raw');raw.mkdir(parents=True,exist_ok=True);h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
record=dict(method=__doc__,scope='Short local macOS release regression/micro workloads, synthetic encrypted post-auth clients. CPU percent of one core and whole Rust PID RSS KiB. No JVM in these cases, no internet capacity or GUI acceptance claim.',before_binary_sha256=h(Path('.tools/m4b/smolmc-m4a')),after_binary_sha256=h(Path('target/release/smolmc')),before_probe_sha256=h(a.before_probe),after_probe_sha256=h(a.after_probe),idle=[],runs=[])
for label,binary in ([('m4b',Path('target/release/smolmc'))]if a.after_only else [('m4a',Path('.tools/m4b/smolmc-m4a')),('m4b',Path('target/release/smolmc'))]):
 for imported in [False,True]:
  cmd=[str(binary.resolve()),'--bind','127.0.0.1:0']
  if imported:cmd+=['--import-world',str(a.world),'--import-bounds','-16,-64,-80,48,320,-16','--import-remove-unsupported','--world-profile','build','--pvp','on']
  if label=='m4b'and imported:cmd+=['--inventory-policy','items-keep']
  with (raw/f'{label}-idle-{imported}.log').open('w')as log:
   child=subprocess.Popen(cmd,stdout=subprocess.DEVNULL,stderr=log)
   try:
    for _ in range(100):
     if'ready address='in Path(log.name).read_text():break
     assert child.poll()is None;time.sleep(.05)
    time.sleep(2.5);start=sample(child.pid);began=time.monotonic();time.sleep(3);end=sample(child.pid);seconds=time.monotonic()-began
    record['idle'].append(dict(release=label,imported=imported,seconds=seconds,rss_kib=end['rss_kib'],threads=end['threads'],cpu_one_core_percent=(end['cpu_seconds']-start['cpu_seconds'])/seconds*100))
   finally:child.send_signal(signal.SIGTERM);child.wait(timeout=10)
   assert child.returncode==0
os.environ['SMOLMC_PROBE_IMPORT']=str(a.world.resolve())
for label,exe in ([('m4b',a.after_probe)]if a.after_only else [('m4a',a.before_probe),('m4b',a.after_probe)]):
 folder=raw/label;folder.mkdir(exist_ok=True)
 for players,worlds,combat in [(128,1,False),(256,16,False),(32,2,True)]:
  os.environ['SMOLMC_PROBE_PVP']='1'if combat else'0';os.environ['SMOLMC_PROBE_COMBAT']='1'if combat else'0'
  r=run(exe.resolve(),folder,players,worlds,8);r.update(release=label,items=False);record['runs'].append(r);a.output.write_text(json.dumps(record,indent=2)+'\n');print(label,players,worlds,r['result'],round(r['cpu_one_core_percent'],2),r['rss_kib'],flush=True);assert r['result']=='PASS'
folder=raw/'m4b-armor';folder.mkdir(exist_ok=True);os.environ.update(SMOLMC_PROBE_ITEMS='1',SMOLMC_PROBE_ARMOR='1',SMOLMC_PROBE_PVP='1',SMOLMC_PROBE_COMBAT='1')
r=run(a.after_probe.resolve(),folder,32,2,8);r.update(release='m4b-iron-armor',items=True);record['runs'].append(r);a.output.write_text(json.dumps(record,indent=2)+'\n');print('armor',r['result'],round(r['cpu_one_core_percent'],2),r['rss_kib'],flush=True);assert r['result']=='PASS'
