#!/usr/bin/env python3
"""AUDIT assertions for fraction edges, Left traversal and Backspace entering from body.

Uses the same frozen scenario as the headless core. Run via scripts/window-checks.py.
"""
import importlib.util
from pathlib import Path
import sys
import subprocess
import time

ROOT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("parity", ROOT / "parity-check.py")
parity = importlib.util.module_from_spec(spec)
spec.loader.exec_module(parity)


def main():
    binary, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
    scenario = ROOT / "scenarios" / "math-edge-caret.json"
    # The scenario's intermediate cursor and undo_depth assertions are immutable inputs.
    def capture(replay, state, step):
        if step not in (10, 16, 18, 23):
            return
        caret = parity.cursor_caret(state)
        assert caret and caret["exact"], f"no exact edge caret at step {step}"
        assert state["chrome"]["facts"]["status"]["context"] == "chrome.status.context.math"
        time.sleep(0.12)  # AUDIT precedes presentation; not a timing measurement.
        subprocess.run(["import", "-window", replay.window, str(output / f"edge-{step}.png")],
                       env=replay.env, check=True, timeout=20)
    parity.scenario(binary, scenario, output / "replay", on_step=capture)
    import json
    rows = json.loads((output / "replay" / "steps.json").read_text())
    edge_steps = [10, 16, 18, 23]
    for step in edge_steps:
        caret = rows[step - 1]["caret"]
        assert caret and caret["exact"], f"missing exact Scene caret at edge step {step}"
    print("PASS exact fraction-edge Scene carets and frozen AUDIT traversal", flush=True)


if __name__ == "__main__":
    main()
