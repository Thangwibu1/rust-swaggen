"""Bind the observed crash-matrix command to its scoped fixture CSV and current code hashes."""
import csv
import hashlib
import io
import json
import re
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
raw = (ROOT / 'reports/raw/journal-crash-matrix.txt').read_text(encoding='utf-8-sig')
match = re.search(r'^Evidence:\s*(.+)$', raw, re.MULTILINE)
assert match, 'No successful crash-matrix artifact was emitted'
artifact = Path(match.group(1).strip()).resolve()
assert artifact.is_relative_to((ROOT / '.local/p00-crash-matrix').resolve())
text = artifact.read_text(encoding='utf-8')
rows = list(csv.DictReader(io.StringIO(text)))
assert [r['case'] for r in rows] == ['created', 'partial', 'written', 'synced', 'replaced', 'acked']
assert all(r['status'] == 'PASS' for r in rows)
for row in rows:
    replaced = row['case'] in {'replaced', 'acked'}
    assert int(row['recovered_batches']) == (1 if replaced else 2)
    assert int(row['orphan_files']) == (0 if replaced else 1)
(ROOT / 'reports/compaction-crash-results.csv').write_text(text, encoding='utf-8')
sources = list((ROOT / 'spikes/p00/src').rglob('*.rs'))
sources += list((ROOT / 'spikes/p00-native/src').rglob('*.rs'))
sources += [ROOT / 'rust-toolchain.toml', ROOT / 'spikes/p00/Cargo.toml', ROOT / 'spikes/p00/Cargo.lock',
            ROOT / 'spikes/p00-native/Cargo.toml', ROOT / 'spikes/p00-native/Cargo.lock', ROOT / 'tools/p00-check.ps1']
evidence = {
    'timestamp': datetime.now(timezone.utc).isoformat(),
    'command': 'python -X utf8 tools/p00-durability-evidence.py', 'exit_code': 0, 'status': 'PASS',
    'scope': 'Six completed process-crash cases and artifact consistency; no power-loss acceptance',
    'fixture_artifact': artifact.relative_to(ROOT).as_posix(), 'cases': rows,
    'source_hashes_observed_at_review': [
        {'path': p.relative_to(ROOT).as_posix(), 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()}
        for p in sorted(sources)
    ],
    'hash_limit': 'Current review-time fingerprints, not a Git diff or retroactive hashes for earlier UI observations.'
}
(ROOT / 'reports/durability-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'status': 'PASS', 'crash_cases': len(rows), 'source_files_fingerprinted': len(sources)}, indent=2))
