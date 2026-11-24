"""Check current slice evidence consistency, not full phase acceptance."""
import hashlib,json,re
from pathlib import Path
from datetime import datetime,timezone
ROOT=Path(__file__).resolve().parents[1]
def read(p):return json.loads((ROOT/p).read_text(encoding='utf-8-sig'))
checks=read('reports/p01-checks.json')
assert len(checks)==6 and all(c['status']=='PASS' and c['exit_code']==0 for c in checks)
log=(ROOT/'reports/raw/p01-unit.txt').read_text(encoding='utf-8-sig')
counts=re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored',log)
assert sum(map(int,counts))==22
assert 'real_dpapi_persistence_binding_and_plaintext_absence ... ok' in log
assert 'PASS: product workspace adapter recovered acknowledged revision' in (ROOT/'reports/raw/p01-process-crash.txt').read_text(encoding='utf-8-sig')
ui=read('reports/p01-ui-binary.json');exe=ROOT/ui['path']
assert hashlib.sha256(exe.read_bytes()).hexdigest()==ui['sha256'],'Observed binary was replaced'
build=next(c for c in checks if c['name']=='p01-build')
built_at=datetime.fromisoformat(build['timestamp']).timestamp()
for folder in ('apps','crates'):
 for p in (ROOT/folder).rglob('*.rs'):
  assert p.stat().st_mtime<=built_at,'Source changed after build: '+p.name
security=read('reports/p01-dependency-check.json');assert security['status']=='PASS'
for name,digest in security['input_sha256'].items():
 assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==digest,'Dependency inputs changed'
meta=read('reports/raw/p01-metadata.json');ids={n['id'] for n in meta['resolve']['nodes']}
assert not any(p['name']=='mongodb' and p['id'] in ids for p in meta['packages']),'MongoDB driver in desktop graph'
assert all(c['status']=='PASS' for c in read('reports/p01-ui-checks.json'))
progress=read('reports/p01-progress.json');assert progress['status']=='in_progress' and progress['phase_gate_passed'] is False
req=read('reports/requirement-ledger.json');screens=read('reports/screen-ledger.json')
assert len(req)==160 and len(screens)==316
assert not any(r['status']=='verified' for r in req+screens)
assert sum(r['status']=='in_progress' for r in req)==5
result=dict(name='p01-evidence-consistency',status='PASS',timestamp=datetime.now(timezone.utc).isoformat(),exit_code=0,scope='Current slice only; P01 remains in_progress, no full requirement or phase acceptance')
(ROOT/'reports/p01-evidence-audit.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8');print(json.dumps(result,indent=2))
