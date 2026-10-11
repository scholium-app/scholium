#!/usr/bin/env python3
"""Real-window source workspace: generated Typst, A4 preview, click-to-locate, return to edit.

Run under Xvfb from the repository root:
  xvfb-run -a -s '-screen 0 1280x900x24' python3 spikes/typst-main-editor/source-view-check.py \\
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
    raise RuntimeError(f"timeout: {what}: {state() and {k: state()[k] for k in ('source_mode', 'preview')}}")


def xdo(*args):
    subprocess.run(["xdotool", *map(str, args)], check=True, env=env)


def paste(value):
    subprocess.run(["xclip", "-selection", "clipboard"], input=value.encode(), env=env, check=True)
    xdo("key", "--clearmodifiers", "ctrl+v")


def shot(name):
    time.sleep(0.12)  # audit precedes GPU presentation; not a latency measurement
    subprocess.run(["import", "-window", window, str(output / f"{name}.png")], check=True, env=env)


def record(name, ok, **detail):
    results[name] = {"pass": bool(ok), **detail}
    print(("PASS " if ok else "FAIL ") + name, flush=True)
    if not ok:
        raise AssertionError(f"{name}: {detail}")


def texts(s):
    return [b["body"]["text"] for block in s["snapshot"]["blocks"] for b in block["content"]
            if b["body"]["kind"] == "text"]


with log_path.open("w") as log:
    app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
try:
    wait(lambda s: s["current"], "first scene")
    window = subprocess.check_output(["xdotool", "search", "--pid", str(app.pid)], env=env, text=True).split()[0]
    xdo("windowfocus", window)
    paste("第一段 alpha\n第二段 beta\n第三段 gamma")
    xdo("type", "--delay", "20", " $")
    xdo("type", "x")
    xdo("key", "--clearmodifiers", "ctrl+slash")
    edited = wait(lambda s: s["current"] and texts(s)[:3] == ["第一段 alpha", "第二段 beta", "第三段 gamma "],
                  "edited document")
    record("01-document-edited", True, texts=texts(edited))

    xdo("key", "--clearmodifiers", "ctrl+2")
    s = wait(lambda s: s["source_mode"] and s["preview"]["shown"] and s["preview"]["rect"]
             and len(s["preview"]["anchors"] or []) == 3, "preview shown")
    shot("02-source-and-preview")
    record("02-source-and-preview", s["preview"]["pages"] == 1 and s["preview"]["error"] is None,
           preview={k: s["preview"][k] for k in ("generation", "pages", "error")})

    anchors, rect, scale = s["preview"]["anchors"], s["preview"]["rect"], s["preview"]["scale"]
    y_pt = (anchors[0]["y"] + anchors[1]["y"]) / 2
    second = s["body_leaves"][1]["leaf"]
    xdo("mousemove", "--window", window, round((rect[0] + rect[2]) / 2), round(rect[1] + y_pt * scale))
    xdo("click", 1)
    s = wait(lambda s: s["cursor_leaf"] == second and s["cursor_byte"] == 0, "caret moved to clicked block")
    shot("03-located-second-block")
    record("03-click-locates-block", True, cursor=[s["cursor_leaf"], s["cursor_byte"]])

    generation = s["preview"]["generation"]
    xdo("key", "--clearmodifiers", "ctrl+1")
    wait(lambda s: not s["source_mode"], "back to the editor")
    xdo("type", "X")
    s = wait(lambda s: s["current"] and texts(s)[1] == "X第二段 beta", "typing at the located block")
    record("04-typing-after-return", True, texts=texts(s))

    xdo("key", "--clearmodifiers", "ctrl+2")
    s = wait(lambda s: s["source_mode"] and s["preview"]["shown"]
             and s["preview"]["generation"] > generation, "preview of the new snapshot")
    shot("05-preview-after-edit")
    record("05-preview-follows-edits", s["preview"]["error"] is None,
           generation=[generation, s["preview"]["generation"]])
finally:
    (output / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
    if app.poll() is None:
        app.terminate()
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired:
            app.kill()
