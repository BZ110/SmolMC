#!/usr/bin/env python3
"""Three fixed short release-Play scenarios; identical before/after instrumentation.
Synthetic post-auth TCP/AES-CFB8/compression; 20Hz movement, original 50ms policies.
"""
import argparse,datetime,hashlib,json,os,re,signal,subprocess,time
from pathlib import Path
from measure_stage1 import sample
from measure_m1d_active import wait_file

def number(text,key):return int(re.search(r'\b'+key+r': (\d+)',text).group(1))
def percentile(hist,p):
    total=sum(hist)
    if not total:return None
    target=max(1,int(total*p+0.999999));seen=0
    for i,n in enumerate(hist):
        seen+=n
        if seen>=target:return 2**i

def run(exe,root,players,worlds,seconds,bridge=False):
    stem=root/f'p{players}-w{worlds}';ready=stem.with_suffix('.ready');live=ready.with_suffix('.sample')
    for p in [ready,live,ready.with_suffix('.clients')]:p.unlink(missing_ok=True)
    env=dict(os.environ,SMOLMC_PROBE_READY=str(ready),SMOLMC_PROBE_REPORT=str(stem.with_suffix('.server.json')),SMOLMC_PROBE_ROLE='server',SMOLMC_PROBE_WORLDS=str(worlds))
    directory=stem.with_suffix(".ipc")
    if bridge:env["SMOLMC_PROBE_BRIDGE_DIRECTORY"]=str(directory.resolve())
    cmd=[str(exe),'m1d_probe::active_play_resource_probe','--ignored','--exact','--nocapture']
    with stem.with_suffix('.server.log').open('w') as log,stem.with_suffix('.client.log').open('w') as clog:
        server=subprocess.Popen(cmd,env=env,stdout=log,stderr=subprocess.STDOUT);client=None;jvm=None
        def snapshot():
            live.unlink(missing_ok=True);server.send_signal(signal.SIGUSR1);wait_file(live,server,5);return json.loads(live.read_text())
        try:
            wait_file(ready,server,10)
            if bridge:
                jvm=subprocess.Popen(['java','-cp','target/java/classes','dev.smolmc.BridgeObserver',str(directory.resolve())],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
                assert jvm.stdout.readline().strip()=='READY'
            client_env=dict(env,SMOLMC_PROBE_ROLE='client',SMOLMC_PROBE_PLAYERS=str(players),SMOLMC_PROBE_SECONDS=str(seconds+3),SMOLMC_PROBE_REPORT=str(stem.with_suffix('.client.json')))
            client=subprocess.Popen(cmd,env=client_env,stdout=clog,stderr=subprocess.STDOUT)
            wait_file(ready.with_suffix('.clients'),client,40);time.sleep(1)
            before=snapshot();start=sample(server.pid);began=time.monotonic();samples=[];jstart=sample(jvm.pid) if jvm else None;jsamples=[]
            profile=subprocess.Popen(['/usr/bin/sample',str(server.pid),'1','10','-file',str(stem.with_suffix('.sample.txt'))],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            while time.monotonic()-began<seconds:
                samples.append(sample(server.pid));
                if jvm:jsamples.append(sample(jvm.pid))
                time.sleep(0.25)
            end=sample(server.pid);wall=time.monotonic()-began;after=snapshot();profile.wait(timeout=3);jend=sample(jvm.pid) if jvm else None
            if jvm:jvm.communicate("\n",timeout=3);assert jvm.returncode==0
            client.wait(timeout=6);time.sleep(0.25);server.send_signal(signal.SIGTERM);server.wait(timeout=5)
            final=json.loads(stem.with_suffix('.server.json').read_text());clients=json.loads(stem.with_suffix('.client.json').read_text())
            diff={k:[b-a for a,b in zip(before['perf'][k],after['perf'][k])] if isinstance(before['perf'][k],list) else after['perf'][k]-before['perf'][k] for k in before['perf']}
            timing={k:dict(samples=sum(v),p50_us_upper=percentile(v,.5),p95_us_upper=percentile(v,.95),p99_us_upper=percentile(v,.99)) for k,v in diff.items() if isinstance(v,list)}
            expected=players*(players//worlds-1)
            combat=os.environ.get('SMOLMC_PROBE_COMBAT')=='1'
            healthy=server.returncode==0 and client.returncode==0 and clients['closed_clients']==0 and (number(after['sessions'],'peak_visible_edges')>=expected if combat else number(after['sessions'],'visible_edges')==expected) and all(number(final['network'],k)==0 for k in ['active','frame_bytes_retained','protocol_errors','timeouts','task_failures','io_errors']) and number(final['sessions'],'active_replicas')==0
            return dict(java=dict(cpu_one_core_percent=100*(jend['cpu_seconds']-jstart['cpu_seconds'])/wall,rss_kib=max(s['rss_kib'] for s in jsamples),threads=max(s['threads'] for s in jsamples)) if jvm else None,result='PASS' if healthy else 'FAIL',players=players,worlds=worlds,measurement_seconds=wall,cpu_one_core_percent=100*(end['cpu_seconds']-start['cpu_seconds'])/wall,rss_kib=max(s['rss_kib'] for s in samples),visible_edges=number(after['sessions'],'visible_edges'),expected_nominal_edges=expected,combat=combat,delivered_versions=diff['fanout_versions'],position_packets=diff['positions'],positions_per_second=diff['positions']/wall,published_updates=number(after['sessions'],'replica_updates')-number(before['sessions'],'replica_updates'),timing=timing,perf_deltas=diff,profile_exit_code=profile.returncode,before=before,after=after,final=final,clients=clients)
        finally:
            for p in [jvm,client,server]:
                if p and p.poll() is None:p.kill();p.wait()

def main():
    p=argparse.ArgumentParser();p.add_argument('--executable',required=True);p.add_argument('--output',required=True);p.add_argument('--seconds',type=int,default=9);p.add_argument('--dense-only',action='store_true');p.add_argument('--small-only',action='store_true');p.add_argument('--bridge',action='store_true');args=p.parse_args()
    exe=Path(args.executable).resolve();out=Path(args.output);root=out.parent/(out.stem+'-raw');root.mkdir(parents=True,exist_ok=True)
    record=dict(timestamp_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),executable_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),warmup_seconds=1,active_seconds=args.seconds,workload='Existing synthetic post-auth Play fixture, 20Hz circular movement/yaw, 1Hz crouch, one chat sender/2s; unchanged 50ms publish/flush pacing; separate server/client PIDs.',latency='Test-only log2 microsecond histograms; upper bucket bounds. Flush includes awaits/yields; sampled publication-to-write age is server-side, not network/client render latency.',runs=[])
    for players,worlds in ([(128,1)] if args.small_only else [(256,1)] if args.dense_only else [(256,1),(256,16),(128,1)]):
        r=run(exe,root,players,worlds,args.seconds,args.bridge);record['runs'].append(r);out.write_text(json.dumps(record,indent=2)+'\n');print(f'{players}/{worlds}: {r["result"]} CPU {r["cpu_one_core_percent"]:.2f}% RSS {r["rss_kib"]} positions/s {r["positions_per_second"]:.0f}',flush=True)
        assert r['result']=='PASS'
if __name__=='__main__':main()
