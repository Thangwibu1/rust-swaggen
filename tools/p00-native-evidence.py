"""Verify only scoped deterministic native transfer fixture files; no UI performance claim."""
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path

root = Path(__file__).resolve().parents[1]
paths = sorted((root / '.local/p00-native').glob('body-*.bin'))
assert paths, 'Run the native controlled transfer fixture before recording evidence'
results = []
for path in paths:
    count = 0
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        while block := stream.read(64 * 1024):
            assert block == b'x' * len(block), 'Unexpected deterministic fixture contents'
            digest.update(block)
            count += len(block)
    assert count == 20 * 1024 * 1024
    results.append({'fixture_file': path.relative_to(root).as_posix(), 'bytes': count,
                    'sha256': digest.hexdigest(), 'all_bytes_verified': True})
evidence = {'name': 'native-ui-http-artifact', 'status': 'PASS', 'exit_code': 0,
            'command': 'python -X utf8 tools/p00-native-evidence.py',
            'timestamp': datetime.now(timezone.utc).isoformat(),
            'artifact_path': 'reports/native-ui-http-artifact.json', 'files': results,
            'limits': 'Artifact integrity only; manual UI displayed 64KiB preview. No p95/RAM/IME/accessibility acceptance.'}
(root / 'reports/native-ui-http-artifact.json').write_text(json.dumps(evidence, indent=2) + '\n', encoding='utf-8')
print(json.dumps(evidence, indent=2))
