#!/usr/bin/env python3
"""Real-window pointer fixes and the math indicator.

Clicks below the last line, double/triple click selection and the formula tint, checked
against AUDIT data and screenshots. Run under Xvfb from the repository root:
  xvfb-run -a -s '-screen 0 1280x900x24' python3 spikes/typst-main-editor/interaction-check.py \\
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
    path = output / f"{name}.png"
    subprocess.run(["import", "-window", window, str(path)], check=True, env=env)
    return path


def record(name, ok, **detail):
    results[name] = {"pass": bool(ok), **detail}
    print(("PASS " if ok else "FAIL ") + name, flush=True)
    if not ok:
        raise AssertionError(f"{name}: {detail}")


def caret(s, leaf, byte):
    return next(c for c in s["carets"] if c["leaf"] == leaf and c["byte"] == byte and c["exact"])


def body_leaves(s):
    return s["body_leaves"]


def point_at(x, y):
    xdo("mousemove", "--window", window, round(x), round(y))


def tinted_pixels(path):
    """Pixels with the accent-tint hue (blue above red, light): the formula background."""
    ppm = subprocess.run(["convert", str(path), "ppm:-"], check=True, capture_output=True).stdout
    header, data = ppm.split(b"\n255\n", 1)
    count = 0
    for i in range(0, len(data), 3):
        r, g, b = data[i], data[i + 1], data[i + 2]
        if b > r + 6 and r > 200 and b > 235:
            count += 1
    return count


with log_path.open("w") as log:
    app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
try:
    wait(lambda s: s["current"], "first scene")
    window = subprocess.check_output(["xdotool", "search", "--pid", str(app.pid)], env=env, text=True).split()[0]
    xdo("windowfocus", window)
    xdo("type", "--delay", "30", "hello world, again")
    xdo("key", "Return")
    xdo("type", "--delay", "30", "beta $12")
    xdo("key", "ctrl+slash")
    xdo("type", "--delay", "30", "3")
    s = wait(lambda s: s["current"] and len(s["snapshot"]["blocks"]) > 1
         and any(i["body"]["kind"] == "math" for i in s["snapshot"]["blocks"][1]["content"])
         and "3" in json.dumps(s["snapshot"]["blocks"][1]), "formula")
    inside = shot("01-caret-in-formula")
    xdo("key", "ctrl+m")
    xdo("type", "--delay", "30", " end")
    xdo("key", "Return")
    xdo("type", "--delay", "30", "gamma")
    s = wait(lambda s: s["current"] and len(body_leaves(s)) == 4 and s["cursor_byte"] == 5, "three paragraphs")
    outside = shot("02-caret-in-body")
    record("01-math-indicator-tints-only-inside-the-formula",
           tinted_pixels(inside) > 200 and tinted_pixels(outside) < tinted_pixels(inside) // 4,
           inside=tinted_pixels(inside), outside=tinted_pixels(outside))

    # 1. A click far below the last line lands at the end of the last paragraph, not in the formula.
    leaves = body_leaves(s)
    last = leaves[-1]
    last_caret = caret(s, last["leaf"], 0)
    point_at(last_caret["x"] + 40, last_caret["y"] + 200)
    xdo("click", 1)
    s = wait(lambda s: s["cursor_leaf"] == last["leaf"] and s["cursor_byte"] == last["bytes"], "click below")
    record("02-click-below-last-line-goes-to-document-end", True, cursor_byte=s["cursor_byte"])

    # 2. Double click selects the word under the pointer.
    first = leaves[0]
    # Immediately after the far click above: egui alone would report this double click as a
    # triple one; the app counts only nearby consecutive presses, so the word is selected.
    inside_word = caret(s, first["leaf"], 8)
    point_at(inside_word["x"], inside_word["y"])
    xdo("click", "--repeat", 2, "--delay", 80, 1)
    s = wait(lambda s: s["selection"] is not None, "word selection")
    sel = s["selection"]
    record("03-double-click-selects-the-word",
           (sel["start_leaf"], sel["start_byte"], sel["end_byte"]) == (first["leaf"], 6, 11), selection=sel)
    shot("03-double-click-word-selected")

    # 3. Triple click selects the block's body text.
    xdo("click", "--repeat", 3, "--delay", 80, 1)
    s = wait(lambda s: s["selection"] is not None and s["selection"]["end_byte"] == first["bytes"], "block selection")
    sel = s["selection"]
    record("04-triple-click-selects-the-block", (sel["start_byte"], sel["end_byte"]) == (0, first["bytes"]), selection=sel)
    shot("04-triple-click-block-selected")
finally:
    (output / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
    if app.poll() is None:
        app.terminate()
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired:
            app.kill()
