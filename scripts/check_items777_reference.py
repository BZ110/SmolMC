#!/usr/bin/env python3
"""Original Rust corpus -> actual installed 26.3 codecs and mechanics. No credentials.
Only hashes/results leave the private work directory; no JARs/assets distributed.
"""
import argparse,hashlib,json,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--output',type=Path,default=Path('evidence/m4b-reference.json'));a=p.parse_args()
root=Path.home()/'Library/Application Support/minecraft';version=json.loads((root/'versions/26.3/26.3.json').read_text());assert version['id']=='26.3'
jar=root/'versions/26.3/26.3.jar';jars=[root/'libraries'/x['downloads']['artifact']['path']for x in version['libraries']if'artifact'in x.get('downloads',{})]+[jar]
cp=':'.join(str(f)for f in jars if f.is_file());work=Path('.tools/m4b/reference').resolve();work.mkdir(parents=True,exist_ok=True)
for package,example,out in [('smolmc-protocol-777','items_vectors','items-vectors.bin'),('smolmc-sessions','items_math','items-math.json')]:
 r=subprocess.run(['scripts/cargo-local.sh','run','--release','-p',package,'--example',example,'--locked'],capture_output=True,check=True);(work/out).write_bytes(r.stdout)
subprocess.run(['javac','-cp',cp,'-d',str(work),'reference-tests/diagnostics/modern-26-3/FlatLevel.java','reference-tests/Items777ReferenceCheck.java'],check=True)
r=subprocess.run(['java','-Xmx512m','-cp',str(work)+':'+cp,'Items777ReferenceCheck',str(work/'items-vectors.bin'),str(work/'items-math.json')],capture_output=True,text=True,cwd=work,timeout=60)
checks=[line.split('[STDOUT]:',1)[-1].strip()for line in r.stdout.splitlines()if'PASS'in line]
h=lambda f:hashlib.sha256(f.read_bytes()).hexdigest()
record=dict(result='PASS'if r.returncode==0 else'FAIL',checks=checks,minecraft_jar_sha256=h(jar),corpus_sha256={n:h(work/n)for n in ['items-vectors.bin','items-math.json']},scope='Actual 26.3 codec exact decode/consume/re-encode and stack/item metadata semantics; 72 Rust armor outputs vs actual CombatRules; gear durability/count/cost facts and seven actual applyDamage break transitions. No full server tick, loot-engine, GUI/auth/render or client acceptance claim.',diagnostic=r.stderr[-6000:]if r.returncode else'')
a.output.write_text(json.dumps(record,indent=2)+'\n');print('\n'.join(checks));assert r.returncode==0,r.stderr
