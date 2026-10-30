#!/usr/bin/env python3
"""macOS separate-process, real-Service M2B empty/edited/COW lifecycle profile.

The child executor stays active while one fixed controller thread waits on stdin.
RSS includes allocator retention. Charged snapshot bytes are known headers/boxed
payloads, excluding allocator rounding, temporary edit buffers and global runtime.
"""
import argparse,datetime,hashlib,json,os,platform,subprocess,time
from pathlib import Path
from measure_stage1 import sample

def run(binary,root,edits,mode,seconds):
    stem=f'{mode}-{edits}'
    commands=['hold 0']
    if mode=='clones': commands+=['clones 1','clones 10','clones 100','clones 1000','cow','reset','clear']
    else:
        commands += [f'hold {n}' for n in ([1,10,100,1000,4096] if edits==0 else [1,10,100,1000])]
        commands += ['reset','clear',f'churn {100 if edits<5000 else 20} 100','clear']
    env=dict(os.environ,SMOLMC_PROFILE_EDITS=str(edits))
    if mode=='spread':env['SMOLMC_PROFILE_SPREAD']='1'
    result=dict(edits_per_independent_world=edits,mode=mode,phases=[])
    with (root/(stem+'.log')).open('w') as log:
        p=subprocess.Popen([str(binary)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=log,text=True,bufsize=1,env=env)
        try:
            result['ready']=json.loads(p.stdout.readline());baseline=None
            for command in commands:
                cpu_before=sample(p.pid)['cpu_seconds']
                p.stdin.write(command+'\n');p.stdin.flush()
                phase=json.loads(p.stdout.readline());before=sample(p.pid)
                start=time.monotonic();time.sleep(seconds);after=sample(p.pid);wall=time.monotonic()-start
                if baseline is None:baseline=after['rss_kib']
                phase.update(before_idle=before,after_idle=after,incremental_rss_kib=after['rss_kib']-baseline,idle_wall_seconds=wall,idle_cpu_one_core_percent=100*(after['cpu_seconds']-before['cpu_seconds'])/wall,operation_cpu_seconds=before['cpu_seconds']-cpu_before)
                result['phases'].append(phase)
                (root/(stem+'.json')).write_text(json.dumps(result,indent=2)+'\n')
                print(f'{stem} {command}: {after["rss_kib"]} KiB, charged {phase["retained_overlay_charged_bytes"]}, idle CPU {phase["idle_cpu_one_core_percent"]:.3f}%',flush=True)
            p.stdin.write('quit\n');p.stdin.flush();_,status,usage=os.wait4(p.pid,0);p.returncode=os.waitstatus_to_exitcode(status);assert p.returncode==0
            result.update(result='PASS',process_usage=dict(peak_rss_kib=usage.ru_maxrss/1024,user_cpu_seconds=usage.ru_utime,system_cpu_seconds=usage.ru_stime))
            (root/(stem+'.json')).write_text(json.dumps(result,indent=2)+'\n');return result
        finally:
            if p.poll() is None:p.kill();p.wait()

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',default='target/release/examples/profile_m2b');parser.add_argument('--seconds',type=float,default=3);parser.add_argument('--output',default='evidence/m2b-world-resources.json');args=parser.parse_args()
    assert platform.system()=='Darwin'
    binary=Path(args.binary).resolve();output=Path(args.output);root=output.parent/(output.stem+'-raw');root.mkdir(parents=True,exist_ok=True)
    record=dict(timestamp_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),host=platform.platform(),binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),command=[str(binary)],idle_seconds=args.seconds,raw_directory=str(root),runs=[],limitations=['Shared macOS host; ps CPU resolution 0.01s; short windows, no long soak.','Separate child PID; real Service plus fixed benchmark controller, no listener or clients.','Charged bytes are deterministic payload/header accounting, not RSS or allocator live bytes.','COW clone cost includes new identity/lifecycle but shares unchanged immutable payload.'])
    for edits,mode in [(0,'independent'),(1,'independent'),(100,'independent'),(5000,'independent'),(5000,'spread'),(5000,'clones')]:
        record['runs'].append(run(binary,root,edits,mode,args.seconds));output.write_text(json.dumps(record,indent=2)+'\n')
    record['result']='PASS';output.write_text(json.dumps(record,indent=2)+'\n')
if __name__=='__main__':main()
