import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('dependency_review', Path(__file__).parents[1]/'p00-dependency-review.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class LicenseChoices(unittest.TestCase):
    def test_font_terms_are_preserved_alongside_code_license(self):
        self.assertEqual(module.choose_license('(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0'),
                         ['MIT', 'OFL-1.1', 'Ubuntu-font-1.0'])

    def test_permissive_or_branch_does_not_select_gpl(self):
        self.assertEqual(module.choose_license('Apache-2.0 OR GPL-2.0-only'), ['Apache-2.0'])
        self.assertIsNone(module.choose_license('MIT AND GPL-2.0-only'))

    def test_unknown_exception_and_invalid_syntax_fail_closed(self):
        for expression in ('MIT WITH invented-exception', 'MIT@', '', '(MIT', 'MIT OR'):
            self.assertIsNone(module.choose_license(expression))
        self.assertEqual(module.choose_license('Apache-2.0 / MIT'), ['Apache-2.0'])

    def test_manifest_notice_path_cannot_escape_package_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package = root/'package'
            package.mkdir()
            (package/'LICENSE').write_text('fixture license')
            (root/'outside.txt').write_text('must not be copied')
            self.assertEqual(module.notice_paths(package, '../outside.txt'), [package/'LICENSE'])


if __name__ == '__main__':
    unittest.main()
