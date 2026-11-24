"""Evaluate the foundation gate from explicit evidence; never infer manual PASS."""
import json
import subprocess
import sys
from pathlib import Path
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[1]
CRITERIA = {
    'Windows toolchain and link': ['Rust compiler', 'Cargo', 'Rustup target', 'Git', 'MSVC installation', 'Windows SDK', 'MSVC link smoke test'],
    'Selected UI compile and editor regression': ['egui-build', 'egui-release-build', 'egui-unit', 'egui-clippy'],
    'Native editor, search and keyboard': ['p00-editor-highlight-search', 'p00-keyboard-modal'],
    'Native Vietnamese input': ['p00-telex-input'],
    'Native resize and DPI smoke': ['p00-native-resize-dpi'],
    'Screen reader': ['p00-narrator'],
    'Integrated local save and restore': ['p00-ui-local-restore'],
    'Integrated bounded 20 MiB response': ['p00-ui-large-response'],
    'Journal crash and corruption': ['journal-unit', 'journal-process-crash', 'journal-crash-matrix'],
    'Real replica-set transactions': ['mongo-runtime'],
    'Declared source license/MSRV review': ['dependency-source-license-msrv'],
    # Windows is the supported target. Iced is a rejected comparison, never an
    # adopted dependency; its full-lock and Windows failures remain in reports.
    'Current advisory audit': ['rustsec-windows-p00', 'rustsec-windows-p00-egui', 'rustsec-windows-p00-transport'],
    'Recorded framework/storage decisions': ['p00-ui-decision', 'p00-storage-decision'],
}


def evaluate(checks):
    by_name = {check['name']: check for check in checks}
    criteria = []
    for title, required in CRITERIA.items():
        unresolved = [name for name in required if by_name.get(name, {}).get('status') != 'PASS']
        criteria.append(dict(criterion=title, status='PASS' if not unresolved else 'BLOCKED',
                             required_checks=required, unresolved_checks=unresolved))
    return dict(phase='P00', gate_passed=all(c['status'] == 'PASS' for c in criteria),
                criteria=criteria,
                scope='Foundation compatibility and prototype gate only; no product, installer, signing or full performance acceptance')


def main():
    subprocess.run([sys.executable, '-X', 'utf8', str(ROOT/'tools/p00-report.py')], check=True)
    gate = json.loads((ROOT/'reports/p00-gate.json').read_text(encoding='utf-8-sig'))
    print(json.dumps(dict(gate_passed=gate['gate_passed'], passed=sum(c['status']=='PASS' for c in gate['criteria']), total=len(gate['criteria']))))


if __name__ == '__main__':
    main()
