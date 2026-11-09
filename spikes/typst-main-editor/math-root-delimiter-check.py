#!/usr/bin/env python3
"""Replay root/index and delimiter focus actions with visible caret geometry in both locales."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import time

from check_evidence import evidence_enabled

ROOT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("parity", ROOT / "parity-check.py")
parity = importlib.util.module_from_spec(spec)
spec.loader.exec_module(parity)


def main():
    binary, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
    def check(replay, state, step):
        caret = parity.cursor_caret(state)
        assert caret and caret["exact"], (step, caret)
        assert state["ime_caret"] and state["ime_caret"][3] > state["ime_caret"][1]
        assert state["selection_geometry_error"] is None
        if step == 8:
            if evidence_enabled():
                time.sleep(0.12)
                subprocess.run(["import", "-window", replay.window, str(output / "root-index-brackets.png")],
                               env=replay.env, check=True, timeout=20)
    parity.scenario(binary, ROOT / "scenarios/math-root-delimiter.json", output / "scenario", on_step=check)
    print("PASS root/index/delimiter menu and focus actions, traversal and visible caret", flush=True)


if __name__ == "__main__":
    main()
