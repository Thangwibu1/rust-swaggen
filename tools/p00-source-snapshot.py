"""Capture review-time source fingerprints for the next continuation; not test acceptance."""
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path

root = Path(__file__).resolve().parents[1]
paths = set()
for crate in (root / 'spikes').iterdir():
    if not crate.is_dir():
        continue
    paths.update((crate / 'src').rglob('*.rs'))
    for name in ('Cargo.toml', 'Cargo.lock'):
        if (crate / name).is_file():
            paths.add(crate / name)
paths.update((root / 'tools').glob('p00-*.py'))
paths.update((root / 'tools').glob('p00-*.ps1'))
paths.update((root / 'tools/tests').glob('test_p00*.py'))
paths.update((root / 'tools/tests').glob('test-p00*.ps1'))
for directory in ('apps', 'crates'):
    paths.update((root / directory).rglob('*.rs'))
    paths.update((root / directory).rglob('Cargo.toml'))
paths.update((root / 'tools').glob('p01-*.py'))
for name in ('Cargo.toml', 'Cargo.lock'):
    if (root / name).exists():
        paths.add(root / name)
for name in ('rust-toolchain.toml', 'AGENTS.md', '05-prompts/MASTER-PROMPT.md',
             '05-prompts/RESUME-PROMPT.md', '01-prd/00-authoritative-decisions.md',
             'tools/doctor.ps1', 'tools/install-p00-deps.ps1'):
    paths.add(root / name)
result = {
    'observed_at': datetime.now(timezone.utc).isoformat(),
    'scope': 'Current review-time inputs, not a Git diff or cryptographic signature of earlier test execution',
    'files': [{'path': p.relative_to(root).as_posix(), 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()}
              for p in sorted(paths)],
}
(root / 'reports/code-fingerprints.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'review_files': len(paths), 'artifact': 'reports/code-fingerprints.json'}))
