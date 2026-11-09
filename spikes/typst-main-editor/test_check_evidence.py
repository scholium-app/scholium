"""Evidence captures must not launch ImageMagick in ordinary acceptance runs."""
import unittest
from pathlib import Path
from unittest.mock import patch

from project_ui import ProjectApp


class EvidenceCaptureTests(unittest.TestCase):
    def test_ordinary_run_skips_image_capture_and_presentation_delay(self):
        app = ProjectApp('app', Path('/tmp/evidence-test'), 'project')
        with patch.dict('os.environ', {}, clear=True), \
                patch('project_ui.subprocess.run') as run, \
                patch('project_ui.time.sleep') as sleep:
            app.capture('review')
        run.assert_not_called()
        sleep.assert_not_called()

    def test_explicit_evidence_run_keeps_capture_name_window_and_environment(self):
        app = ProjectApp('app', Path('/tmp/evidence-test'), 'project')
        app.window = '123'
        with patch.dict('os.environ', {'SCHOLIUM_CHECK_EVIDENCE': '1'}, clear=True), \
                patch('project_ui.subprocess.run') as run, \
                patch('project_ui.time.sleep') as sleep:
            app.capture('review')
        run.assert_called_once_with(
            ['import', '-window', '123', '/tmp/evidence-test/review.png'], env=app.env, check=True)
        sleep.assert_called_once_with(0.15)


if __name__ == '__main__':
    unittest.main()
