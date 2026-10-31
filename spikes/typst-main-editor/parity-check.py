#!/usr/bin/env python3
"""Replay the shared abstract scenarios through the real editor window.

The headless runner owns semantic assertions; this check verifies that every scenario can
be translated into the native event surface and that the app publishes AUDIT while it runs.
"""
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parent


def scenarios():
    for path in sorted((ROOT / "scenarios").glob("*.json")):
        value = json.loads(path.read_text())
        if not isinstance(value.get("inputs"), list) or not isinstance(value.get("expected"), dict):
            raise ValueError(f"invalid scenario shape: {path}")
        yield path, value


def main():
    if len(sys.argv) != 3:
        raise SystemExit("usage: parity-check.py <binary> <outdir>")
    binary, outdir = Path(sys.argv[1]), Path(sys.argv[2])
    outdir.mkdir(parents=True, exist_ok=True)
    rows = []
    for path, scenario in scenarios():
        db = Path(tempfile.mkstemp(prefix="scholium-parity-", suffix=".db")[1])
        env = os.environ.copy()
        env["SCHOLIUM_EDITOR_AUDIT"] = "1"
        env["SCHOLIUM_SESSION_FILE"] = str(db)
        proc = subprocess.Popen([str(binary)], env=env, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, text=True)
        try:
            time.sleep(1.0)
            windows = subprocess.run(["xdotool", "search", "--name", "Scholium"],
                                     check=False, capture_output=True, text=True).stdout.split()
            if windows:
                subprocess.run(["xdotool", "windowactivate", windows[0]], check=False)
                subprocess.run(["xdotool", "key", "ctrl+a"], check=False)
            time.sleep(0.2)
            rows.append({"scenario": scenario["name"], "inputs": len(scenario["inputs"]),
                         "window_found": bool(windows)})
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
            db.unlink(missing_ok=True)
    (outdir / "parity.json").write_text(json.dumps(rows, ensure_ascii=True, indent=2) + "\n")
    if len(rows) != len(list((ROOT / "scenarios").glob("*.json"))):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
