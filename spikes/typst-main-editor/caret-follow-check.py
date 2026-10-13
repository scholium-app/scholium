#!/usr/bin/env python3
"""Real-window caret following at scale 1.5 (docs/log/2026-10-13-caret-follow.md).

Types a first paragraph and 45 more lines, then asserts the caret (AUDIT `follow`) lies inside the
visible page range with the margin; a wheel scroll back up is not undone while idle; one more typed
character brings the caret back into view; Ctrl+Home / Ctrl+End reach the page ends.

Run under Xvfb from the repository root:
  xvfb-run -a -s '-screen 0 1280x900x24' python3 spikes/typst-main-editor/caret-follow-check.py \\
    target/debug/scholium-app /tmp/out
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

LINES = 45
WHEEL_CLICKS = 25

binary = Path(sys.argv[1]).resolve()
output = Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
if any(output.iterdir()):
    raise SystemExit("output directory must be empty")
env = dict(os.environ, SCHOLIUM_SESSION_FILE=str(output / "doc.sqlite"), SCHOLIUM_EDITOR_AUDIT="1",
           WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR="1.5")
env.pop("WAYLAND_DISPLAY", None)
log_path = output / "app.log"
results = {}


def state():
    lines = [l for l in log_path.read_text().split("\n")[:-1] if l.startswith("AUDIT ")]
    return json.loads(lines[-1][6:]) if lines else None


def wait(predicate, what, timeout=90):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        current = state()
        if current and predicate(current):
            return current
        time.sleep(0.05)
    raise RuntimeError(f"timeout: {what}: {state() and state().get('follow')}")


def xdo(*args):
    subprocess.run(["xdotool", *map(str, args)], check=True, env=env)


def record(name, ok, **detail):
    results[name] = {"pass": bool(ok), **detail}
    print(("PASS " if ok else "FAIL ") + name, flush=True)
    if not ok:
        raise AssertionError(f"{name}: {detail}")


def settled(s):
    return s["current"] and s["deferred"] == 0 and s["follow"] and not s["follow"]["pending"]


def margin(f):
    return f["margin_lines"] * (f["caret_bottom_pt"] - f["caret_top_pt"])


def visible(s):
    f = s["follow"]
    return (settled(s) and f["caret_top_pt"] - margin(f) >= f["view_top_pt"] - 1
            and f["caret_bottom_pt"] + margin(f) <= f["view_bottom_pt"] + 1)


with log_path.open("w") as log:
    app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
try:
    wait(lambda s: s["current"], "first scene")
    window = subprocess.check_output(["xdotool", "search", "--pid", str(app.pid)], env=env, text=True).split()[0]
    xdo("windowfocus", window)
    xdo("type", "--delay", "12", "Top paragraph.")
    for n in range(1, LINES + 1):
        xdo("key", "Return")
        xdo("type", "--delay", "12", f"Body line number {n} with some words.")
    s = wait(visible, "caret visible after typing")
    f = s["follow"]
    record("01-caret-is-visible-after-typing-past-the-viewport",
           f["view_top_pt"] > 100 and f["caret_bottom_pt"] > f["view_top_pt"], follow=f)

    xdo("mousemove", "--window", window, 640, 400)
    for _ in range(WHEEL_CLICKS):
        xdo("click", 4)
        time.sleep(0.03)
    s = wait(lambda s: s["follow"] and s["follow"]["view_top_pt"] < f["view_top_pt"] - 100
             and s["follow"]["caret_bottom_pt"] > s["follow"]["view_bottom_pt"], "wheel scrolled up")
    time.sleep(1.5)
    after = state()["follow"]
    record("02-wheel-scroll-is-not-undone-while-idle",
           abs(after["view_top_pt"] - s["follow"]["view_top_pt"]) < 1.0
           and after["caret_bottom_pt"] > after["view_bottom_pt"], before=s["follow"], after=after)

    xdo("type", "--delay", "40", "!")
    s = wait(visible, "caret visible again after typing one character")
    record("03-typing-one-character-brings-the-caret-back", True, follow=s["follow"])

    xdo("key", "ctrl+Home")
    s = wait(lambda s: settled(s) and s["follow"]["view_top_pt"] <= 0.5, "page top visible")
    record("04-ctrl-home-shows-the-top-of-the-page", True, follow=s["follow"])
    xdo("key", "ctrl+End")
    s = wait(lambda s: visible(s) and s["follow"]["view_top_pt"] > f["view_top_pt"] - 100, "caret visible at the end")
    record("05-ctrl-end-returns-to-the-caret-at-the-end", True, follow=s["follow"])
finally:
    (output / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
    if app.poll() is None:
        app.terminate()
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired:
            app.kill()
