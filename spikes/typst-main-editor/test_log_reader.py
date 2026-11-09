"""Regression tests for audit-log polling, independent of X11 and the app."""
import tempfile
import unittest
from pathlib import Path

from log_reader import LogReader



class IncrementalLogTests(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.addCleanup(self.folder.cleanup)
        self.path = Path(self.folder.name) / 'app.log'
        self.path.touch()

    def append(self, data):
        with self.path.open('ab') as stream:
            stream.write(data)

    def test_waits_for_complete_utf8_line_and_keeps_partial_tail(self):
        reader = LogReader(self.path, ('AUDIT ',), history=True)
        line = 'AUDIT {"text":"中文"}\n'.encode()
        cut = line.index('中'.encode()) + 1
        self.append(line[:cut])
        self.assertEqual(reader.entries(), [])
        self.append(line[cut:-1])
        self.assertIsNone(reader.latest())
        self.append(b'\n')
        self.assertEqual(reader.entries(), [{'text': '中文'}])

    def test_parses_only_requested_prefixes_and_preserves_event_order(self):
        reader = LogReader(self.path, ('AUDIT ', 'PROJECT '), history=True)
        self.append(b'NOISE invalid json\nAUDIT {"n":1}\nPROJECT {"n":2}\nAUDIT {"n":3}\n')
        self.assertEqual(reader.entries(), [{'n': 1}, {'n': 2}, {'n': 3}])
        self.assertEqual(reader.latest('PROJECT '), {'n': 2})
        self.assertEqual(reader.latest(), {'n': 3})
        self.assertEqual(reader.poll(), [])
        self.assertEqual(reader.bytes_read, self.path.stat().st_size)
        self.append(b'AUDIT {"n":4}\n')
        self.assertEqual(reader.poll(), [('AUDIT ', {'n': 4})])
        self.assertEqual(reader.bytes_read, self.path.stat().st_size)

    def test_latest_only_does_not_accumulate_history(self):
        reader = LogReader(self.path, ('AUDIT ',))
        self.append(b'AUDIT {"n":1}\nAUDIT {"n":2}\n')
        self.assertEqual(reader.latest(), {'n': 2})
        self.assertIsNone(reader.history)

    def test_truncation_and_replacement_clear_history_and_partial_tail(self):
        reader = LogReader(self.path, ('AUDIT ',), history=True)
        self.append(b'AUDIT {"n":10000}\nAUDIT {')
        reader.poll()
        self.path.write_bytes(b'AUDIT {"n":2}\n')
        self.assertEqual(reader.entries(), [{'n': 2}])
        replacement = self.path.with_suffix('.new')
        replacement.write_bytes(b'AUDIT {"n":3}\n')
        replacement.replace(self.path)
        self.assertEqual(reader.entries(), [{'n': 3}])

    def test_explicit_restart_resets_even_if_new_log_is_larger(self):
        reader = LogReader(self.path, ('AUDIT ',), history=True)
        self.append(b'AUDIT {"n":1}\n')
        reader.poll()
        self.path.write_bytes(b'AUDIT {"n":22222}\n')
        reader.reset()
        self.assertEqual(reader.entries(), [{'n': 22222}])


if __name__ == '__main__':
    unittest.main()
