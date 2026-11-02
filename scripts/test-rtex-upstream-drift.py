#!/usr/bin/env python3
"""Drift checks must preserve report-only findings and fail closed on incomplete smoke."""
import importlib.util
from pathlib import Path
import sys
import subprocess
import tempfile
import time
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('drift', Path(__file__).with_name('rtex-upstream-drift.py'))
drift = importlib.util.module_from_spec(spec)
spec.loader.exec_module(drift)


class DriftReporting(unittest.TestCase):
    def test_external_failure_stays_visible_in_summary(self):
        report = drift.table('v0.0.6', 'abc', 'v0.0.7', 'def', [('build', 'FAIL', 'exit 1')])
        self.assertIn('FAIL', report)
        self.assertIn('v0.0.7', report)
        self.assertIn('v0.0.6', report)

    def test_summary_lists_commits_since_pin(self):
        report = drift.table('v0.0.6', 'abc', 'v0.0.7', 'def', [], ['1234 upstream change'])
        self.assertIn('1234 upstream change', report)

    def test_moved_pin_tag_is_reported_as_drift(self):
        self.assertEqual(drift.tag_status('v0.0.6', 'abc', 'v0.0.6', 'abc'), 'same')
        self.assertEqual(drift.tag_status('v0.0.6', 'abc', 'v0.0.6', 'def'), 'tag moved')
        self.assertEqual(drift.tag_status('v0.0.6', 'abc', 'v0.0.7', 'def'), 'new tag')

    def test_dispatch_ref_can_name_a_remote_branch(self):
        with patch.object(drift, 'git', side_effect=[subprocess.CalledProcessError(128, 'git'), 'abc']) as git:
            self.assertEqual(drift.resolve(Path('/external'), 'topic'), 'abc')
            self.assertEqual(git.call_args.args[-1], 'origin/topic^{commit}')

    def test_smoke_requires_own_edit_and_a_layout(self):
        events = [{'event': 'LayoutUpdate', 'compile': {'state': 'Ok'}},
                  {'reply': 'edit', 'result': {'edit_id': 3}},
                  {'event': 'ParagraphUpdate', 'edit_id': 2, 'status': 'ok'}]
        self.assertFalse(drift.smoke_passed(events))
        events.append({'event': 'ParagraphUpdate', 'edit_id': 3, 'status': 'ok'})
        self.assertTrue(drift.smoke_passed(events))

    def test_error_and_missing_layout_are_not_smoke_success(self):
        self.assertFalse(drift.smoke_passed([{'reply': 'error'}]))
        self.assertFalse(drift.smoke_passed([{'reply': 'edit', 'result': {'edit_id': 1}},
                                           {'event': 'ParagraphUpdate', 'edit_id': 1, 'status': 'ok'}]))

    def test_timeout_kills_the_whole_process_group(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pid_file = root / 'child.pid'
            script = "import subprocess,sys,time; p=subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)']); open(sys.argv[1],'w').write(str(p.pid)); time.sleep(30)"
            code = drift.run([sys.executable, '-c', script, str(pid_file)], root, root / 'run.log', seconds=0.2)
            self.assertEqual(code, 124)
            child_pid = int(pid_file.read_text())
            for _ in range(20):
                if not Path(f'/proc/{child_pid}').exists():
                    break
                time.sleep(0.05)
            self.assertFalse(Path(f'/proc/{child_pid}').exists())
            self.assertIn('TIMEOUT after 0.2s', (root / 'run.log').read_text())


if __name__ == '__main__':
    unittest.main()
