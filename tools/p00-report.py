"""Summarize observed evidence only. Does not promote acceptance gates automatically."""
import json
import hashlib
import importlib.util
import re
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPORTS = ROOT / 'reports'
_spec = importlib.util.spec_from_file_location('p00_gate', Path(__file__).with_name('p00-gate.py'))
_gate = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_gate)
_audit_spec = importlib.util.spec_from_file_location('p00_audit', Path(__file__).with_name('p00-audit.py'))
_audit = importlib.util.module_from_spec(_audit_spec)
_audit_spec.loader.exec_module(_audit)

def read(path, default=None):
    p = ROOT / path
    return json.loads(p.read_text(encoding='utf-8-sig')) if p.exists() and p.stat().st_size else default

def write(path, data):
    (ROOT / path).write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')

def records(path):
    value = read(path, [])
    return value if isinstance(value, list) else [value] if value else []

def latest_by_name(checks):
    selected = {}
    for check in checks:
        stamp = datetime.fromisoformat(check['timestamp'].replace('Z', '+00:00'))
        prior = selected.get(check['name'])
        # Older hand-recorded UI observations carry dates only. Preserve their
        # actual precision when unique; never invent a timezone to order duplicates.
        if prior is not None and (stamp.tzinfo is None or prior[0].tzinfo is None):
            raise ValueError('Duplicate evidence timestamps must include their timezones')
        if prior is None or stamp >= prior[0]:
            selected[check['name']] = (stamp, check)
    return [entry[1] for entry in selected.values()]

def verify_current_lock_evidence(checks):
    allowed = {'p00', 'p00-native', 'p00-egui', 'p00-transport'}
    for check in checks:
        if check['name'].startswith('rustsec-windows-') and check['status'] == 'PASS':
            if not _audit.validate_windows_evidence(ROOT, check):
                check.update(status='BLOCKED', detail='Windows audit projection/build inputs changed or are incomplete; rerun audit')
        elif check['name'].startswith('rustsec-') and check['status'] == 'PASS':
            crate = check['name'].removeprefix('rustsec-')
            lock = ROOT / 'spikes' / crate / 'Cargo.lock' if crate in allowed else None
            if lock is None or not lock.is_file() or hashlib.sha256(lock.read_bytes()).hexdigest() != check.get('lock_sha256'):
                check.update(status='BLOCKED', detail='Advisory evidence does not match the current lockfile')
        if check['name'] == 'dependency-source-license-msrv' and check['status'] == 'PASS':
            review = read('reports/dependency-source-review.json', {})
            reviewed = {}
            for entry in review.get('metadata_commands', []):
                args = entry.get('command', [])
                if '--manifest-path' in args:
                    crate = Path(args[args.index('--manifest-path')+1]).parent.name
                    if crate in allowed:
                        reviewed[crate] = entry.get('lock_sha256')
            if set(reviewed) != allowed or any(not (ROOT/'spikes'/name/'Cargo.lock').exists() or hashlib.sha256((ROOT/'spikes'/name/'Cargo.lock').read_bytes()).hexdigest()!=checksum for name,checksum in reviewed.items()):
                check.update(status='BLOCKED', detail='Source review does not cover the current lockfiles; rerun p00-dependency-review.py')

