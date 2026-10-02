#!/usr/bin/env python3
"""Observe a same-frame revision barcode in an isolated native X11 window for 30s."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from PIL import ImageGrab

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]
OUTPUT = Path(sys.argv[1]).resolve()
OUTPUT.mkdir(parents=True, exist_ok=True)
assert not any(OUTPUT.iterdir()), "output directory must be empty"
env = dict(os.environ, TYPST_EDIT_AUDIT="1", TYPST_EDIT_PRESENT_PROBE="1",
           WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR="1")
for name in ["WAYLAND_DISPLAY", "TYPST_EDIT_LAYOUT_DELAY_MS"]:
    env.pop(name, None)
target = Path(env.get("CARGO_TARGET_DIR", REPO / "spikes/render-latency/target"))
log_path = OUTPUT / "app.log"


def command(*args):
    return subprocess.check_output(["xdotool", *map(str, args)], env=env, text=True)


def latest():
    global pending_lines
    data = pending_lines + reader.read()
    lines = data.split("\n")
    pending_lines = lines.pop()
    for line in lines:
        if line.startswith("AUDIT "):
            states.append(json.loads(line[6:]))
    return states[-1] if states else None


def wait_state(predicate, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if app.poll() is not None:
            raise RuntimeError("native window exited")
        state = latest()
        if state and predicate(state):
            return state
        time.sleep(0.001)
    raise RuntimeError(f"state timed out: {latest()}")


def revision(state):
    x, y = round(state["probe_x"]), round(state["probe_y"])
    pixels = ImageGrab.grab(bbox=(x, y, x + 64, y + 4), xdisplay=env["DISPLAY"])
    return sum((pixels.getpixel((2 * bit + 1, 1))[0] > 127) << bit for bit in range(32))


def p95(values):
    values = sorted(values)
    return values[(len(values) * 95 + 99) // 100 - 1]


states = []
pending_lines = ""
with log_path.open("w") as log:
    app = subprocess.Popen([str(target / "release/typst-edit-window")], env=env, stdout=log, stderr=subprocess.STDOUT)
    reader = log_path.open()
    try:
        state = wait_state(lambda s: s["current"])
        window = command("search", "--name", "Typst edit kernel").splitlines()[0]
        command("windowfocus", window)
        command("windowmove", window, 0, 0)
        deadline = time.monotonic() + 2
        while revision(state) != state["revision"]:
            assert time.monotonic() < deadline, "initial frame did not become visible"
            time.sleep(0.001)
        start = time.monotonic()
        rows = []
        for sample in range(300):
            due = start + sample / 10
            time.sleep(max(0, due - time.monotonic()))
            before = latest()["revision"]
            launched_ns = time.time_ns()
            if sample % 2 == 0:
                command("type", "--clearmodifiers", "--delay", 0, "q")
            else:
                command("key", "--clearmodifiers", "BackSpace")
            state = wait_state(lambda s: s["current"] and s["revision"] > before)
            deadline = time.monotonic() + 2
            while revision(state) != state["revision"]:
                assert time.monotonic() < deadline, "frame did not become visible"
                time.sleep(0.001)
            seen_ns = time.time_ns()
            assert state["projection"]["received"] == 1
            assert state["projection"]["built"] <= 5
            rows.append({"revision":state["revision"],
                         "accepted_to_observed_ms":(seen_ns-state["accepted_ns"])/1e6,
                         "accepted_to_ready_ms":(state["ready_ns"]-state["accepted_ns"])/1e6,
                         "ready_to_adopted_ms":(state["adopted_ns"]-state["ready_ns"])/1e6,
                         "adopted_to_observed_ms":(seen_ns-state["adopted_ns"])/1e6,
                         "injection_to_observed_ms":(seen_ns-launched_ns)/1e6,
                         "projection_ms":state["projection_ms"],
                         "layout_ms":state["layout_ms"], "raster_ms":state["raster_ms"]})
        time.sleep(max(0, start + 30 - time.monotonic()))
        elapsed = time.monotonic() - start
        accept = [row["accepted_to_observed_ms"] for row in rows]
        summary = {"samples":len(rows), "elapsed_seconds":elapsed, "event_hz":len(rows)/elapsed,
                   "accepted_to_observed_p95_ms":p95(accept),
                   "accepted_to_ready_p95_ms":p95([r["accepted_to_ready_ms"] for r in rows]),
                   "ready_to_adopted_p95_ms":p95([r["ready_to_adopted_ms"] for r in rows]),
                   "adopted_to_observed_p95_ms":p95([r["adopted_to_observed_ms"] for r in rows]),
                   "injection_to_observed_p95_ms":p95([r["injection_to_observed_ms"] for r in rows]),
                   "projection_p95_ms":p95([r["projection_ms"] for r in rows]),
                   "layout_p95_ms":p95([r["layout_ms"] for r in rows]),
                   "raster_p95_ms":p95([r["raster_ms"] for r in rows]),
                   "within_32ms":p95(accept)<=32, "platform":"Xvfb X11 scale 1, default vsync",
                   "measurement":"semantic acceptance to first captured same-frame revision barcode; capture latency included"}
        (OUTPUT/"samples.json").write_text(json.dumps(rows, indent=2)+"\n")
        (OUTPUT/"summary.json").write_text(json.dumps(summary, indent=2)+"\n")
        ImageGrab.grab(xdisplay=env["DISPLAY"]).save(OUTPUT/"last-frame.png")
        print(json.dumps(summary), flush=True)
        assert rows[-1]["revision"] - rows[0]["revision"] == 299
        assert all(not s["committed_text_echo"] for s in states)
        assert elapsed >= 29.9
        if not summary["within_32ms"]:
            raise SystemExit("FAIL acceptance-to-observed p95 exceeds 32 ms")
    finally:
        reader.close()
        app.terminate()
        try:
            app.wait(timeout=5)
        except subprocess.TimeoutExpired:
            app.kill()
