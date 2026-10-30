#!/usr/bin/env python3
"""Short macOS process-separated M2C overhead measurements; no account workflows."""
import argparse,datetime,hashlib,json,signal,subprocess,tempfile,time
from pathlib import Path
from measure_stage1 import sample

def window(process,seconds=3):
    a=sample(process.pid);start=time.monotonic();samples=[]
    while time.monotonic()-start<seconds:samples.append(sample(process.pid));time.sleep(.2)
    b=sample(process.pid);wall=time.monotonic()-start
    return dict(seconds=wall,cpu_one_core_percent=(b['cpu_seconds']-a['cpu_seconds'])/wall*100,rss_kib=max(s['rss_kib'] for s in samples),threads=max(s['threads'] for s in samples))
def main():
    p=argparse.ArgumentParser();p.add_argument('--binary',default='target/release/smolmc');p.add_argument('--output',default='evidence/m2c-resources.json');args=p.parse_args();binary=Path(args.binary).resolve();report=dict(timestamp_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),method='Separate Rust/JVM PIDs; 2.5s idle warmup, ~3s windows. Java timings after 100 warmup calls. JVM default ergonomics, no forced GC/heap cap. Event latency = Java submit through callback including IPC/control commit, not synchronized cross-process emission time.')
    with tempfile.TemporaryDirectory(prefix='smc-perf-') as directory:
        root=Path(directory)
        for mode in ['off','listener','java']:
            bridge=root/mode;command=[str(binary),'--bind','127.0.0.1:0']
            if mode!='off':command+=['--bridge-dir',str(bridge),'--bridge-access','control']
            with (root/f'{mode}.log').open('w') as log:
                rust=subprocess.Popen(command,stderr=log);java=None
                try:
                    end=time.monotonic()+8
                    while 'ready address=' not in Path(log.name).read_text():assert rust.poll() is None and time.monotonic()<end;time.sleep(.01)
                    if mode=='java':
                        java=subprocess.Popen(['java','-cp','target/java/classes','dev.smolmc.BridgeBenchmark',str(bridge)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
                        report['bridge_measurements']=json.loads(java.stdout.readline())
                    time.sleep(2.5)
                    if java:
                        # Simultaneous window; independent per-PID CPU deltas.
                        ja=sample(java.pid);began=time.monotonic();report[mode]=window(rust);jb=sample(java.pid);wall=time.monotonic()-began
                        report['jvm']=dict(cpu_one_core_percent=(jb['cpu_seconds']-ja['cpu_seconds'])/wall*100,rss_kib=jb['rss_kib'],threads=jb['threads'],seconds=wall)
                        java.communicate('\n',timeout=4);assert java.returncode==0
                    else:report[mode]=window(rust)
                finally:
                    if java and java.poll() is None:java.kill();java.wait()
                    rust.send_signal(signal.SIGTERM);rust.wait(timeout=5);assert rust.returncode==0
            assert not bridge.exists()
    Path(args.output).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