def main():
    now = datetime.now(timezone.utc).isoformat()
    checks = records('reports/p00-checks.json')
    checks += read('reports/network-checks.json', [])
    installs = read('reports/dependency-install.json', [])
    checks += installs if isinstance(installs, list) else [installs]
    for report in ('reports/software-checks.json', 'reports/mongo-diagnostics-checks.json',
                   'reports/egui-checks.json', 'reports/egui-ui-checks.json',
                   'reports/evidence-regression-checks.json', 'reports/security-checks.json',
                   'reports/dependency-source-check.json', 'reports/p00-completion-ui-checks.json',
                   'reports/p00-decision-checks.json', 'reports/host-check.json', 'reports/idle-observation.json'):
        additional = read(report, [])
        checks += additional if isinstance(additional, list) else [additional]
    software = read('reports/software-renderer-evidence.json')
    if software:
        checks.append(software)
    default_check = read('reports/native-default-check.json')
    if default_check:
        checks.append(default_check)
    checks += read('reports/software-ui-checks.json', [])
    checks += read('reports/native-ui-checks.json', [])
    artifact = read('reports/native-ui-http-artifact.json')
    if artifact:
        checks.append(artifact)
    checks = latest_by_name(records('reports/mongo-check.json') + checks)
    mongo = next((c for c in checks if c['name'] == 'mongo-runtime'), None)
    presence = read('reports/doctor.json', [])
    unit = next((c for c in checks if c['name'] == 'journal-unit' and c['status'] == 'PASS'), None)
    if unit:
        for c in presence:
            if c['name'] == 'MSVC link smoke test':
                c.update(status='PASS', detail='Native test executable linked and executed; GUI validation remains separate',
                         command=unit['command'], exit_code=unit['exit_code'],
                         timestamp=unit['timestamp'], artifact_path=unit['artifact_path'])
    for c in presence:
        if c['name'] == 'Windows GUI/IME/DPI/Narrator' and (REPORTS / 'native-ui-checks.json').exists():
            c.update(detail='Native rendering and selected interactions observed; internal accessibility controls absent; IME/Narrator/OS DPI still pending', artifact_path='reports/native-ui.md')
    if mongo and mongo['status'] == 'BLOCKED_OR_FAILED':
        mongo['status'] = 'FAIL'
    checks = presence + checks
    verify_current_lock_evidence(checks)
    # P00 spikes are evidence for a gate, not completed product requirements.
    trace = read('04-phases/traceability.json')
    req_ledger = [{'requirement_id': r['id'], 'phase': r['phase'], 'status': r['status'],
                   'code_paths': [], 'screen_ids': r['screen_ids'], 'test_evidence': [],
                   'benchmark': None, 'commit': None, 'blockers': ['P00 gate not passed'],
                   'last_updated': now} for r in trace]
    target = ROOT / 'reports/requirement-ledger.json'
    # Never reset a later implementation ledger on rerun.
    if not target.exists():
        write('reports/requirement-ledger.json', req_ledger)
    screens = read('02-design/screen-requirement-map.json')
    if not (REPORTS / 'screen-ledger.json').exists():
        write('reports/screen-ledger.json', [{'screen_id': s['id'], 'requirements': s['requirements'],
            'source_path': s['path'], 'source_png_exists': (ROOT / (s['path'] + '.png')).exists(),
            'status': 'not_started', 'native_evidence': [], 'disposition': 'pending P00 and implementation'} for s in screens])
    metadata = read('reports/raw/cargo-metadata.json', {})
    native_metadata = read('reports/raw/native-metadata.json', {})
    egui_metadata = read('reports/raw/egui-metadata.txt', {})
    inventory = []
    seen = set()
    for p in metadata.get('packages', []) + native_metadata.get('packages', []) + egui_metadata.get('packages', []):
        if not p.get('source'):
            continue
        key = (p['name'], p['version'])
        if key in seen:
            continue
        seen.add(key)
        inventory.append({k: p.get(k) for k in ('name', 'version', 'license', 'rust_version', 'repository', 'source')})
    write('reports/dependency-inventory.json', sorted(inventory, key=lambda p: p['name']))
    source_review = read('reports/dependency-source-review.json')
    if source_review:
        inventory = [{k:p.get(k) for k in ('name','version','license','rust_version','repository','source')} for p in source_review['packages']]
        write('reports/dependency-inventory.json', inventory)
    gate = _gate.evaluate(checks)
    gate['generated_at'] = now
    write('reports/p00-gate.json', gate)
    for check in checks:
        if check['name'] == 'Windows GUI/IME/DPI/Narrator':
            selected_ui = ['p00-editor-highlight-search', 'p00-keyboard-modal', 'p00-telex-input', 'p00-native-resize-dpi', 'p00-narrator']
            check.update(status='PASS' if all(any(c['name']==name and c['status']=='PASS' for c in checks) for name in selected_ui) else 'BLOCKED',
                         detail='Selected prototype smoke only; exact scope in p00-final-native.md', artifact_path='reports/p00-final-native.md')
        if check['name'] == 'Dependency compatibility and licenses':
            selected_deps = ['dependency-source-license-msrv'] + _gate.CRITERIA['Current advisory audit']
            check.update(status='PASS' if all(any(c['name']==name and c['status']=='PASS' for c in checks) for name in selected_deps) else 'BLOCKED',
                         detail='Declared source license/MSRV policy and adopted Windows graph advisory audit; cross-platform/rejected Iced failures retained. See p00-security-disposition.md')
    blockers = [c['criterion'] + ': ' + ', '.join(c['unresolved_checks']) for c in gate['criteria'] if c['status'] != 'PASS']
    deferred = [
        'Full product keyboard/accessibility/DPI matrices, multiple monitors, automatic device-loss recovery and release performance budgets are phase-specific; selected P00 smoke observations do not certify them.',
        'Production store schema migrations, workspace scoping and physical disk/power-loss qualification remain separate from the observed process-crash and OS-flush contract.',
        'Some upstream published crates omit notice files; the release notice bundle must be completed from exact upstream revisions before distribution. See dependency-source-review.json.',
        '464 design renders missing; no full visual parity claim.',
        'No git repository detected; no commit evidence.',
    ]
    summary = {'generated_at': now, 'phase': 'P00', 'status': 'passed' if gate['gate_passed'] else 'blocked', 'gate_passed': gate['gate_passed'],
        'product_scaffold_created': (ROOT / 'apps/desktop/Cargo.toml').is_file(), 'docker_used': False, 'checks': checks, 'blockers': blockers,
        'deferred_release_or_product_checks': deferred,
        'product_requirements_verified': 0, 'product_screens_verified': 0}
    write('reports/preflight.json', summary)
    lines = ['# P00 preflight - ' + ('PASS' if gate['gate_passed'] else 'BLOCKED'), '', f'Generated: {now}', '',
        'P00 Windows native UI, local storage and controlled-transfer spike evidence only. Current implementation phase is recorded separately in 04-phases/status.md.', '',
        '| Foundation criterion | Result |', '| --- | --- |']
    lines += [f"| {c['criterion']} | {c['status']} |" for c in gate['criteria']]
    lines += ['', '## Raw observations, including historical rejected candidates', '',
        '| Check | Status | Exit | Evidence / command |', '| --- | --- | --- | --- |']
    for c in checks:
        detail = c.get('artifact_path') or c.get('command') or c.get('detail', '')
        detail = detail.replace('|', '\\|').replace('\n', ' ')
        lines.append(f"| {c['name']} | {c['status']} | {c.get('exit_code')} | {detail} |")
    if mongo and mongo['status'] == 'PASS':
        lines += ['', 'Mongo isolated commit/rollback probe passed. Product publish/idempotency/revoke-race tests remain phase-specific; this successful fixture is not a P00 blocker.']
    lines += ['', '## Acceptance blockers', ''] + ['- ' + b for b in blockers]
    lines += ['', '## Later phase evidence still required', ''] + ['- ' + b for b in deferred]
    lines += ['', '## Limits of the evidence', '',
        'Journal unit and process-crash tests do not establish power-loss safety or a production-ready store. '
        'Driver compilation does not establish Mongo connectivity or transaction correctness. '
        'Tool/SDK presence does not establish native GUI accessibility, IME, DPI or performance.', '',
        'No Docker/Compose commands were executed. No existing application server, user database, production service or replica-set configuration was changed. Controlled loopback HTTP fixtures are isolated test servers. '
        'Mongo credentials were process-local and are absent from source and reports.', '',
        'See dependency-matrix.md, ADR-0001-ui.md, ADR-0002-local-store.md, transaction-smoke.md and checkpoint.md.']
    (REPORTS / 'preflight.md').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    phase_path = ROOT / '04-phases/status.md'
    if phase_path.exists() and all(r['status']=='not_started' for r in read('reports/requirement-ledger.json', [])):
        phase = phase_path.read_text(encoding='utf-8')
        label = 'passed foundation gate' if gate['gate_passed'] else 'blocked on remaining foundation evidence'
        phase = re.sub(r'^P00:.*$', f'P00: **{label}**. Current criteria: reports/p00-gate.json and reports/preflight.md.', phase, flags=re.M)
        phase = re.sub(r'^\| P00 \|.*$', '| P00 | '+('passed' if gate['gate_passed'] else 'blocked')+' | '+('; '.join(blockers) if blockers else 'Required foundation evidence passed; no product requirements verified')+' |', phase, flags=re.M)
        phase_path.write_text(phase, encoding='utf-8')
    print(json.dumps({'gate_passed': gate['gate_passed'], 'checks': len(checks), 'dependency_inventory': len(inventory),
                      'requirements': len(req_ledger), 'screens': len(screens)}, indent=2))

if __name__ == '__main__':
    main()
