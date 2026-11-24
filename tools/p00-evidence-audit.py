"""Check handoff consistency and absence of credential-bearing Mongo URIs in new files."""
import json
import re
from datetime import datetime, timezone
from pathlib import Path

root = Path(__file__).resolve().parents[1]

def read(path):
    return json.loads((root / path).read_text(encoding='utf-8-sig'))

summary = read('reports/preflight.json')
summary_stamp = datetime.fromisoformat(summary['generated_at']).timestamp()
for name in ('p00-checks.json', 'egui-checks.json', 'egui-ui-checks.json', 'evidence-regression-checks.json',
             'security-checks.json', 'dependency-source-check.json', 'p00-completion-ui-checks.json', 'p00-decision-checks.json', 'host-check.json'):
    path = root / 'reports' / name
    if path.exists():
        assert path.stat().st_mtime <= summary_stamp, f'Summary predates {name}; regenerate it before auditing'
        source = read('reports/' + name)
        source = source if isinstance(source, list) else [source]
        assert {c['name'] for c in source} <= {c['name'] for c in summary['checks']}, f'Missing evidence scopes from {name}'
assert summary['gate_passed'] == read('reports/p00-gate.json')['gate_passed']
assert summary['docker_used'] is False
assert summary['product_scaffold_created'] == (root / 'apps/desktop/Cargo.toml').is_file()
assert len(read('reports/requirement-ledger.json')) == 160
assert len(read('reports/screen-ledger.json')) == 316
assert all(r['status'] in {'not_started', 'in_progress', 'implemented_unverified', 'verified', 'blocked'} for r in read('reports/requirement-ledger.json'))
assert re.search(r'test result: ok\. \d+ passed; 0 failed; 0 ignored', (root / 'reports/raw/journal-unit.txt').read_text(encoding='utf-8-sig'))
for check in read('reports/p00-checks.json'):
    assert (check['status'] == 'PASS') == (check['exit_code'] == 0)
assert read('reports/mongo-check.json')['exit_code'] in {0, 2}
assert all(c['status'] in {'PASS', 'FAIL', 'BLOCKED', 'NOT_APPLICABLE'} for c in summary['checks'])
credential_uri = re.compile(r'mongodb(?:\+srv)?://[^\s/]+:[^\s/]+@', re.I)
paths = list((root / 'reports').rglob('*'))
paths += list((root / 'spikes/p00/src').rglob('*'))
paths += list((root / 'spikes/p00-native/src').rglob('*'))
paths += list((root / 'spikes/p00-egui/src').rglob('*'))
paths += list((root / 'spikes/p00-transport/src').rglob('*'))
paths += list((root / 'tools').glob('p00-*'))
paths += list((root / 'tools').glob('p01-*'))
paths += list((root / 'apps').rglob('*.rs'))
paths += list((root / 'crates').rglob('*.rs'))
for path in paths:
    if path.is_file() and path.suffix in {'.py', '.ps1', '.rs', '.json', '.md', '.txt', '.toml'}:
        assert not credential_uri.search(path.read_text(encoding='utf-8-sig')), f'Credential-bearing URI found in {path.name}'
evidence = {'status': 'PASS', 'scope': 'handoff consistency and credential-URI scan only; no product acceptance',
            'timestamp': datetime.now(timezone.utc).isoformat(), 'command': 'python -X utf8 tools/p00-evidence-audit.py', 'exit_code': 0}
(root / 'reports/evidence-audit.json').write_text(json.dumps(evidence, indent=2) + '\n', encoding='utf-8')
print(json.dumps(evidence, indent=2))
