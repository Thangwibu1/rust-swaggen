"""Verify the selected Windows software renderer graph, not GUI or performance acceptance."""
import hashlib
import json
import re
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
metadata = json.loads((ROOT / 'reports/raw/software-metadata.json').read_text(encoding='utf-8-sig'))
packages = {p['id']: p for p in metadata['packages']}
active = {packages[n['id']]['name']: (packages[n['id']], n['features']) for n in metadata['resolve']['nodes']}
assert 'iced_tiny_skia' in active
# cargo metadata can retain optional weak-feature nodes that are not built.
# Use cargo tree for the selected build graph, plus the exact renderer feature flags.
tree = (ROOT / 'reports/raw/software-tree.txt').read_text(encoding='utf-8-sig')
assert re.search(r'^iced_tiny_skia\s', tree, re.MULTILINE)
assert not re.search(r'^(?:iced_wgpu|wgpu)\s', tree, re.MULTILINE)
assert not re.search(r'^mongodb\s', tree, re.MULTILINE), 'Mongo driver must remain outside the native probe build'
assert 'tiny-skia' in active['iced_renderer'][1]
assert 'wgpu-bare' not in active['iced_renderer'][1]
assert active['iced'][0]['version'] == '0.14.0'
checks = json.loads((ROOT / 'reports/software-checks.json').read_text(encoding='utf-8-sig'))
build = next(c for c in checks if c['name'].startswith('software-build-') and c['status'] == 'PASS')
profile = build['name'].removeprefix('software-build-')
assert profile in {'debug', 'release'}
binary = ROOT / f'spikes/p00-native/target-software/x86_64-pc-windows-msvc/{profile}/native-gate.exe'
assert binary.is_file()
digest = hashlib.sha256()
with binary.open('rb') as stream:
    while block := stream.read(1024 * 1024):
        digest.update(block)
evidence = {
    'name': f'software-renderer-feature-graph-{profile}', 'status': 'PASS', 'exit_code': 0,
    'command': 'python -X utf8 tools/p00-software-evidence.py',
    'timestamp': datetime.now(timezone.utc).isoformat(), 'artifact_path': f'reports/software-renderer-evidence-{profile}.json',
    'profile': profile,
    'binary': binary.relative_to(ROOT).as_posix(), 'binary_sha256': digest.hexdigest(),
    'renderer_crates': {name: {'version': active[name][0]['version'], 'features': active[name][1]} for name in ('iced', 'iced_renderer', 'iced_tiny_skia')},
    'wgpu_selected': False,
    'limits': 'Build/feature graph only. Interactive launch must be observed separately. Windows compositor/GPU driver behavior and release performance are not certified.'
}
(ROOT / 'reports/software-renderer-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n', encoding='utf-8')
(ROOT / evidence['artifact_path']).write_text(json.dumps(evidence, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'status': 'PASS', 'wgpu_selected': False, 'binary': evidence['binary']}, indent=2))
