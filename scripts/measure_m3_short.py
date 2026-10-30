#!/usr/bin/env python3
"""Revised M3: two matched movement cases + one short combat case, no parameter sweep.
Use preserved M2C evidence as before. Test-only encrypted Play fixture; production
cannot enable this entry point. CPU/RSS are server PID only, no Java sidecar.
"""
import argparse,json,os,signal,subprocess,time,hashlib
from pathlib import Path
from measure_stage1 import sample
from measure_replication_short import run

def main():
 p=argparse.ArgumentParser();p.add_argument('--executable',required=True);p.add_argument('--binary',default='target/release/smolmc');p.add_argument('--seconds',type=int,default=8);p.add_argument('--output',default='evidence/m3-after.json');args=p.parse_args()
 out=Path(args.output);root=out.parent/(out.stem+'-raw');root.mkdir(parents=True,exist_ok=True)
 exe=Path(args.executable).resolve();record=dict(binary_sha256=hashlib.sha256(Path(args.binary).read_bytes()).hexdigest(),probe_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),runs=[],method='1s all-ready warmup then ~8s server-PID active samples. Existing 20Hz circles/yaw/pose/chat. Combat: 32 players/2 worlds, grounded at y64 to exercise sweeps/effects, one attack per 13 client ticks to lowest currently visible peer, automatic vanilla respawn request, full health/kit/reset/chunk/entity handlers. Input/edges vary during death/loading; no lower movement pacing for efficiency. No authentication capacity or production capacity claim.')
 for enabled in [False,True]:
  server=subprocess.Popen([str(Path(args.binary).resolve()),'--bind','127.0.0.1:0','--pvp','on'if enabled else'off'],stderr=subprocess.PIPE,text=True)
  ready=server.stderr.readline();assert ready.startswith('ready address=');time.sleep(2.5);before=sample(server.pid);start=time.monotonic();time.sleep(3);after=sample(server.pid);wall=time.monotonic()-start;server.send_signal(signal.SIGTERM);_,logs=server.communicate(timeout=5);assert server.returncode==0
  record['idle_pvp_on'if enabled else'idle_off']=dict(rss_kib=after['rss_kib'],cpu_percent=(after['cpu_seconds']-before['cpu_seconds'])/wall*100,seconds=wall,threads=after['threads'])
 for players,worlds,combat in [(128,1,False),(256,16,False),(32,2,True)]:
  os.environ['SMOLMC_PROBE_PVP']='1'if combat else'0';os.environ['SMOLMC_PROBE_COMBAT']='1'if combat else'0'
  r=run(exe,root,players,worlds,args.seconds);record['runs'].append(r);out.write_text(json.dumps(record,indent=2)+'\n');print(players,worlds,combat,r['result'],round(r['cpu_one_core_percent'],2),'RSS',r['rss_kib'],flush=True)
  assert r['result']=='PASS'
if __name__=='__main__':main()
