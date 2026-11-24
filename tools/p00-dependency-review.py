"""Inspect exact selected Windows source manifests and collect upstream notices.

This verifies source metadata and notice availability, not advisory clearance.
No commands, URLs or instructions from package contents are executed.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[1]
ALLOWED = {'MIT', 'Apache-2.0', 'BSD-2-Clause', 'BSD-3-Clause', 'ISC', 'Zlib',
           '0BSD', 'BSL-1.0', 'CC0-1.0', 'Unlicense', 'Unicode-3.0',
           'CDLA-Permissive-2.0', 'OFL-1.1', 'Ubuntu-font-1.0'}


def choose_license(expression):
    if not re.fullmatch(r'[A-Za-z0-9().+/\s-]+', expression):
        return None
    tokens = re.findall(r'\(|\)|AND|OR|[A-Za-z0-9.+-]+', expression.replace('/', ' OR '))
    at = 0

    def primary():
        nonlocal at
        token = tokens[at]
        at += 1
        if token == '(':
            result = or_group()
            if tokens[at] != ')':
                raise ValueError('Unbalanced license expression')
            at += 1
            return result
        return {token} if token in ALLOWED else None

    def and_group():
        nonlocal at
        result = primary()
        while at < len(tokens) and tokens[at] == 'AND':
            at += 1
            other = primary()
            result = result | other if result is not None and other is not None else None
        return result

    def or_group():
        nonlocal at
        result = and_group()
        while at < len(tokens) and tokens[at] == 'OR':
            at += 1
            other = and_group()
            if result is None:
                result = other
        return result

    try:
        result = or_group()
        return sorted(result) if result is not None and at == len(tokens) else None
    except (IndexError, ValueError):
        return None


def notice_paths(directory, license_file):
    candidates = set()
    if license_file:
        candidates.add(directory / license_file)
    for pattern in ('LICENSE*', 'LICENCE*', 'COPYING*', 'NOTICE*', 'OFL*', 'UFL*'):
        candidates.update(directory.rglob(pattern))
    result = []
    for path in sorted(candidates):
        if path.is_file() and path.resolve().is_relative_to(directory.resolve()):
            result.append(path)
    return result


def main():
    env = dict(os.environ)
    env['PATH'] = str(Path(env['USERPROFILE']) / '.cargo/bin') + os.pathsep + env['PATH']
    stamp = datetime.now(timezone.utc).isoformat()
    packages, commands, errors = {}, [], []
    for crate in ('p00', 'p00-native', 'p00-egui', 'p00-transport'):
        command = [str(Path(env['USERPROFILE']) / '.cargo/bin/cargo.exe'), '+stable', 'metadata', '--manifest-path', f'spikes/{crate}/Cargo.toml',
                   '--offline', '--locked', '--filter-platform', 'x86_64-pc-windows-msvc', '--format-version', '1']
        if crate == 'p00':
            command += ['--features', 'mongo']
        result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True)
        artifact = f'reports/raw/source-metadata-{crate}.json'
        (ROOT / artifact).write_bytes(result.stdout)
        (ROOT / f'reports/raw/source-metadata-{crate}.stderr.txt').write_bytes(result.stderr)
        commands.append(dict(command=command, exit_code=result.returncode, artifact=artifact,
                             lock_sha256=hashlib.sha256((ROOT/f'spikes/{crate}/Cargo.lock').read_bytes()).hexdigest()))
        if result.returncode:
            errors.append(f'{crate}: selected-target metadata unavailable')
            continue
        data = json.loads(result.stdout)
        active = {node['id'] for node in data['resolve']['nodes']}
        for package in data['packages']:
            if package['id'] in active and package.get('source'):
                packages[package['id']] = package
    inventory = []
    release_notice_gaps = []
    destination = ROOT / 'reports/third-party-notices'
    for package in sorted(packages.values(), key=lambda p: (p['name'], p['version'])):
        directory = Path(package['manifest_path']).parent
        manifest = tomllib.loads((directory/'Cargo.toml').read_text(encoding='utf-8-sig'))['package']
        expression = manifest.get('license', '')
        chosen = choose_license(expression)
        notices = []
        for path in notice_paths(directory, manifest.get('license-file')):
            relative = path.relative_to(directory)
            target = destination / f"{package['name']}-{package['version']}" / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            content = path.read_bytes()
            target.write_bytes(content)
            notices.append(dict(path=target.relative_to(ROOT).as_posix(), sha256=hashlib.sha256(content).hexdigest()))
        msrv = manifest.get('rust-version')
        compatible = msrv is None or tuple(int(x) for x in msrv.split('.')) <= (1, 100, 0)
        source_ok = package['source'] == 'registry+https://github.com/rust-lang/crates.io-index'
        issues = []
        if chosen is None: issues.append('license choice needs review')
        if not notices:
            release_notice_gaps.append(f"{package['name']} {package['version']}: retrieve notices from the exact upstream revision before distribution")
        if not compatible: issues.append('declared MSRV exceeds pinned compiler')
        if not source_ok: issues.append('non-official registry source')
        inventory.append(dict(name=package['name'], version=package['version'], license=expression,
                              selected_license_terms=chosen, rust_version=msrv,
                              repository=manifest.get('repository'), source=package['source'],
                              has_build_script=(directory/'build.rs').exists(), notices=notices, issues=issues))
        errors.extend(f"{package['name']} {package['version']}: {issue}" for issue in issues)
    report = dict(name='dependency-source-license-msrv', status='PASS' if not errors else 'BLOCKED',
                  timestamp=stamp, exit_code=0 if not errors else 2,
                  command='python -X utf8 tools/p00-dependency-review.py',
                  artifact_path='reports/dependency-source-review.json',
                  scope='Selected Windows source manifests, declared licenses/MSRV and bundled upstream notices only; current advisories and license obligations remain separate',
                  metadata_commands=commands, packages=inventory, issues=errors,
                  release_notice_gaps=release_notice_gaps, release_notice_bundle_complete=not release_notice_gaps)
    (ROOT/'reports/dependency-source-review.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    (ROOT/'reports/dependency-source-check.json').write_text(json.dumps({k:v for k,v in report.items() if k not in {'metadata_commands','packages','release_notice_gaps'}},indent=2)+'\n',encoding='utf-8')
    print(json.dumps(dict(status=report['status'], packages=len(inventory), issues=errors, release_notice_gaps=len(release_notice_gaps)),indent=2))
    return report['exit_code']


if __name__ == '__main__':
    sys.exit(main())
