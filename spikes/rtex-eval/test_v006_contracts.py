"""Behaviour checks for the v0.0.6 acceptance decisions (no TeX required)."""
import unittest

from v006_contracts import e8_verdict
from e8_faults import timeout_args


class FaultAcceptance(unittest.TestCase):
    def test_default_cli_does_not_claim_an_explicit_timeout(self):
        self.assertEqual(timeout_args(None), ())
        self.assertEqual(timeout_args(8000), ("--pass-timeout-ms", "8000"))

    def test_a_killed_background_pass_cannot_pass_when_reported_ok(self):
        row = {"scenario": "background kill", "killed": [123],
               "layouts": [{"compile": {"state": "Ok"}}], "recovered": True}
        self.assertFalse(e8_verdict(row)["pass"])

    def test_a_timeout_requires_upstream_failure_and_successful_recovery(self):
        row = {"scenario": "upstream timeout", "layouts": [{"compile": {"state": "Failed"}}],
               "recovered": False, "timeout_seen": True}
        self.assertFalse(e8_verdict(row)["pass"])
        row["recovered"] = True
        self.assertTrue(e8_verdict(row)["pass"])

    def test_every_malformed_command_must_error_and_leave_server_alive(self):
        row = {"scenario": "malformed host commands", "cases": [
            {"reply": {"reply": "error"}, "alive": True},
            {"reply": {"reply": "edit"}, "alive": True}], "written_outside_snapshot": []}
        self.assertFalse(e8_verdict(row)["pass"])
        row["cases"][1]["reply"] = {"reply": "error"}
        self.assertTrue(e8_verdict(row)["pass"])

    def test_empty_fault_observations_are_not_a_pass(self):
        self.assertFalse(e8_verdict({"scenario": "background kill", "killed": [],
                                    "layouts": [], "recovered": True})["pass"])


if __name__ == "__main__":
    unittest.main()
