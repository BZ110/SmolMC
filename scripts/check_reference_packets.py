#!/usr/bin/env python3
"""Optional interoperability check against the operator's installed official 26.3
client codecs. Reads only version metadata, libraries and the version JAR; never
account files, auth tokens, launch arguments or credentials. No game assets/source
are exported. Does not download or redistribute Minecraft.
"""
import argparse,hashlib,json,platform,subprocess
from pathlib import Path

def main():
 p=argparse.ArgumentParser();p.add_argument('--minecraft-dir',type=Path,default=Path.home()/('Library/Application Support/minecraft'if platform.system()=='Darwin'else'.minecraft'));p.add_argument('--output',type=Path,default=Path('evidence/m3-reference-codecs.json'));args=p.parse_args()
 root=args.minecraft_dir;metadata=root/'versions/26.3/26.3.json';version=json.loads(metadata.read_text());assert version['id']=='26.3'
 jar=root/'versions/26.3/26.3.jar';libraries=[root/'libraries'/lib['downloads']['artifact']['path']for lib in version['libraries']if 'artifact'in lib.get('downloads',{})];libraries.append(jar)
 assert jar.is_file(),'install the official 26.3 client first'
 # Version metadata lists optional artifacts for other OS/CPU targets as well.
 libraries=[path for path in libraries if path.is_file()]
 cp=':'.join(map(str,libraries));work=Path('target/reference-check').resolve();work.mkdir(parents=True,exist_ok=True)
 vectors=subprocess.run(['scripts/cargo-local.sh','run','--release','-p','smolmc-protocol-777','--example','pvp_vectors','--locked'],capture_output=True,check=True).stdout
 path=work/'pvp-vectors.bin';path.write_bytes(vectors)
 math=subprocess.run(['scripts/cargo-local.sh','run','--release','-p','smolmc-sessions','--example','pvp_math','--locked'],capture_output=True,check=True).stdout
 math_path=work/'pvp-math.json';math_path.write_bytes(math)
 subprocess.run(['javac','-cp',cp,'-d',str(work),'reference-tests/Protocol777ReferenceCheck.java'],check=True)
 run=subprocess.run(['java','-Xmx512m','-cp',str(work)+':'+cp,'Protocol777ReferenceCheck',str(path),str(math_path)],capture_output=True,text=True,timeout=45,cwd=work)
 passes=[line.split('[STDOUT]:',1)[-1].strip()for line in run.stdout.splitlines()if'PASS'in line]
 record=dict(result='PASS'if run.returncode==0 else'FAIL',version='26.3',minecraft_jar_sha256=hashlib.sha256(jar.read_bytes()).hexdigest(),vector_sha256=hashlib.sha256(vectors).hexdigest(),math_sha256=hashlib.sha256(math).hexdigest(),checks=passes,exit_code=run.returncode,scope='Actual installed vanilla codecs with locally initialized builtin/dynamic registries; no authentication, renderer or gameplay claim; no JAR/assets included. Each outbound vector decodes to its expected class, consumes exactly and re-encodes byte-for-byte. Off-hand action 7 independently decoded.',diagnostic=run.stderr if run.returncode else'')
 args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(record,indent=2)+'\n');print('\n'.join(passes));assert run.returncode==0
if __name__=='__main__':main()
