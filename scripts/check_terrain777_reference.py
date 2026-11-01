#!/usr/bin/env python3
"""Optional operator-owned 26.3 map/codecs check; no credentials or game assets exported.
Private corpus stays under .tools. Source map is opened read-only by Rust.
"""
import argparse,hashlib,json,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--world',type=Path,required=True);p.add_argument('--output',type=Path,default=Path('evidence/m4a-reference-codecs.json'));p.add_argument('--mechanics-output',type=Path);a=p.parse_args()
root=Path.home()/'Library/Application Support/minecraft';v=json.loads((root/'versions/26.3/26.3.json').read_text());jars=[root/'libraries'/x['downloads']['artifact']['path']for x in v['libraries']if'artifact'in x.get('downloads',{})]+[root/'versions/26.3/26.3.jar'];cp=':'.join(str(f)for f in jars if f.exists());work=Path('.tools/m4a/reference').resolve();work.mkdir(parents=True,exist_ok=True);corpus=Path('.tools/m4a/wire').resolve()
subprocess.run(['scripts/cargo-local.sh','run','--release','-p','smolmc-sessions','--example','import_wire','--locked','--',str(a.world),str(corpus)],check=True,capture_output=True)
subprocess.run(['javac','-cp',cp,'-d',str(work),'reference-tests/Protocol777ReferenceCheck.java','reference-tests/Terrain777ReferenceCheck.java'],check=True)
r=subprocess.run(['java','-Xmx512m','-cp',str(work)+':'+cp,'Terrain777ReferenceCheck',str(corpus)],capture_output=True,text=True,timeout=60,cwd=work)
passes=[line.split('[STDOUT]:',1)[-1].strip()for line in r.stdout.splitlines()if'PASS'in line];h=lambda f:hashlib.sha256(f.read_bytes()).hexdigest()
record=dict(result='PASS'if r.returncode==0 else'FAIL',checks=passes,jar_sha256=h(root/'versions/26.3/26.3.jar'),corpus_sha256={f.name:h(f)for f in corpus.iterdir()if f.suffix in ['.wire','.nbt','.packet','.txt','.server']},scope='Actual 26.3 disk palettes and wire sections for operator-owned 16-chunk full-height converted map: supported cells preserved exactly; explicitly unsupported cells removed to air. Exact packet decode/consume/reencode. Light/game-mode packets checked, not full lighting-engine equivalence or GUI acceptance.',diagnostic=r.stderr if r.returncode else'')
a.output.write_text(json.dumps(record,indent=2)+'\n');print('\n'.join(passes));assert r.returncode==0,r.stderr
math=subprocess.run(['scripts/cargo-local.sh','run','--release','-p','smolmc-sessions','--example','terrain_math','--locked'],capture_output=True,check=True).stdout
math_path=work/'terrain-math.json';math_path.write_bytes(math)
subprocess.run(['javac','-cp',str(work)+':'+cp,'-d',str(work),'reference-tests/diagnostics/modern-26-3/FlatLevel.java','reference-tests/TerrainMechanicsReferenceCheck.java'],check=True)
r=subprocess.run(['java','-Xmx512m','-cp',str(work)+':'+cp,'TerrainMechanicsReferenceCheck',str(math_path),str(Path('crates/sessions/data/mining_tags.json').resolve())],capture_output=True,text=True,timeout=60,cwd=work)
checks=[line.split('[STDOUT]:',1)[-1].strip()for line in r.stdout.splitlines()if'PASS'in line]
mechanics=dict(result='PASS'if r.returncode==0 else'FAIL',checks=checks,corpus_sha256=hashlib.sha256(math).hexdigest(),jar_sha256=record['jar_sha256'],scope='Actual 26.3 mining progress with real item components/tag memberships/ground state, and actual entity shape clipping; 48 mining and 72 full-block collision vectors. Original flat level does not simulate all vanilla world/tick services.',diagnostic=r.stderr if r.returncode else'')
(a.mechanics_output or a.output.with_name('m4a-reference-mechanics.json')).write_text(json.dumps(mechanics,indent=2)+'\n');print('\n'.join(checks));assert r.returncode==0,r.stderr
