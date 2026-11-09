#!/usr/bin/env python3
"""Runner contracts: complete shards, cross-process exclusion, private displays."""
import fcntl
import importlib.util
import multiprocessing as mp
import os
from pathlib import Path
import tempfile
from unittest.mock import patch
import unittest

spec = importlib.util.spec_from_file_location("runner", Path(__file__).with_name("window-checks.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
CONTEXT = mp.get_context("spawn")


def hold_slot(prefix, exclusive, entered, release, jobs=2):
    with runner.check_slot(jobs, exclusive, prefix=prefix):
        entered.set()
        release.wait(10)


class RunnerContracts(unittest.TestCase):
    def test_shards_cover_discovered_checks_exactly_once_and_balance_long_checks(self):
        names = runner.discover()
        weights = {name: (100 if i < 4 else 10) for i, name in enumerate(names)}
        shards = runner.balanced_shards(names, weights, 2)
        self.assertEqual(sorted(sum(shards, [])), sorted(names))
        totals = [sum(weights[n] for n in shard) for shard in shards]
        self.assertLessEqual(max(totals) - min(totals), 10)
        self.assertEqual(shards, runner.balanced_shards(list(reversed(names)), weights, 2))

    def test_missing_durations_still_cover_new_checks(self):
        self.assertEqual(sorted(sum(runner.balanced_shards(["new", "old"], {"old": 50}, 3), [])),
                         ["new", "old"])

    def test_invalid_shards_are_rejected(self):
        for value in ("0/2", "3/2", "1/0", "1", "x/2"):
            with self.assertRaises(runner.argparse.ArgumentTypeError):
                runner.parse_shard(value)
        self.assertEqual(runner.parse_shard("2/3"), (2, 3))

    def test_exclusive_covers_every_timing_sensitive_check(self):
        for name in ("latex-exit", "latex-view", "caret-follow", "sharp-raster", "parity",
                     "source-preview-layout"):
            self.assertTrue(runner.EXCEPTIONS[name]["exclusive"], name)

    def test_machine_slots_and_exclusive_are_shared_between_processes(self):
        with tempfile.TemporaryDirectory() as tmp:
            prefix = str(Path(tmp) / "locks")
            releases = [CONTEXT.Event() for _ in range(4)]
            entered = [CONTEXT.Event() for _ in range(4)]
            processes = [CONTEXT.Process(target=hold_slot, args=(prefix, i == 3, entered[i], releases[i]))
                         for i in range(4)]
            try:
                for i in (0, 1):
                    processes[i].start()
                    self.assertTrue(entered[i].wait(3))
                processes[2].start()
                self.assertFalse(entered[2].wait(.2), "third check exceeded two machine slots")
                releases[0].set()
                self.assertTrue(entered[2].wait(3))
                processes[3].start()
                self.assertFalse(entered[3].wait(.2), "exclusive overlapped normal checks")
                releases[1].set()
                releases[2].set()
                self.assertTrue(entered[3].wait(3))
                # Different --jobs values must still respect the exclusive holder.
                with open(f"{prefix}.lock", "a") as gate:
                    with self.assertRaises(BlockingIOError):
                        fcntl.flock(gate, fcntl.LOCK_SH | fcntl.LOCK_NB)
            finally:
                for event in releases:
                    event.set()
                for process in processes:
                    if process.pid:
                        process.join(3)
                        if process.is_alive():
                            process.kill()
                            process.join()
            self.assertTrue(all(p.exitcode == 0 for p in processes))

    def test_legacy_suite_lock_blocks_new_runner(self):
        with tempfile.TemporaryDirectory() as tmp:
            prefix = str(Path(tmp) / "locks")
            entered, release = CONTEXT.Event(), CONTEXT.Event()
            process = CONTEXT.Process(target=hold_slot, args=(prefix, False, entered, release))
            try:
                with open(f"{prefix}.lock", "a") as legacy:
                    fcntl.flock(legacy, fcntl.LOCK_EX)
                    process.start()
                    self.assertFalse(entered.wait(.2))
                self.assertTrue(entered.wait(3))
            finally:
                release.set()
                process.join(3)
                if process.is_alive():
                    process.kill()
                    process.join()
            self.assertEqual(process.exitcode, 0)

    def test_timeout_terminates_the_check_process_group(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            child_pid = out / "child.pid"
            program = ("import subprocess,time; from pathlib import Path; "
                       "p=subprocess.Popen(['sleep','60']); "
                       f"Path({str(child_pid)!r}).write_text(str(p.pid)); time.sleep(60)")
            with patch.object(runner, "command", return_value=["python3", "-c", program]), \
                    patch.dict(runner.EXCEPTIONS, {"timeout-probe": {"timeout": 3}}):
                result = runner.execute("timeout-probe", Path("unused"), out, "en", False, out/"check.log")
            self.assertEqual(result, "TIMEOUT")
            pid = int(child_pid.read_text())
            status = Path(f"/proc/{pid}/stat")
            self.assertTrue(not status.exists() or status.read_text().split()[2] == "Z",
                            "child still runs after check timeout")

    def test_evidence_flag_and_environment_override_are_preserved(self):
        with patch.dict(os.environ, {"DISPLAY": ":0", "WAYLAND_DISPLAY": "desktop",
                                     "SCHOLIUM_CHECK_EVIDENCE": "1"}):
            env = runner.check_environment("zh-Hans", Path("/tmp/check"), False)
            self.assertEqual(env["SCHOLIUM_CHECK_EVIDENCE"], "1")
            self.assertNotIn("DISPLAY", env)
            self.assertNotIn("WAYLAND_DISPLAY", env)
        self.assertEqual(runner.check_environment("en", Path("/tmp/check"), True)
                         ["SCHOLIUM_CHECK_EVIDENCE"], "1")

    def test_xvfb_uses_distinct_ready_displays_without_desktop_environment(self):
        with tempfile.TemporaryDirectory() as tmp:
            env = runner.check_environment("en", Path(tmp), False)
            self.assertNotIn("DISPLAY", env)
            self.assertNotIn("WAYLAND_DISPLAY", env)
            with runner.private_display("640x480x24", env) as first:
                with runner.private_display("640x480x24", env) as second:
                    self.assertNotEqual(first["DISPLAY"], second["DISPLAY"])
                    self.assertNotEqual(first["DISPLAY"], ":0")
                    self.assertEqual(first["WINIT_UNIX_BACKEND"], "x11")
                    for current in (first, second):
                        runner.subprocess.run(["xdpyinfo"], env=current, check=True,
                                              stdout=runner.subprocess.DEVNULL)


if __name__ == "__main__":
    unittest.main()
