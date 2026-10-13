#!/usr/bin/env python3
"""Real-window A4 surface check: the Visual editing sheet and the Ctrl+2 A4 preview show the same
headings and paragraphs; screenshots are taken side by side (see docs/log/2026-10-12-a4-editor-surface.md).

Run under Xvfb from the repository root:
  xvfb-run -a -s '-screen 0 1280x900x24' python3 spikes/typst-main-editor/a4-surface-check.py \\
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
# Ribbon buttons (1280x900 window): paragraph styles 标题 1 / 标题 2.
HEADING_1 = (578, 79)
HEADING_2 = (646, 79)
PARAGRAPHS = (
    "第一章 排版与输出\n1.1 Line breaking 与断行\n"
    "所见即所得的排版编辑器必须让编辑面与输出完全一致：字体、字号、行距、缩进与断行都不能因为预览而改变。"
    "This sentence mixes English words with 中文字符 so that the line breaker has to choose between Latin "
    "and CJK break opportunities, 并且足够长以便换行三次以上。\n"
    "第二段同样混排。Typesetting parity means the same glyphs land on the same lines in both the quick "
    "preview and the continuous editing surface, 无论段落多长，每一行的首个字符都应当相同。"
)


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
    paste(PARAGRAPHS)
    wait(lambda s: s["current"] and len(texts(s)) == 4, "four paragraphs pasted")
    xdo("key", "--clearmodifiers", "ctrl+Home")
    xdo("mousemove", "--window", window, *HEADING_1)
    xdo("click", 1)
    xdo("key", "Down")
    xdo("mousemove", "--window", window, *HEADING_2)
    xdo("click", 1)
    s = wait(lambda s: s["current"] and [b["kind"] for b in s["snapshot"]["blocks"]]
             == ["Heading1", "Heading2", "Paragraph", "Paragraph"], "heading styles applied")
    shot("01-visual-editor")
    record("01-visual-editor", True, kinds=[b["kind"] for b in s["snapshot"]["blocks"]])

    xdo("key", "--clearmodifiers", "ctrl+2")
    s = wait(lambda s: s["source_mode"] and s["preview"]["shown"] and s["preview"]["rect"]
             and len(s["preview"]["anchors"] or []) == 4, "preview shown")
    shot("02-a4-preview")
    record("02-a4-preview", s["preview"]["pages"] == 1 and s["preview"]["error"] is None,
           preview={k: s["preview"][k] for k in ("generation", "pages", "error")})
finally:
    (output / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
    if app.poll() is None:
        app.terminate()
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired:
            app.kill()
