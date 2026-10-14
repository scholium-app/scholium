#!/usr/bin/env python3
"""Real-window superscript/subscript input: type `x^2 + y_i`, inspect the model, screenshot.

Run under Xvfb from the repository root:
  xvfb-run -a -s '-screen 0 1280x900x24' python3 spikes/typst-main-editor/math-attach-check.py \\
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


def formula(s):
    for block in s["snapshot"]["blocks"]:
        for inline in block["content"]:
            if inline["body"]["kind"] == "math":
                return inline["body"]["root"]
    return None


def shape(node):
    """Compact structure: text -> its string, hole -> '_', row -> list, attach -> dict."""
    body = node["body"]
    kind = body["kind"]
    if kind == "text":
        return body["text"]
    if kind == "hole":
        return "_"
    if kind == "row":
        return [shape(c) for c in body["children"]]
    if kind == "attach":
        out = {"base": shape(body["base"])}
        for slot in ("sub", "sup"):
            if body.get(slot):
                out[slot] = shape(body[slot])
        return out
    return kind


with log_path.open("w") as log:
    app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
try:
    wait(lambda s: s["current"], "first scene")
    window = subprocess.check_output(["xdotool", "search", "--pid", str(app.pid)], env=env, text=True).split()[0]
    xdo("windowfocus", window)
    xdo("type", "--delay", "40", "$x^2")
    xdo("key", "Right")
    xdo("type", "--delay", "40", " + y_i")
    expected = [{"base": "x", "sup": "2"}, " + ", {"base": "y", "sub": "i"}]
    # Keys arrive asynchronously: wait for the complete expected structure (not just the
    # first appearance of a slot), then assert it below. On timeout the last state is
    # asserted, so a wrong structure still fails with its actual shape.
    try:
        s = wait(lambda s: s["current"] and formula(s) and shape(formula(s)) == expected,
                 "complete formula typed", timeout=20)
    except RuntimeError:
        s = state()
    record("01-typed-scripts-are-nodes", shape(formula(s)) == expected, shape=shape(formula(s)))
    leaves = json.dumps(s["snapshot"])
    record("02-no-literal-markers", "^" not in leaves and '"_"' not in leaves.replace('"kind":"hole"', ""),
           note="no caret or underscore stored in math text")
    shot("03-visual-x2-plus-yi")

    xdo("key", "--clearmodifiers", "ctrl+shift+m")
    s = wait(lambda s: s["source_mode"] and s["preview"]["shown"] and s["preview"]["rect"], "preview shown")
    shot("04-preview-x2-plus-yi")
    record("05-preview-compiles", s["preview"]["error"] is None and s["preview"]["pages"] == 1,
           preview={k: s["preview"][k] for k in ("generation", "pages", "error")})
finally:
    (output / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
    if app.poll() is None:
        app.terminate()
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired:
            app.kill()
