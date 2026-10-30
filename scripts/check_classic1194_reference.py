#!/usr/bin/env python3
"""Optional numeric interoperability with an operator-owned Paper 1.19.4 JAR.
No download, auth, server launch, EULA acceptance or assets copied into repo.
"""
import argparse,hashlib,json,subprocess
from pathlib import Path

def main():
 p=argparse.ArgumentParser();p.add_argument('--jar',type=Path,default=Path('.tools/m3/paper1194-run/versions/1.19.4/paper-1.19.4.jar'));p.add_argument('--libraries',type=Path);p.add_argument('--output',type=Path,default=Path('evidence/m3-classic1194-reference.json'));a=p.parse_args()
 jar=a.jar.resolve();libs=a.libraries or jar.parents[2]/'libraries';assert jar.is_file()
 cp=':'.join(map(str,[jar,*sorted(libs.rglob('*.jar'))]));work=Path('target/reference-classic1194').resolve();work.mkdir(exist_ok=True)
 math=subprocess.run(['scripts/cargo-local.sh','run','--release','-p','smolmc-sessions','--example','pvp_math','--locked'],capture_output=True,check=True).stdout
 path=work/'corpus.json';path.write_bytes(math)
 subprocess.run(['javac','-cp',cp,'-d',str(work),'reference-tests/Classic1194ReferenceCheck.java'],check=True)
 result=subprocess.run(['java','-Xmx256m','-cp',str(work)+':'+cp,'Classic1194ReferenceCheck',str(path)],capture_output=True,text=True,timeout=30,cwd=work)
 record=dict(result='PASS'if result.returncode==0 else'FAIL',reference='Paper 1.19.4 build 550 Mojang-mapped',jar_sha256=hashlib.sha256(jar.read_bytes()).hexdigest(),corpus_sha256=hashlib.sha256(math).hexdigest(),checks=[l for l in result.stdout.splitlines()if'PASS'in l],scope='Actual historical Mth and velocity packet implementation only. Knockback equations independently inspected in hash-pinned private vanilla/Paper source. Grounded-contact settling is explicit local policy, not a claim of bitwise Paper server simulation.',exit_code=result.returncode,diagnostic=result.stderr if result.returncode else'')
 a.output.write_text(json.dumps(record,indent=2)+'\n');print(result.stdout);assert result.returncode==0,result.stderr
if __name__=='__main__':main()
