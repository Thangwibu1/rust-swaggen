"""Reproducible Windows foundation checks; no DB operation or UI acceptance."""
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import sys
import time
ROOT = Path(__file__).resolve().parents[1]
cargo = str(Path(os.environ['USERPROFILE'])/'.cargo/bin/cargo.exe')
rustc = str(Path(os.environ['USERPROFILE'])/'.cargo/bin/rustc.exe')
records=[]
def run(name, command):
    started=time.monotonic()
    result=subprocess.run(command,cwd=ROOT,capture_output=True)
    artifact='reports/raw/p01-'+name+'.txt'
    (ROOT/artifact).write_bytes(result.stdout+result.stderr)
    status='PASS' if result.returncode==0 else 'FAIL'
    records.append(dict(name='p01-'+name,status=status,exit_code=result.returncode,
        timestamp=datetime.now(timezone.utc).isoformat(),duration_seconds=time.monotonic()-started,
        command=subprocess.list2cmdline(command),artifact_path=artifact))
    print(name+': '+status,flush=True)
    return result
version=run('toolchain',[rustc,'+stable','--version'])
if sys.platform!='win32' or not version.stdout.startswith(b'rustc 1.100.0 '):
    records[-1].update(status='BLOCKED',detail='Windows and exact Rust 1.100.0 required')
else:
    for name,args in [
        ('format',['fmt','--all','--','--check']),
        ('unit',['test','--workspace','--all-targets','--offline','--locked','--target','x86_64-pc-windows-msvc']),
        ('clippy',['clippy','--workspace','--all-targets','--offline','--locked','--target','x86_64-pc-windows-msvc','--','-D','warnings']),
        ('build',['build','-p','swaggen-desktop','--offline','--locked','--target','x86_64-pc-windows-msvc']),
        ('process-crash',['run','-p','swaggen-local-store','--bin','store-crash-probe','--offline','--locked','--target','x86_64-pc-windows-msvc']),
    ]:
        result=run(name,[cargo,'+stable']+args)
        if result.returncode:
            print(result.stderr.decode('utf-8',errors='replace')[-6000:],flush=True)
            break
(ROOT/'reports/p01-checks.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8')
sys.exit(0 if len(records)==6 and all(r['status']=='PASS' for r in records) else 2)
