#!/usr/bin/env python3
"""Short macOS release item-owner microprofile and encrypted mining/input workloads.
The encrypted test PID includes its Rust server and Rust fixture clients; its CPU
is inclusive, not a server-only capacity comparison. No JVM runs in these cases.
"""
import argparse,hashlib,json,os,subprocess,time
from pathlib import Path
from measure_stage1 import sample
from measure_replication_short import percentile
p=argparse.ArgumentParser();p.add_argument('--executable',type=Path,required=True);p.add_argument('--output',type=Path,default=Path('evidence/m4b-gameplay-resources.json'));a=p.parse_args()
raw=a.output.parent/(a.output.stem+'-raw');raw.mkdir(parents=True,exist_ok=True);exe=a.executable.resolve()
record=dict(method=__doc__,probe_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),runs=[])
micro=raw/'item-owner.json'
env=dict(os.environ,SMOLMC_ITEMS_REPORT=str(micro.resolve()))
with (raw/'item-owner.log').open('w')as log:subprocess.run([str(exe),'replication::items::tests::item_resource_profile','--exact','--ignored','--nocapture'],env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
record['item_owner']=json.loads(micro.read_text())
for name in ['items_encrypted_mining_drop_pickup_equipment_armor_durability_and_isolation','pipelined_placement_mining_early_finish_and_armor_clicks_reconcile']:
 timing=raw/(name+'.timing.json');env=dict(os.environ,SMOLMC_ITEMS_TIMING=str(timing.resolve()));cmd=[str(exe),'m1d_probe::'+name,'--exact','--nocapture']
 with (raw/(name+'.log')).open('w')as log:
  child=subprocess.Popen(cmd,env=env,stdout=log,stderr=subprocess.STDOUT);samples=[];began=time.monotonic()
  while child.poll()is None:
   try:samples.append((time.monotonic(),sample(child.pid)))
   except (subprocess.CalledProcessError,ValueError):pass
   time.sleep(.05)
  child.wait();assert child.returncode==0,Path(log.name).read_text()
 assert len(samples)>1
 start,s0=samples[0];end,s1=samples[-1];window=end-start;hist=json.loads(timing.read_text())
 row=dict(test=name,result='PASS',command=cmd,wall_seconds=time.monotonic()-began,sampling_seconds=window,cpu_one_core_percent=100*(s1['cpu_seconds']-s0['cpu_seconds'])/window,rss_kib=max(s['rss_kib']for _,s in samples),threads=max(s['threads']for _,s in samples),samples=len(samples),timing={k:dict(samples=sum(v),p50_us_upper=percentile(v,.5),p99_us_upper=percentile(v,.99))for k,v in hist.items()if isinstance(v,list)},scope='Inclusive Rust encrypted server + fixture-client PID; no JVM. Log2 histogram bounds are server processing/write timings, not input/RTT/render latency.')
 record['runs'].append(row);print(name,row['wall_seconds'],row['cpu_one_core_percent'],row['rss_kib'],flush=True)
a.output.write_text(json.dumps(record,indent=2)+'\n')
