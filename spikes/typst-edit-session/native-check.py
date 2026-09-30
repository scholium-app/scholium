#!/usr/bin/env python3
"""Run under isolated Xvfb; verify real native editing using scene-derived positions."""

import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]
OUTPUT = Path(sys.argv[1]).resolve()
OUTPUT.mkdir(parents=True, exist_ok=True)
if any(OUTPUT.iterdir()):
    raise SystemExit("output directory must be empty")
target = Path(os.environ.get("CARGO_TARGET_DIR", REPO / "spikes/render-latency/target"))
environment = dict(os.environ, TYPST_EDIT_AUDIT="1", TYPST_EDIT_LAYOUT_DELAY_MS="200")
environment.pop("WAYLAND_DISPLAY", None)
environment["WINIT_UNIX_BACKEND"] = "x11"
environment["WINIT_X11_SCALE_FACTOR"] = "1"
log_path = OUTPUT / "app.log"


def latest():
    events = [json.loads(line[6:]) for line in log_path.read_text().splitlines() if line.startswith("AUDIT ")]
    return events[-1] if events else None


def wait(predicate, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if app.poll() is not None:
            raise RuntimeError(f"window exited: {log_path.read_text()}")
        state = latest()
        if state and predicate(state):
            return state
        time.sleep(0.03)
    raise RuntimeError(f"native state timed out: {latest()}")


def command(*args):
    subprocess.run(["xdotool", *map(str, args)], check=True, env=environment)


def type_text(text):
    command("type", "--clearmodifiers", "--delay", "10", text)


def text(state, leaf):
    return next(item["text"] for item in state["texts"] if item["leaf"] == leaf)


def click(state, leaf, byte):
    caret = next(item for item in state["carets"] if item["leaf"] == leaf and item["byte"] == byte)
    command("mousemove", "--window", window, round(caret["x"]), round(caret["y"]))
    command("click", 1)


def record(name, state):
    (OUTPUT / f"{name}.json").write_text(json.dumps(state, ensure_ascii=False, indent=2) + "\n")
    subprocess.run(["import", "-window", str(window), str(OUTPUT / f"{name}.png")], check=True, env=environment)
    print(f"PASS {name}", flush=True)


with log_path.open("w") as log:
    app = subprocess.Popen([str(target / "release/typst-edit-window")], env=environment, stdout=log, stderr=subprocess.STDOUT)
    try:
        state = wait(lambda state: state["current"])
        window = subprocess.check_output(["xdotool", "search", "--name", "Typst edit kernel"], env=environment, text=True).splitlines()[0]
        command("windowfocus", window)
        denominator = state["cursor_leaf"]
        assert text(state, denominator) == ""
        record("initial-hole", state)
        click(state, denominator, 0)
        type_text("12")
        state = wait(lambda s: s["current"] and text(s, denominator) == "12")
        assert state["cursor_byte"] == 2
        record("filled-denominator", state)
        command("key", "--clearmodifiers", "ctrl+slash")
        state = wait(lambda s: s["current"] and s["cursor_leaf"] != denominator)
        nested_denominator = state["cursor_leaf"]
        assert text(state, nested_denominator) == "" and state["rules"] == 2
        type_text("3")
        state = wait(lambda s: s["current"] and text(s, nested_denominator) == "3")
        record("nested-fraction", state)
        command("key", "--clearmodifiers", "ctrl+z")
        state = wait(lambda s: s["current"] and text(s, nested_denominator) == "")
        record("undo-restores-hole", state)
        body = next(item["leaf"] for item in state["texts"] if item["text"].startswith("中文"))
        old = state
        type_text("q")
        wait(lambda s: not s["current"] and text(s, nested_denominator) == "q")
        click(old, body, 3)
        type_text("w")
        state = wait(lambda s: s["current"] and text(s, nested_denominator) == "qw")
        assert state["cursor_leaf"] == nested_denominator and text(state, body).startswith("中文")
        record("pending-click-ignored", state)
        click(state, body, 3)
        type_text("ABC")
        state = wait(lambda s: s["current"] and text(s, body).startswith("中ABC文"))
        command("key", "--clearmodifiers", "Left", "BackSpace")
        state = wait(lambda s: s["current"] and text(s, body).startswith("中AC文"))
        command("key", "--clearmodifiers", "ctrl+z")
        state = wait(lambda s: s["current"] and text(s, body).startswith("中ABC文"))
        record("utf8-body-edit-and-undo", state)
        states = [json.loads(line[6:]) for line in log_path.read_text().splitlines() if line.startswith("AUDIT ")]
        assert all(not s["committed_text_echo"] for s in states)
        assert any(not s["current"] for s in states)
        (OUTPUT / "summary.json").write_text(json.dumps({"passed":6, "failed":0, "artificial_layout_delay_ms":200, "audit_states":len(states)}, indent=2) + "\n")
    finally:
        app.terminate()
        try:
            app.wait(timeout=5)
        except subprocess.TimeoutExpired:
            app.kill()
