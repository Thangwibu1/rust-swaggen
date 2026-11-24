"""Audit the actual product graph; inherited P00 reports are not product audits."""
import hashlib, importlib.util, json, os, subprocess, sys
from pathlib import Path
from datetime import datetime, timezone
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('audit',ROOT/'tools/p00-audit.py')
audit=importlib.util.module_from_spec(spec);spec.loader.exec_module(audit)
record=dict(name='p01-dependencies',status='BLOCKED',timestamp=datetime.now(timezone.utc).isoformat(),exit_code=2)
try:
    db=ROOT/'.local/p00-advisory-db'
    date=subprocess.check_output(['git','-c',f'safe.directory={db.as_posix()}','-C',str(db),'log','-1','--format=%cI'],text=True).strip()
    assert 0<=(datetime.now(timezone.utc)-datetime.fromisoformat(date)).total_seconds()<=48*3600,'Current advisory database required'
    tool=ROOT/'.local/p00-audit-tools/bin/cargo-audit.exe'
    assert subprocess.check_output([str(tool),'--version'],text=True).strip()=='cargo-audit 0.23.0'
    command=[str(Path(os.environ['USERPROFILE'])/'.cargo/bin/cargo.exe'),'+stable','metadata','--offline','--locked','--filter-platform',audit.TARGET,'--format-version','1']
    result=subprocess.run(command,cwd=ROOT,capture_output=True,check=True)
    (ROOT/'reports/raw/p01-metadata.json').write_bytes(result.stdout)
    metadata=json.loads(result.stdout);active={n['id'] for n in metadata['resolve']['nodes']}
    current={(p['name'],p['version']) for p in metadata['packages'] if p['id'] in active and p.get('source')}
    prior=json.loads((ROOT/'reports/dependency-source-review.json').read_text(encoding='utf-8-sig'))
    reviewed={(p['name'],p['version']) for p in prior['packages']}
    assert not current-reviewed,'New dependency versions need source/license/MSRV review: '+str(sorted(current-reviewed))
    # New Win32 features are covered by versioned-source review and actual DPAPI tests.
    lock=ROOT/'reports/raw/p01-windows.lock'
    lock.write_text(audit.project_lock((ROOT/'Cargo.lock').read_text(),metadata),encoding='utf-8',newline='\n')
    command=[str(tool),'audit','--json','--file',str(lock),'--db',str(db),'--deny','warnings','--no-fetch']
    result=subprocess.run(command,cwd=ROOT,capture_output=True)
    (ROOT/'reports/raw/p01-audit.json').write_bytes(result.stdout)
    (ROOT/'reports/raw/p01-audit.stderr.txt').write_bytes(result.stderr)
    data=json.loads(result.stdout)
    assert result.returncode==0 and not data['vulnerabilities']['found'] and not any(data['warnings'].values()) and not data['settings']['ignore'],'Product graph advisory audit failed'
    paths=[ROOT/'Cargo.lock',ROOT/'Cargo.toml']+list((ROOT/'apps').glob('*/Cargo.toml'))+list((ROOT/'crates').glob('*/Cargo.toml'))
    record.update(status='PASS',exit_code=0,command=subprocess.list2cmdline(command),artifact_path='reports/raw/p01-audit.json',windows_packages=len(current),database=data['database'],database_git_commit=subprocess.check_output(['git','-c',f'safe.directory={db.as_posix()}','-C',str(db),'rev-parse','HEAD'],text=True).strip(),database_commit_date=date,
        input_sha256={p.relative_to(ROOT).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in paths},
        detail='Complete Windows graph with warnings denied and no advisory ignores; all exact external versions already source-reviewed in P00. New DPAPI API features reviewed from windows-sys 0.61.2 and Microsoft documentation.')
except (AssertionError,ValueError,OSError,subprocess.CalledProcessError) as error:
    record['detail']=str(error)
(ROOT/'reports/p01-dependency-check.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
print(json.dumps(record,indent=2));sys.exit(record['exit_code'])
