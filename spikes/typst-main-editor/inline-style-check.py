#!/usr/bin/env python3
"""Real-window inline styles: Ctrl+B / Ctrl+I on selected words and a pending typing style.

Run under Xvfb from the repository root:
  xvfb-run -a -s '-screen 0 1280x900x24' python3 spikes/typst-main-editor/inline-style-check.py \\
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


def runs(s):
    """Text runs of the first block as (text, style)."""
    return [(i["body"]["text"], i["body"]["style"]) for i in s["snapshot"]["blocks"][0]["content"]
            if i["body"]["kind"] == "text"]


def settled(expected):
    return lambda s: s["current"] and runs(s) == expected


with log_path.open("w") as log:
    app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
try:
    wait(lambda s: s["current"], "first scene")
    window = subprocess.check_output(["xdotool", "search", "--pid", str(app.pid)], env=env, text=True).split()[0]
    xdo("windowfocus", window)
    xdo("type", "--delay", "40", "alpha beta gamma delta")
    wait(lambda s: s["current"] and runs(s) == [("alpha beta gamma delta", "plain")], "typed")

    # Select "gamma" (caret is 6 graphemes from the end, the word is 5 long) and make it bold.
    xdo("key", *["Left"] * 6)
    xdo("key", *["shift+Left"] * 5)
    xdo("key", "ctrl+b")
    plain, strong = ("alpha beta ", "plain"), ("gamma", "strong")
    s = wait(settled([plain, strong, (" delta", "plain")]), "gamma bold")
    record("01-ctrl-b-bolds-only-the-selected-word", runs(s) == [plain, strong, (" delta", "plain")],
           runs=runs(s), selection=s["selection"] is not None)

    # Select "beta" from the line start and emphasize it.
    xdo("key", "Home")
    xdo("key", *["Right"] * 6)
    xdo("key", *["shift+Right"] * 4)
    xdo("key", "ctrl+i")
    expected = [("alpha ", "plain"), ("beta", "emphasis"), (" ", "plain"), strong, (" delta", "plain")]
    s = wait(settled(expected), "beta emphasis")
    record("02-ctrl-i-emphasizes-another-word", runs(s) == expected, runs=runs(s))
    shot("03-bold-and-emphasis-words")

    # No selection: arm bold at the end, then type; the new text is its own Strong leaf.
    xdo("key", "End")
    xdo("key", "ctrl+b")
    xdo("type", "--delay", "40", "ab")
    expected = expected + [("ab", "strong")]
    s = wait(settled(expected), "pending bold typed")
    record("04-pending-bold-types-a-strong-leaf", runs(s) == expected, runs=runs(s))
    shot("05-pending-bold-typed")

    # Arm plain inside bold text, then move the caret: the armed style is dropped, so Z stays bold.
    xdo("key", "ctrl+b")
    xdo("key", "Left")
    xdo("key", "Right")
    xdo("type", "--delay", "40", "Z")
    s = wait(lambda s: s["current"] and "Z" in json.dumps(runs(s)), "plain typed after move")
    record("06-caret-move-clears-pending-style", runs(s)[-1] == ("abZ", "strong"), runs=runs(s))
finally:
    (output / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
    if app.poll() is None:
        app.terminate()
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired:
            app.kill()
