import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('gate', Path(__file__).parents[1]/'p00-gate.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class GateEvidence(unittest.TestCase):
    def test_compilation_alone_cannot_promote_gate(self):
        self.assertFalse(gate.evaluate([{'name':'egui-build','status':'PASS'}])['gate_passed'])

    def test_only_complete_required_evidence_promotes_gate(self):
        checks = [{'name':name,'status':'PASS'} for required in gate.CRITERIA.values() for name in required]
        self.assertTrue(gate.evaluate(checks)['gate_passed'])
        next(c for c in checks if c['name']=='p00-narrator')['status'] = 'BLOCKED'
        self.assertFalse(gate.evaluate(checks)['gate_passed'])

    def test_historical_iced_failure_does_not_overrule_accepted_candidate(self):
        checks = [{'name':name,'status':'PASS'} for required in gate.CRITERIA.values() for name in required]
        checks.append({'name':'native-accessibility','status':'FAIL'})
        self.assertTrue(gate.evaluate(checks)['gate_passed'])

    def test_selected_windows_audit_failure_blocks_even_if_full_lock_passed(self):
        checks = [{'name':name,'status':'PASS'} for required in gate.CRITERIA.values() for name in required]
        next(c for c in checks if c['name']=='rustsec-windows-p00-egui')['status'] = 'FAIL'
        checks.append({'name':'rustsec-p00-egui','status':'PASS'})
        self.assertFalse(gate.evaluate(checks)['gate_passed'])


if __name__ == '__main__':
    unittest.main()
