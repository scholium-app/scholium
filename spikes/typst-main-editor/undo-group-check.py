#!/usr/bin/env python3
"""Real-window undo grouping: Ctrl+Z removes a typed sentence word by word, Ctrl+Y restores it.

Run under Xvfb from the repository root:
  xvfb-run -a -s '-screen 0 1280x900x24' python3 spikes/typst-main-editor/undo-group-check.py \\
    target/debug/scholium-app /tmp/out
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

binary = Path(sys.argv[1]).resolve()
output = Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
if any(output.iterdir()):
    raise SystemExit("output directory must be empty")
env = dict(os.environ, SCHOLIUM_SESSION_FILE=str(output / "doc.sqlite"), SCHOLIUM_EDITOR_AUDIT="1",
           WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR="1")
env.pop("WAYLAND_DISPLAY", None)
log_path = output / "app.log"
results = {}


def state():
    lines = [l for l in log_path.read_text().split("\n")[:-1] if l.startswith("AUDIT ")]
    return json.loads(lines[-1][6:]) if lines else None


def wait(predicate, what, timeout=60):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        current = state()
        if current and predicate(current):
            return current
        time.sleep(0.05)
    raise RuntimeError(f"timeout: {what}")


def xdo(*args):
    subprocess.run(["xdotool", *map(str, args)], check=True, env=env)


def shot(name):
    time.sleep(0.12)  # audit precedes GPU presentation; not a latency measurement
    subprocess.run(["import", "-window", window, str(output / f"{name}.png")], check=True, env=env)


def record(name, ok, **detail):
    results[name] = {"pass": bool(ok), **detail}
    print(("PASS " if ok else "FAIL ") + name, flush=True)
    if not ok:
        raise AssertionError(f"{name}: {detail}")


def text(s):
    """Concatenated text of the first block."""
    return "".join(i["body"]["text"] for i in s["snapshot"]["blocks"][0]["content"]
                   if i["body"]["kind"] == "text")


def settled(expected):
    return lambda s: s["current"] and text(s) == expected


with log_path.open("w") as log:
    app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
try:
    wait(lambda s: s["current"], "first scene")
    window = subprocess.check_output(["xdotool", "search", "--pid", str(app.pid)], env=env, text=True).split()[0]
    xdo("windowfocus", window)
    xdo("type", "--delay", "40", "alpha beta gamma")
    s = wait(settled("alpha beta gamma"), "typed")
    record("01-sentence-typed", text(s) == "alpha beta gamma", text=text(s))

    xdo("key", "ctrl+z")
    s = wait(settled("alpha beta "), "one undo")
    record("02-one-undo-removes-the-last-word", text(s) == "alpha beta ", text=text(s))
    shot("03-after-one-undo")

    xdo("key", "ctrl+z")
    s = wait(settled("alpha "), "second undo")
    record("04-second-undo-removes-the-previous-word", text(s) == "alpha ", text=text(s))

    xdo("key", "ctrl+y")
    s = wait(settled("alpha beta "), "redo")
    record("05-redo-restores-one-word", text(s) == "alpha beta ", text=text(s))

    # Moving the caret splits the group: the inserted "X" is undone alone.
    xdo("key", "Left")
    xdo("type", "--delay", "40", "X")
    wait(settled("alpha betaX "), "inserted")
    xdo("key", "ctrl+z")
    s = wait(settled("alpha beta "), "undo insertion")
    record("06-caret-move-splits-groups", text(s) == "alpha beta ", text=text(s))
finally:
    (output / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
    if app.poll() is None:
        app.terminate()
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired:
            app.kill()
