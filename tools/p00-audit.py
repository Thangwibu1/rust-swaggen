"""Audit original lockfiles and Cargo-resolved Windows closures, without ignores."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
TARGET = 'x86_64-pc-windows-msvc'
SCOPES = ('p00', 'p00-native', 'p00-egui', 'p00-transport')
SELECTED = ('p00', 'p00-egui', 'p00-transport')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def project_lock(lock_text, metadata):
    """Keep every reachable package, including build/proc-macro dependencies.

    Cargo metadata determines membership. Preserve source/checksum/version and
    validate every dependency against the original lock. Audit input only.
    """
    lock = tomllib.loads(lock_text)
    packages = {p['id']: p for p in metadata['packages']}
    nodes = {n['id']: n for n in metadata['resolve']['nodes']}
    pending = list(metadata['workspace_members'])
    reached = set()
    while pending:
        key = pending.pop()
        if key not in reached:
            reached.add(key)
            pending.extend(nodes[key]['dependencies'])
    if reached != set(nodes):
        raise ValueError('Unexpected disconnected resolved packages')
    identity = lambda p: (p['name'], p['version'], p.get('source'))
    original = {identity(p): p for p in lock['package']}
    if len(original) != len(lock['package']):
        raise ValueError('Ambiguous lock package identity')
    output = ['# Generated audit input only; build with original Cargo.lock.', 'version = 4', '']
    for key in sorted(reached):
        package = packages[key]
        source = original[identity(package)]
        deps = []
        for dep_key in nodes[key]['dependencies']:
            dep = packages[dep_key]
            candidates = {dep['name'], f"{dep['name']} {dep['version']}"}
            explicit = f"{dep['name']} {dep['version']}"
            if dep.get('source'):
                explicit += f" ({dep['source']})"
            candidates.add(explicit)
            if not candidates.intersection(source.get('dependencies', [])):
                raise ValueError(f"Resolved edge missing from lock: {package['name']} -> {explicit}")
            deps.append(explicit)
        output.append('[[package]]')
        for field in ('name', 'version', 'source', 'checksum'):
            if field in source:
                output.append(f'{field} = {json.dumps(source[field])}')
        if deps:
            output.append('dependencies = ' + json.dumps(sorted(deps)))
        output.append('')
    return '\n'.join(output)


def validate_windows_evidence(root, record):
    """Reject changed build inputs and tampered or incomplete projections."""
    try:
        scope = record['name'].removeprefix('rustsec-windows-')
        if scope not in SCOPES or record['target'] != TARGET:
            return False
        required = {f'spikes/{name}/Cargo.toml' for name in SCOPES}
        required |= {f'spikes/{scope}/Cargo.lock', record['metadata_path']}
        if set(record['input_sha256']) != required:
            return False
        for path, checksum in record['input_sha256'].items():
            if not (root/path).resolve().is_relative_to(root.resolve()) or sha(root/path) != checksum:
                return False
        lock = root/'spikes'/scope/'Cargo.lock'
        if sha(lock) != record['lock_sha256']:
            return False
        metadata = json.loads((root/record['metadata_path']).read_text(encoding='utf-8-sig'))
        expected = project_lock(lock.read_text(encoding='utf-8-sig'), metadata)
        projection = root/record['projection_path']
        report = json.loads((root/record['artifact_path']).read_text(encoding='utf-8-sig'))
        return (projection.read_text(encoding='utf-8') == expected
                and sha(projection) == record['projection_sha256']
                and sha(root/record['artifact_path']) == record['artifact_sha256']
                and report['vulnerabilities']['found'] is False
                and not any(report.get('warnings', {}).values())
                and report['settings']['ignore'] == []
                and report['database'] == record['database'])
    except (KeyError, ValueError, OSError, TypeError):
        return False


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--no-fetch', action='store_true', help='Use local DB only when latest commit is under 48 hours old')
    args = parser.parse_args()
    tool = ROOT/'.local/p00-audit-tools/bin/cargo-audit.exe'
    database = ROOT/'.local/p00-advisory-db'
    preflight_error = None
    try:
        version = subprocess.run([str(tool), '--version'], capture_output=True, text=True, check=True)
        if version.stdout.strip() != 'cargo-audit 0.23.0':
            raise ValueError('Pinned cargo-audit 0.23.0 is required')
        if args.no_fetch:
            result = subprocess.run(['git', '-c', f'safe.directory={database.as_posix()}', '-C', str(database),
                                     'log', '-1', '--format=%cI'], capture_output=True, text=True, check=True)
            age = (datetime.now(timezone.utc)-datetime.fromisoformat(result.stdout.strip())).total_seconds()
            if not 0 <= age <= 48*3600:
                raise ValueError('Local advisory DB must have a commit less than 48 hours old; fetch current data')
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        preflight_error = str(error)
    records = []
    for scope in SCOPES:
        lock = ROOT/'spikes'/scope/'Cargo.lock'
        for windows in (False, True):
            label = ('windows-' if windows else '') + scope
            record = dict(name='rustsec-'+label, status='BLOCKED', exit_code=None,
                          timestamp=datetime.now(timezone.utc).isoformat(), lock_sha256=sha(lock))
            try:
                if preflight_error:
                    raise ValueError(preflight_error)
                audit_lock = lock
                if windows:
                    cargo = Path(os.environ['USERPROFILE'])/'.cargo/bin/cargo.exe'
                    command = [str(cargo), '+stable', 'metadata', '--manifest-path', f'spikes/{scope}/Cargo.toml',
                               '--offline', '--locked', '--filter-platform', TARGET, '--format-version', '1']
                    if scope == 'p00':
                        command += ['--features', 'mongo']
                    meta = subprocess.run(command, cwd=ROOT, capture_output=True, check=True)
                    meta_path = f'reports/raw/audit-metadata-{scope}.json'
                    (ROOT/meta_path).write_bytes(meta.stdout)
                    data = json.loads(meta.stdout)
                    audit_lock = ROOT/f'reports/raw/audit-windows-{scope}.lock'
                    audit_lock.write_text(project_lock(lock.read_text(encoding='utf-8-sig'), data), encoding='utf-8', newline='\n')
                    inputs = {f'spikes/{name}/Cargo.toml': sha(ROOT/f'spikes/{name}/Cargo.toml') for name in SCOPES}
                    inputs[f'spikes/{scope}/Cargo.lock'] = sha(lock)
                    inputs[meta_path] = sha(ROOT/meta_path)
                    record.update(target=TARGET, metadata_path=meta_path, metadata_command=command,
                                  projection_path=audit_lock.relative_to(ROOT).as_posix(),
                                  projection_sha256=sha(audit_lock), input_sha256=inputs,
                                  resolved_packages=len(data['resolve']['nodes']))
                command = [str(tool), 'audit', '--json', '--file', str(audit_lock), '--db', str(database), '--deny', 'warnings']
                if args.no_fetch:
                    command.append('--no-fetch')
                record['command'] = subprocess.list2cmdline(command)
                result = subprocess.run(command, cwd=ROOT, capture_output=True)
                artifact = f'reports/raw/audit-{label}.json'
                stderr = f'reports/raw/audit-{label}.stderr.txt'
                (ROOT/artifact).write_bytes(result.stdout)
                (ROOT/stderr).write_bytes(result.stderr)
                record.update(exit_code=result.returncode, artifact_path=artifact, stderr_path=stderr,
                              status='PASS' if result.returncode == 0 else 'FAIL' if result.returncode == 1 else 'BLOCKED',
                              detail='Warnings denied; no advisory ignores. ' + ('Complete Windows dependency closure.' if windows else 'Original cross-platform lock; retained even for rejected candidates.'))
                if result.returncode == 0:
                    report = json.loads(result.stdout)
                    if (report['vulnerabilities']['found'] or any(report.get('warnings', {}).values())
                            or report['settings']['ignore']):
                        raise ValueError('Unexpected vulnerable, ignored or warning-bearing clean result')
                    record['database'] = report['database']
                    record['artifact_sha256'] = sha(ROOT/artifact)
            except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
                record.update(status='BLOCKED', detail=str(error))
            records.append(record)
            print(f"{label}: {record['status']} exit={record['exit_code']}", flush=True)
    (ROOT/'reports/security-checks.json').write_text(json.dumps(records, indent=2)+'\n', encoding='utf-8')
    selected = {'rustsec-windows-'+scope for scope in SELECTED}
    passed = all(r['status']=='PASS' for r in records if r['name'] in selected)
    print('Adopted Windows security gate: ' + ('PASS' if passed else 'BLOCKED')
          + '; original cross-platform and rejected Iced results retained.', flush=True)
    return 0 if passed else 2


if __name__ == '__main__':
    sys.exit(main())
