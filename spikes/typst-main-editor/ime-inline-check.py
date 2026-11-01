#!/usr/bin/env python3
"""Private X11 window: normal egui IME events, Typst reflow, anchor, cancel and commit.

The opt-in audit seam supplies Preedit/Commit independent of the host's installed IME.
It does not claim to test fcitx5 or Wayland protocol delivery.
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
assert not any(output.iterdir()), "output directory must be empty"
assert os.environ.get("DISPLAY") not in (None, ":0"), "run only under private Xvfb"
channel = output / "ime-event.json"
log_path = output / "app.log"
env = dict(os.environ, SCHOLIUM_SESSION_FILE=str(output / "doc.sqlite"),
           SCHOLIUM_EDITOR_AUDIT="1", SCHOLIUM_IME_CHECK_FILE=str(channel),
           WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR="1")
env.pop("WAYLAND_DISPLAY", None)
results = {}


def state():
    lines = [line for line in log_path.read_text().split("\n")[:-1] if line.startswith("AUDIT ")]
    return json.loads(lines[-1][6:]) if lines else None


def wait(predicate, what):
    end = time.monotonic() + 60
    while time.monotonic() < end:
        assert app.poll() is None, log_path.read_text()
        s = state()
        if s and predicate(s):
            return s
        time.sleep(0.05)
    raise AssertionError(f"timeout: {what}: {state()}")


def xdo(*args):
    subprocess.run(["xdotool", *map(str, args)], env=env, check=True)


def ime(**event):
    pending = channel.with_suffix(".tmp")
    pending.write_text(json.dumps(event))
    pending.replace(channel)


def text(s):
    return s["snapshot"]["blocks"][0]["content"][0]["body"]["text"]


def current(s):
    return s["current"] and s["deferred"] == 0


def x_at(s, leaf, byte):
    return max(c["x"] for c in s["carets"] if c["leaf"] == leaf and c["byte"] == byte)


def record(name, detail):
    results[name] = detail
    print(f"PASS {name}: {detail}", flush=True)


with log_path.open("w") as log:
    app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
try:
    wait(current, "first scene")
    window = subprocess.check_output(["xdotool", "search", "--pid", str(app.pid)], env=env, text=True).split()[0]
    xdo("windowfocus", window)
    xdo("type", "--delay", "30", "abZZ")
    wait(lambda s: current(s) and text(s) == "abZZ", "body")
    xdo("key", "Left", "Left")
    before = wait(lambda s: current(s) and s["cursor_byte"] == 2, "middle caret")
    leaf = before["cursor_leaf"]
    insertion_x = x_at(before, leaf, 2)
    old_end_x = x_at(before, leaf, 4)
    ime(preedit="中文")
    composing = wait(lambda s: current(s) and s["preedit_caret"] is not None, "inline preedit")
    assert composing["snapshot"] == before["snapshot"]
    assert composing["undo_depth"] == before["undo_depth"]
    assert composing["page_text_draws"] == 0 and composing["source_reads"] == 0
    assert not composing["document_geometry"]
    preedit_leaf = composing["preedit_caret"]["leaf"]
    start_x = x_at(composing, preedit_leaf, 0)
    end_x = x_at(composing, preedit_leaf, len("中文".encode()))
    width = end_x - start_x
    following_x = x_at(composing, leaf, 0)  # suffix split-run start, queries suspended
    following_end_x = x_at(composing, leaf, 2)
    assert abs(start_x - insertion_x) < 0.1
    assert width > 10 and abs(following_x - end_x) < 0.1, (following_x, end_x)
    assert abs(following_end_x - old_end_x - width) < 0.1
    assert composing["ime_area"] == composing["ime_caret"]
    assert abs(composing["ime_area"][0] - end_x) < 0.1
    record("inline-no-overlap-and-caret-anchor", {"preedit_width_px": width,
           "following_shift_px": following_end_x - old_end_x, "ime_area": composing["ime_area"]})
    time.sleep(0.12)
    subprocess.run(["import", "-window", window, str(output / "inline-preedit.png")], env=env, check=True)

    ime(preedit="中文", cursor_chars=1)
    inside = wait(lambda s: current(s) and s["preedit_caret"] and s["preedit_caret"]["byte"] == 3, "IME cursor")
    assert start_x < inside["ime_area"][0] < end_x
    record("ime-cursor-inside-preedit", inside["ime_area"])
    ime(preedit="")
    cancelled = wait(lambda s: current(s) and s["document_geometry"], "cancel")
    assert cancelled["snapshot"] == before["snapshot"]
    assert abs(x_at(cancelled, leaf, 4) - old_end_x) < 0.1
    record("cancel-restores-document-geometry", True)

    ime(preedit="中文")
    wait(lambda s: current(s) and s["preedit_caret"] is not None, "second preedit")
    ime(commit="中文")
    committed = wait(lambda s: current(s) and text(s) == "ab中文ZZ" and s["document_geometry"], "commit")
    assert committed["undo_depth"] == before["undo_depth"] + 1
    assert committed["preedit_caret"] is None and committed["page_text_draws"] == 0
    assert abs(x_at(committed, leaf, len("ab中文ZZ".encode())) - following_end_x) < 0.1
    record("commit-once-without-position-jump", True)
    xdo("key", "ctrl+z")
    wait(lambda s: current(s) and text(s) == "abZZ", "undo commit")
    record("undo-contains-only-committed-text", True)
finally:
    (output / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
    if app.poll() is None:
        app.terminate()
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired:
            app.kill()
            app.wait(timeout=15)
