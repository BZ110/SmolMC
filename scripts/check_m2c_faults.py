#!/usr/bin/env python3
"""Actual Rust/Java bridge fault checks. Credentials stay in memory/private files.
No Minecraft authentication or proprietary asset access. Unix only.
"""
import argparse, json, os, signal, socket, struct, subprocess, tempfile, time
from pathlib import Path
from measure_stage1 import ping

def exact(s,n):
    b=bytearray()
    while len(b)<n:
        c=s.recv(n-len(b))
        if not c:raise EOFError()
        b.extend(c)
    return bytes(b)
def receive(s):
    n=struct.unpack('>I',exact(s,4))[0];assert 10<=n<=131072
    return exact(s,n)
def frame(b):return struct.pack('>I',len(b))+b
def hello(directory,token=None,version=1):
    s=socket.socket(socket.AF_UNIX);s.settimeout(4);s.connect(str(directory/'socket'))
    credential=(directory/'token').read_bytes() if token is None else token
    s.sendall(frame(struct.pack('>BQBHH',1,1,0,version,0)+credential))
    r=receive(s);status=struct.unpack('>H',r[10:12])[0]
    return s,r,status
class Client:
    def __init__(self,directory):
        self.s,r,status=hello(directory);assert status==0;self.server=r[12:28];self.sequence=1
    def request(self,op,payload=b''):
        self.sequence+=1;self.s.sendall(frame(struct.pack('>BQB',1,self.sequence,op)+self.server+payload));r=receive(self.s);assert r[0]==2 and r[9]==op;return struct.unpack('>H',r[10:12])[0],r[12:]
    def close(self):self.s.close()
def text(s):b=s.encode();return struct.pack('>H',len(b))+b

def main():
    p=argparse.ArgumentParser();p.add_argument('--binary',default='target/release/smolmc');p.add_argument('--output',default='evidence/m2c-faults.json');args=p.parse_args();checks=[]
    with tempfile.TemporaryDirectory(prefix='smc-fault-') as root:
        root=Path(root)
        def start(access='control',worlds=1):
            d=root/f'ipc-{len(checks)}';log=(root/f'log-{len(checks)}').open('w+')
            process=subprocess.Popen([str(Path(args.binary).resolve()),'--bind','127.0.0.1:0','--instances',str(worlds),'--bridge-dir',str(d),'--bridge-access',access],stderr=log)
            end=time.monotonic()+8
            while time.monotonic()<end:
                log.flush();lines=Path(log.name).read_text()
                ready=next((line for line in lines.splitlines() if line.startswith('ready address=')),None)
                if ready:return process,d,tuple([ready.split('address=')[1].split(':')[0],int(ready.split('address=')[1].split(':')[1].split()[0])]),log
                assert process.poll() is None;time.sleep(.01)
            raise AssertionError('startup timeout')
        process,d,address,log=start(worlds=512)
        try:
            s,_,status=hello(d,b'\0'*32);assert status==3;s.close();checks.append('wrong credential rejected')
            s,_,status=hello(d,version=99);assert status==2;s.close();checks.append('schema mismatch rejected')
            c=Client(d);status,world=c.request(1);assert status==0
            status,_=c.request(2,struct.pack('>QB',1,1)+b'bad');assert status==1
            status,data=c.request(18);assert status==0 and struct.unpack('>H',data[:2])[0]==512;checks.append('trailing mutation rejected atomically; bounded large listing')
            # Declared frame exceeds cap: reject before allocation; core status survives.
            bad=socket.socket(socket.AF_UNIX);bad.connect(str(d/'socket'));bad.sendall(struct.pack('>I',131073));assert bad.recv(1)==b'';bad.close();ping(address);checks.append('oversized framing isolated')
            bad=socket.socket(socket.AF_UNIX);bad.settimeout(4);bad.connect(str(d/'socket'));bad.sendall(b'\0');assert bad.recv(1)==b'';bad.close();ping(address);checks.append('stalled partial handshake expires')
            # Fill a peer's outbound socket and stop consuming; write timeout releases
            # its command ownership while an independent peer/core remains healthy.
            slow=Client(d);assert slow.request(15,text('blocked'))[0]==0
            requests=b''.join(frame(struct.pack('>BQB',1,i,18)+slow.server) for i in range(3,103));slow.s.sendall(requests)
            recovered=False;end=time.monotonic()+5
            while time.monotonic()<end:
                status,_=c.request(15,text('blocked'))
                if status==0:recovered=True;break
                assert status==10;ping(address);time.sleep(.1)
            assert recovered;slow.close();assert c.request(16,text('blocked'))[0]==0;checks.append('nonreading peer write deadline and ownership cleanup')
            # An actual JVM stalls its callback executor. Bounded callback overflow
            # closes only that sidecar, with a machine-readable reason.
            java=subprocess.Popen(['java','-ea','-cp','target/java/classes','dev.smolmc.FaultClient',str(d),'slow'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            out,err=java.communicate(timeout=15);assert java.returncode==0,(out,err);assert 'BACKPRESSURE' in out;ping(address);checks.append('real JVM slow callback -> explicit backpressure; Rust healthy')
            java=subprocess.Popen(['java','-ea','-cp','target/java/classes','dev.smolmc.FaultClient',str(d),'hold'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            assert java.stdout.readline().strip()=='READY';java.kill();java.wait(timeout=3)
            end=time.monotonic()+3
            while c.request(15,text('owned'))[0]!=0:
                assert time.monotonic()<end;time.sleep(.02)
            assert c.request(16,text('owned'))[0]==0;ping(address);checks.append('killed JVM releases registration; core responsive; reattach')
            # Shared request quota is observable, without silently dropping calls.
            rejected=0
            for i in range(2500):
                status,_=c.request(7,world);assert status in (0,9);rejected+=status==9
            assert rejected>0;checks.append(f'shared work saturation: {rejected} explicit Backpressure responses')
            c.close()
        finally:
            process.send_signal(signal.SIGTERM);process.wait(timeout=5);log.close()
        assert not d.exists() and process.returncode==0;checks.append('shutdown joins peers and removes private files')
        process,d,address,log=start('observe')
        try:
            c=Client(d);assert c.request(1)[0]==0;assert c.request(2,struct.pack('>QB',1,1))[0]==3;c.close();checks.append('observe credential cannot mutate')
        finally:process.send_signal(signal.SIGTERM);process.wait(timeout=5);log.close()
    Path(args.output).write_text(json.dumps(dict(result='PASS',checks=checks),indent=2)+'\n');print('\n'.join(checks))
if __name__=='__main__':main()
