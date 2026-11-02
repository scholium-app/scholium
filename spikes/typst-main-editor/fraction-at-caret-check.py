#!/usr/bin/env python3
"""AUDIT: insertion/selection via toolbar and menu, nested walks and symmetric deletion.

Both frozen scenarios are also replayed by the production headless core. Run through
scripts/window-checks.py for private Xvfb, serialization and both interface locales.
"""
import importlib.util
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("parity", ROOT / "parity-check.py")
parity = importlib.util.module_from_spec(spec)
spec.loader.exec_module(parity)


def main():
    binary, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
    for name in ("fraction-at-caret", "structure-audit-walks"):
        def check(replay, state, step):
            caret = parity.cursor_caret(state)
            assert caret and caret["exact"], (step, caret)
            area = state["ime_caret"]
            assert area and area[3] > area[1], (step, area)
            assert state["selection_geometry_error"] is None, (step, state["selection_geometry_error"])
            if name == "fraction-at-caret" and step in (4, 6, 11, 39):
                time.sleep(0.12)  # AUDIT precedes GPU presentation; not a timing measurement.
                subprocess.run(["import", "-window", replay.window, str(output / f"{name}-{step}.png")],
                               env=replay.env, check=True, timeout=20)
        parity.scenario(binary, ROOT / "scenarios" / f"{name}.json", output / name, on_step=check)
    print("PASS fraction commands, selection geometry and every audited native walk stop", flush=True)


if __name__ == "__main__":
    main()
