#!/usr/bin/env python3
"""Bundle publication must reject tampering and retain the last complete installation."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location('bundle', Path(__file__).with_name('bundle-rtex.py'))
bundle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bundle)


class BundleContract(unittest.TestCase):
    def test_verification_rejects_changed_binary_support_and_escaping_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'bin').mkdir()
            (root / 'tex').mkdir()
            (root / 'licenses').mkdir()
            for name in ('bin/rtex', 'tex/rtex-dl.lua', 'licenses/LICENSE'):
                (root / name).write_text('fixture')
            manifest = {'schema': 1, 'commit': 'a'*40, 'files': bundle.inventory(root)}
            manifest['binary_sha256'] = manifest['files']['bin/rtex']
            bundle.verify(root, manifest)
            (root / 'tex/rtex-dl.lua').write_text('tampered')
            with self.assertRaises(ValueError):
                bundle.verify(root, manifest)
            manifest['files'] = {'../escape': 'a'*64}
            with self.assertRaises(ValueError):
                bundle.verify(root, manifest)

    def test_e1_evidence_must_match_pin_and_locked_dependency_graph(self):
        report = {'E1': True, 'candidate': 'a'*40, 'lock_sha256': 'b'*64}
        bundle.verify_review(report, 'a'*40, 'b'*64)
        for key, value in [('E1', False), ('candidate', 'c'*40), ('lock_sha256', 'd'*64)]:
            changed = dict(report, **{key: value})
            with self.assertRaises(ValueError):
                bundle.verify_review(changed, 'a'*40, 'b'*64)

    def test_failed_publication_restores_previous_complete_bundle(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output, stage = root/'installed', root/'stage'
            output.mkdir()
            stage.mkdir()
            (output/'manifest.json').write_text('previous')
            rename = Path.rename
            def fail_stage(path, target):
                if path == stage:
                    raise OSError('simulated publication failure')
                return rename(path, target)
            with mock.patch.object(Path, 'rename', fail_stage):
                with self.assertRaises(OSError):
                    bundle.publish(stage, output)
            self.assertEqual((output/'manifest.json').read_text(), 'previous')


if __name__ == '__main__':
    unittest.main()
