"""Verify target projection cannot hide a reachable advisory-bearing package."""
import importlib.util
import json
from pathlib import Path
import tempfile
import tomllib
import unittest

spec = importlib.util.spec_from_file_location('audit', Path(__file__).parents[1]/'p00-audit.py')
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class WindowsAudit(unittest.TestCase):
    def fixture(self):
        lock = '''version = 4
[[package]]
name = "root"
version = "0.0.0"
dependencies = ["build-helper", "linux-only"]
[[package]]
name = "build-helper"
version = "1.0.0"
dependencies = ["unsafe-transitive"]
[[package]]
name = "unsafe-transitive"
version = "2.0.0"
[[package]]
name = "linux-only"
version = "3.0.0"
'''
        packages = [dict(id=p['name'], **p) for p in tomllib.loads(lock)['package']]
        metadata = dict(packages=packages, workspace_members=['root'], resolve=dict(nodes=[
            dict(id='root', dependencies=['build-helper']),
            dict(id='build-helper', dependencies=['unsafe-transitive']),
            dict(id='unsafe-transitive', dependencies=[])]))
        return lock, metadata

    def test_target_projection_keeps_build_and_transitive_dependencies(self):
        lock, metadata = self.fixture()
        packages = tomllib.loads(audit.project_lock(lock, metadata))['package']
        self.assertEqual({p['name'] for p in packages}, {'root', 'build-helper', 'unsafe-transitive'})

    def test_edge_not_in_original_lock_is_rejected(self):
        lock, metadata = self.fixture()
        metadata['resolve']['nodes'][0]['dependencies'].append('unsafe-transitive')
        with self.assertRaises(ValueError):
            audit.project_lock(lock, metadata)

    def test_unexplained_disconnected_package_is_rejected(self):
        lock, metadata = self.fixture()
        metadata['resolve']['nodes'].append(dict(id='linux-only', dependencies=[]))
        with self.assertRaises(ValueError):
            audit.project_lock(lock, metadata)

    def test_missing_input_hashes_cannot_validate_pass(self):
        with tempfile.TemporaryDirectory() as folder:
            self.assertFalse(audit.validate_windows_evidence(Path(folder), dict(
                name='rustsec-windows-p00-egui', target=audit.TARGET, input_sha256={})))

    def test_current_real_projection_detects_changed_manifest_and_projection(self):
        root = Path(__file__).resolve().parents[2]
        records = json.loads((root/'reports/security-checks.json').read_text(encoding='utf-8-sig'))
        record = next(r for r in records if r['name']=='rustsec-windows-p00-egui')
        self.assertTrue(audit.validate_windows_evidence(root, record))
        changed = json.loads(json.dumps(record))
        changed['input_sha256']['spikes/p00-egui/Cargo.toml'] = 'stale'
        self.assertFalse(audit.validate_windows_evidence(root, changed))
        changed = dict(record, projection_sha256='tampered')
        self.assertFalse(audit.validate_windows_evidence(root, changed))


if __name__ == '__main__':
    unittest.main()
