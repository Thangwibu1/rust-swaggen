"""Regression checks for evidence ordering and preservation, not product acceptance."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO

spec = importlib.util.spec_from_file_location('p00_report', Path(__file__).parents[1] / 'p00-report.py')
report = importlib.util.module_from_spec(spec)
spec.loader.exec_module(report)


def check(status, timestamp, name='mongo-runtime'):
    return {'name': name, 'status': status, 'timestamp': timestamp,
            'exit_code': 0 if status == 'PASS' else 2, 'command': 'fixture-only'}


class EvidenceOrdering(unittest.TestCase):
    def test_stale_advisory_hash_never_counts_as_clean_current_lock(self):
        prior_root = report.ROOT
        try:
            with tempfile.TemporaryDirectory() as directory:
                report.ROOT = Path(directory)
                lock = report.ROOT/'spikes/p00/Cargo.lock'
                lock.parent.mkdir(parents=True)
                lock.write_text('changed lock')
                checks = [{'name':'rustsec-p00','status':'PASS','lock_sha256':'old-hash'}]
                report.verify_current_lock_evidence(checks)
                self.assertEqual(checks[0]['status'], 'BLOCKED')
        finally:
            report.ROOT = prior_root

    def test_missing_source_manifest_review_never_counts_as_current_review(self):
        prior_root = report.ROOT
        try:
            with tempfile.TemporaryDirectory() as directory:
                report.ROOT = Path(directory)
                checks = [{'name':'dependency-source-license-msrv','status':'PASS'}]
                report.verify_current_lock_evidence(checks)
                self.assertEqual(checks[0]['status'], 'BLOCKED')
        finally:
            report.ROOT = prior_root

    def test_latest_failure_replaces_older_pass(self):
        old = check('PASS', '2026-11-24T05:00:00Z')
        new = check('FAIL', '2026-11-24T05:01:00Z')
        self.assertEqual(report.latest_by_name([new, old]), [new])

    def test_timezone_offsets_are_compared_as_instants(self):
        older = check('BLOCKED', '2026-11-24T12:00:00+07:00')
        newer = check('PASS', '2026-11-24T05:01:00.1234567Z')
        self.assertEqual(report.latest_by_name([older, newer]), [newer])

    def test_unrelated_scopes_are_not_dropped(self):
        mongo = check('PASS', '2026-11-24T05:00:00Z')
        gui = check('FAIL', '2026-11-24T05:01:00Z', 'native-accessibility')
        self.assertEqual(report.latest_by_name([mongo, gui]), [mongo, gui])

    def test_unqualified_timestamp_is_rejected(self):
        with self.assertRaises(ValueError):
            report.latest_by_name([check('PASS', '2026-11-24T05:00:00'),
                                   check('FAIL', '2026-11-24T05:01:00Z')])

    def test_unique_legacy_date_keeps_its_original_precision(self):
        observation = check('PASS', '2026-11-24', 'old-ui-observation')
        self.assertEqual(report.latest_by_name([observation]), [observation])

    def test_report_keeps_new_mongo_and_existing_product_ledgers(self):
        prior_root, prior_reports = report.ROOT, report.REPORTS
        try:
            with tempfile.TemporaryDirectory() as directory:
                report.ROOT = Path(directory)
                report.REPORTS = report.ROOT / 'reports'
                for folder in ('reports/raw', '04-phases', '02-design'):
                    (report.ROOT / folder).mkdir(parents=True, exist_ok=True)
                fixtures = {
                    'reports/p00-checks.json': [check('PASS', '2026-11-24T05:29:30Z')],
                    'reports/mongo-check.json': check('BLOCKED', '2026-11-24T05:19:19Z'),
                    '04-phases/traceability.json': [],
                    '02-design/screen-requirement-map.json': [],
                    'reports/requirement-ledger.json': [{'status': 'in_progress'}],
                    'reports/screen-ledger.json': [{'status': 'implemented_unverified'}],
                }
                for path, value in fixtures.items():
                    report.write(path, value)
                with redirect_stdout(StringIO()):
                    report.main()
                summary = report.read('reports/preflight.json')
                self.assertFalse(summary['gate_passed'])
                self.assertEqual(summary['checks'][0]['status'], 'PASS')
                self.assertEqual(report.read('reports/requirement-ledger.json'), fixtures['reports/requirement-ledger.json'])
                self.assertEqual(report.read('reports/screen-ledger.json'), fixtures['reports/screen-ledger.json'])
                self.assertFalse(any('Mongo runtime did not complete' in b for b in summary['blockers']))
        finally:
            report.ROOT, report.REPORTS = prior_root, prior_reports


if __name__ == '__main__':
    unittest.main()
