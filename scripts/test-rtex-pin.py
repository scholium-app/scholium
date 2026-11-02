#!/usr/bin/env python3
"""Pin promotion must require actual sandbox fault evidence and preserve PIN on failure."""
import importlib.util
import copy
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('pin', Path(__file__).with_name('rtex-pin.py'))
pin = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pin)


class PinGate(unittest.TestCase):
    def test_all_e2_controls_need_effective_host_and_contained_sandbox(self):
        rows = [{'verdict': {'control': c, 'config': cfg, 'result': 'Pass',
                            'host_effective': True, 'sandbox_effective': False},
                 'runs': [{'stray_processes': []}, {'stray_processes': []}]}
                for c, cfg in [('a','default'), ('b','default'), ('c','default'), ('c','lifted'),
                               ('d-lua','default'), ('d-openout','default'), ('e','lifted'), ('f','default')]]
        self.assertTrue(pin.e2_passed(rows))
        self.assertFalse(pin.e2_passed(rows[:-1]))
        rows[-1]['verdict']['host_effective'] = False
        self.assertFalse(pin.e2_passed(rows))

    def test_empty_or_partial_e8_matrix_cannot_pass(self):
        self.assertFalse(pin.e8_passed([]))
        self.assertFalse(pin.e8_passed([{'scenario': 'live engine SIGKILL', 'sandbox_alive': True}]))

    def test_full_matrix_rejects_missing_timeout_failure_or_restart_loops(self):
        rows = [
            {'scenario':'live engine SIGKILL', 'killed':[123], 'sandbox_alive':True, 'recovered_live_s':1,
             'engine_states':[{'state':'Ready'}], 'layout_after':{'compile':{'state':'Ok'}}},
            {'scenario':'live and background hang', 'watchdog_s':None, 'upstream_pass_timeout_ms':8000,
             'layouts':[{'compile':{'state':'Failed'},'wall_ms':8005}], 'layout_recovered_after_fix_s':1,
             'diagnostics':['time limit']},
            {'scenario':'preamble error: missing-package', 'fixed_at_s':20,
             'engine_states':[{'t':1,'state':'Starting'},{'t':2,'state':'Failed'}],
             'recovered_live_s':1, 'layout_ok_after_fix_s':1},
            {'scenario':'preamble error: undefined-command','recovered_live_s':1,'layout_ok_after_fix_s':1},
            {'scenario':'malformed host commands','cases':[{'alive':True,'reply':{'reply':'error'}}]*11,
             'written_outside_snapshot':[]},
            {'scenario':'rtex serve SIGKILL','killed':[123],'sandbox_exit':137,'eof_after_s':0.001,
             'stray_processes':[],'restart_warm-cache_first_layout_s':1,'restart_cache-copy_first_layout_s':1}]
        self.assertTrue(pin.e8_passed(rows))
        broken = copy.deepcopy(rows)
        broken[1]['layouts'][0]['compile']['state'] = 'Ok'
        self.assertFalse(pin.e8_passed(broken))
        broken = copy.deepcopy(rows)
        broken[2]['engine_states'].append({'t':3,'state':'Starting'})
        self.assertFalse(pin.e8_passed(broken))
        broken = copy.deepcopy(rows)
        broken[5]['stray_processes'] = [123]
        self.assertFalse(pin.e8_passed(broken))

    def test_advisories_and_internal_bans_do_not_override_licenses_sources(self):
        self.assertTrue(pin.e1_passed({'licenses-sources': 0, 'bans': 2, 'advisories': 1}))
        self.assertFalse(pin.e1_passed({'licenses-sources': 1, 'bans': 0, 'advisories': 0}))

    def test_promotion_keeps_old_pin_on_any_failed_gate(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'PIN'
            original = b'{"repository":"upstream", "tag":"v0.0.6", "commit":"old"}\n'
            path.write_bytes(original)
            self.assertFalse(pin.promote(path, 'upstream', 'a'*40, {'E1': True, 'E2': False, 'E8': True}))
            self.assertEqual(path.read_bytes(), original)
            self.assertTrue(pin.promote(path, 'upstream', 'a'*40, {'E1': True, 'E2': True, 'E8': True}))
            self.assertEqual(json.loads(path.read_text()), {'repository':'upstream','branch':'main','commit':'a'*40})

    def test_nested_application_paths_are_never_upstream_build_directories(self):
        with self.assertRaises(ValueError):
            pin.external(pin.ROOT / 'target/rtex')


if __name__ == '__main__':
    unittest.main()
