#!/usr/bin/env python3
"""Assert BackSpace/Delete block merging and undo/redo using real-window AUDIT.

Uses the same frozen scenario as the headless core; run via scripts/window-checks.py.
"""
import importlib.util
from pathlib import Path
import re
import os
import sys

ROOT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("parity_check", ROOT / "parity-check.py")
parity = importlib.util.module_from_spec(spec)
spec.loader.exec_module(parity)

if __name__ == "__main__":
    display = os.environ.get("DISPLAY", "")
    if not display or re.fullmatch(r"(?:[^:]*):0(?:\.\d+)?", display):
        raise SystemExit("a private Xvfb DISPLAY is required")
    parity.scenario(Path(sys.argv[1]).resolve(), ROOT / "scenarios/backspace-block-merge.json",
                    Path(sys.argv[2]).resolve() / "scenario")
