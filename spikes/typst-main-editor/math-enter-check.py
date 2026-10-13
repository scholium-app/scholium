#!/usr/bin/env python3
"""Real-window Enter inside a formula, long formulas and the overflow warning flag.

Run under Xvfb from the repository root:
  xvfb-run -a -s '-screen 0 1280x900x24' python3 spikes/typst-main-editor/math-enter-check.py \\
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


def blocks(s):
    return s["snapshot"]["blocks"]


def formulas(s):
    return [i for b in blocks(s) for i in b["content"] if i["body"]["kind"] == "math"]


def body_caret(s):
    """The caret is at byte 0 of a body (non-formula) leaf."""
    leaves = {l["leaf"] for l in s["body_leaves"]}
    return s["cursor_byte"] == 0 and s["cursor_leaf"] in leaves


def settled(predicate):
    return lambda s: s["current"] and predicate(s)


with log_path.open("w") as log:
    app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
try:
    wait(lambda s: s["current"], "first scene")
    window = subprocess.check_output(["xdotool", "search", "--pid", str(app.pid)], env=env, text=True).split()[0]
    xdo("windowfocus", window)

    xdo("type", "--delay", "40", "$x^2")
    s = wait(settled(lambda s: len(formulas(s)) == 1 and s["cursor_leaf"] not in {l["leaf"] for l in s["body_leaves"]}),
             "formula with caret in the superscript")
    before = blocks(s)
    xdo("key", "Return")
    s = wait(settled(lambda s: len(blocks(s)) == 2), "Enter in the superscript splits the block")
    record("01-enter-in-math-splits-after-the-formula",
           len(formulas(s)) == 1 and formulas(s)[0] in blocks(s)[0]["content"] and body_caret(s),
           blocks=len(blocks(s)), caret=[s["cursor_leaf"], s["cursor_byte"]])
    shot("02-after-enter")

    xdo("key", "ctrl+z")
    s = wait(settled(lambda s: len(blocks(s)) == 1), "one undo restores the paragraph")
    record("03-one-undo-restores-the-formula", blocks(s) == before and not body_caret(s),
           blocks=len(blocks(s)))

    xdo("key", "shift+Return")
    s = wait(settled(lambda s: len(blocks(s)) == 2), "Shift+Enter splits as well")
    record("04-shift-enter-does-the-same", body_caret(s) and len(formulas(s)) == 1)

    # A long operator formula with typed spaces breaks after operators and does not overflow.
    xdo("type", "--delay", "20", "$" + " + ".join("abcdefghijklmnopqrstuvwxyzabcdefghijklmn"))
    s = wait(settled(lambda s: "n" in json.dumps(formulas(s)[-1]) and s["cursor_byte"] > 0), "long sum typed")
    time.sleep(0.5)
    s = wait(settled(lambda s: s["layout_error"] is None), "long sum laid out")
    record("05-operator-formula-wraps-without-overflow", s["formula_overflow"] is False)
    shot("06-long-sum-editor")

    xdo("key", "Return")
    wait(settled(lambda s: len(blocks(s)) == 3), "Enter leaves the long formula")
    xdo("type", "--delay", "20", "$" + "abcdefghij" * 10)
    s = wait(settled(lambda s: len(formulas(s)) == 3 and s["formula_overflow"]), "overflow flag while inside")
    record("07-operator-free-run-reports-overflow", s["formula_overflow"] is True)
    shot("08-operator-free-run-editor")

    xdo("key", "Right")
    s = wait(settled(lambda s: not s["formula_overflow"]), "flag clears outside the formula")
    record("09-overflow-flag-clears-after-leaving", s["formula_overflow"] is False)

    xdo("key", "--clearmodifiers", "ctrl+2")
    s = wait(lambda s: s["source_mode"] and s["preview"]["shown"] and s["preview"]["rect"], "preview shown")
    shot("10-preview-long-formulas")
    record("11-preview-compiles", s["preview"]["error"] is None, preview={k: s["preview"][k] for k in ("pages", "error")})
finally:
    (output / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
    if app.poll() is None:
        app.terminate()
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired:
            app.kill()
