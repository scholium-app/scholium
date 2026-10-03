#!/usr/bin/env python3
"""Native main-program workflow; run under Xvfb, never the standalone spike editor."""
import hashlib
import ctypes
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
env = dict(os.environ, SCHOLIUM_SESSION_FILE=str(output / "session.sqlite"), SCHOLIUM_EDITOR_AUDIT="1",
           WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR="1")
env.pop("WAYLAND_DISPLAY", None)
app = None
log_path = None
window = None


def entries(prefix="AUDIT "):
    return [json.loads(line[len(prefix):]) for line in log_path.read_text().splitlines()
            if line.startswith(prefix)]


def wait(predicate, timeout=30):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if app.poll() is not None:
            raise RuntimeError(f"application exited: {log_path.read_text()}")
        events = entries()
        if events and predicate(events[-1]):
            return events[-1]
        time.sleep(0.03)
    raise RuntimeError(f"state timeout: {entries()[-1] if entries() else log_path.read_text()}")


def command(*args):
    subprocess.run(["xdotool", *map(str, args)], check=True, env=env)


def type_text(value):
    command("type", "--clearmodifiers", "--delay", "15", value)


def paste(value):
    subprocess.run(["xclip", "-selection", "clipboard"], input=value.encode(), env=env, check=True)
    command("key", "--clearmodifiers", "ctrl+v")


def leaves(state):
    result = []

    def math(node):
        body = node["body"]
        if body["kind"] in ("text", "hole"):
            result.append((node["node"], body.get("text", "")))
        elif body["kind"] == "fraction":
            math(body["numerator"])
            math(body["denominator"])
        elif body["kind"] == "row":
            for child in body["children"]:
                math(child)

    for block in state["snapshot"]["blocks"]:
        for inline in block["content"]:
            if inline["body"]["kind"] == "text":
                result.append((inline["node"], inline["body"]["text"]))
            elif inline["body"]["kind"] == "math":
                math(inline["body"]["root"])
    return result


def values(state):
    return [value for _, value in leaves(state)]


def click(state, leaf_debug, byte):
    caret = next(c for c in state["carets"] if c["leaf"] == leaf_debug and c["byte"] == byte)
    command("mousemove", "--window", window, round(caret["x"]), round(caret["y"]))
    command("click", 1)


def caret_point(state, leaf, byte):
    caret = next(c for c in state["carets"] if c["leaf"] == leaf and c["byte"] == byte and c["exact"])
    return round(caret["x"]), round(caret["y"])


def drag(state, first, last):
    x, y = caret_point(state, *first)
    command("mousemove", "--window", window, x, y)
    command("mousedown", 1)
    wait(lambda s: s["cursor_leaf"] == first[0] and s["cursor_byte"] == first[1])
    x, y = caret_point(state, *last)
    command("mousemove", "--window", window, x, y)
    selected = wait(lambda s: s["selection"] is not None and s["selection_quads"] > 0)
    command("mouseup", 1)
    return selected



def record(name, state):
    assert state["backend"] == "Typst Content main app"
    assert state["current"]
    assert any(c["leaf"] == state["cursor_leaf"] and c["byte"] == state["cursor_byte"]
               for c in state["carets"]), "current cursor has same-scene geometry"
    assert state["page_text_draws"] == 0
    assert state["source_reads"] == 0
    (output / f"{name}.json").write_text(json.dumps(state, ensure_ascii=False, indent=2) + "\n")
    # Audit is emitted before the GPU swap. Allow presentation before capturing;
    # this capture delay is not an input latency measurement.
    time.sleep(0.12)
    subprocess.run(["import", "-window", str(window), str(output / f"{name}.png")], check=True, env=env)
    print(f"PASS {name}", flush=True)


def start(name):
    global app, log_path, window
    log_path = output / f"{name}.log"
    with log_path.open("w") as log:
        app = subprocess.Popen([str(binary), "--typst-editor"], env=env, stdout=log, stderr=subprocess.STDOUT)
    state = wait(lambda s: s["current"])
    window = subprocess.check_output(["xdotool", "search", "--pid", str(app.pid)], env=env, text=True).splitlines()[0]
    command("windowfocus", window)
    return state


def close():
    # No window manager runs under Xvfb. Send its standard close request instead
    # of xdotool windowclose, which destroys the X drawable behind winit.
    x11 = ctypes.CDLL("libX11.so.6")
    class Data(ctypes.Union):
        _fields_ = [("l", ctypes.c_long * 5)]
    class ClientMessage(ctypes.Structure):
        _fields_ = [("type", ctypes.c_int), ("serial", ctypes.c_ulong), ("send_event", ctypes.c_int),
                    ("display", ctypes.c_void_p), ("window", ctypes.c_ulong), ("message_type", ctypes.c_ulong),
                    ("format", ctypes.c_int), ("data", Data)]
    class Event(ctypes.Union):
        _fields_ = [("client", ClientMessage), ("pad", ctypes.c_long * 24)]
    x11.XOpenDisplay.restype = ctypes.c_void_p
    display = x11.XOpenDisplay(None)
    if not display:
        raise RuntimeError("X display unavailable")
    x11.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
    x11.XInternAtom.restype = ctypes.c_ulong
    x11.XSendEvent.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_long, ctypes.POINTER(Event)]
    x11.XFlush.argtypes = [ctypes.c_void_p]
    x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
    event = Event()
    event.client.type = 33  # X11 ClientMessage
    event.client.display = display
    event.client.window = int(window)
    event.client.message_type = x11.XInternAtom(display, b"WM_PROTOCOLS", 0)
    event.client.format = 32
    event.client.data.l[0] = x11.XInternAtom(display, b"WM_DELETE_WINDOW", 0)
    x11.XSendEvent(display, int(window), 0, 0, ctypes.byref(event))
    x11.XFlush(display)
    x11.XCloseDisplay(display)
    app.wait(timeout=15)
    assert app.returncode == 0


try:
    initial = start("first")
    record("01-initial", initial)
    command("key", "--clearmodifiers", "ctrl+n")
    end = time.monotonic() + 10
    # egui may move a modal after its initial sizing pass; act on stable observed geometry.
    while True:
        dialogs = entries("DIALOG ")
        if len(dialogs) >= 3 and dialogs[-1] == dialogs[-2]:
            break
        if time.monotonic() > end:
            raise RuntimeError("new document confirmation missing")
        time.sleep(0.03)
    subprocess.run(["import", "-window", str(window), str(output / "new-confirmation.png")], check=True, env=env)
    x, y = entries("DIALOG ")[-1]["proceed"]
    command("mousemove", "--window", window, round(x), round(y))
    command("click", 1)
    state = wait(lambda s: s["current"] and s["snapshot"]["document"] != initial["snapshot"]["document"])
    record("02-new", state)
    click(state, state["cursor_leaf"], 0)
    type_text("English ")
    paste("中文 空格")
    state = wait(lambda s: s["current"] and values(s) == ["English 中文 空格"])
    record("03-chinese-english-space", state)
    type_text("$")
    state = wait(lambda s: s["current"] and len(leaves(s)) == 3)
    type_text("12")
    command("key", "--clearmodifiers", "ctrl+slash")
    state = wait(lambda s: s["current"] and values(s) == ["English 中文 空格", "12", "", ""])
    record("04-empty-denominator", state)
    type_text("3")
    command("key", "--clearmodifiers", "ctrl+slash")
    type_text("4")
    state = wait(lambda s: s["current"] and values(s) == ["English 中文 空格", "12", "3", "4", ""])
    record("05-nested-fraction", state)
    original = state["snapshot"]
    command("key", "--clearmodifiers", "ctrl+z")
    state = wait(lambda s: s["current"] and values(s)[-2] == "")
    record("06-undo-hole", state)
    command("key", "--clearmodifiers", "ctrl+z")
    state = wait(lambda s: s["current"] and values(s) == ["English 中文 空格", "12", "3", ""])
    command("key", "--clearmodifiers", "ctrl+shift+z")
    wait(lambda s: s["current"] and values(s) == ["English 中文 空格", "12", "3", "", ""])
    command("key", "--clearmodifiers", "ctrl+shift+z")
    state = wait(lambda s: s["current"] and s["snapshot"] == original)
    record("07-redo", state)
    denominator = state["cursor_leaf"]
    command("key", "--clearmodifiers", "ctrl+s")
    state = wait(lambda s: s["saved"])
    record("08-saved", state)
    close()
    reopened = start("reopened")
    assert reopened["snapshot"] == original
    assert reopened["wanted"] != state["wanted"]
    record("09-reopened", reopened)
    click(reopened, denominator, 1)
    type_text("5")
    state = wait(lambda s: s["current"] and values(s) == ["English 中文 空格", "12", "3", "45", ""])
    record("10-continued", state)
    command("key", "--clearmodifiers", "Right")
    body = wait(lambda s: s["current"] and s["cursor_leaf"] != denominator)["cursor_leaf"]
    paste("👩🔬")
    wait(lambda s: s["current"] and values(s)[-1] == "👩🔬")
    command("key", "--clearmodifiers", "Home", "Right")
    wait(lambda s: s["cursor_leaf"] == body and s["cursor_byte"] == len("👩".encode()))
    paste("\u200d")
    wait(lambda s: s["current"] and values(s)[-1] == "👩‍🔬")
    type_text("x")
    state = wait(lambda s: s["current"] and values(s)[-1] == "👩‍🔬x")
    record("11-joined-emoji", state)
    paste(" 🇦x🇧")
    wait(lambda s: s["current"] and values(s)[-1] == "👩‍🔬x 🇦x🇧")
    command("key", "--clearmodifiers", "Left", "BackSpace")
    wait(lambda s: s["current"] and values(s)[-1] == "👩‍🔬x 🇦🇧")
    type_text("y")
    state = wait(lambda s: s["current"] and values(s)[-1] == "👩‍🔬x 🇦🇧y")
    record("12-joined-regional-indicators", state)
    unicode_snapshot = state["snapshot"]
    command("key", "--clearmodifiers", "ctrl+s")
    wait(lambda s: s["saved"])
    close()
    state = start("unicode-reopened")
    assert state["snapshot"] == unicode_snapshot
    record("13-unicode-reopened", state)
    original = state["snapshot"]
    tail = state["body_leaves"][-1]
    click(state, tail["leaf"], 0)
    wait(lambda s: s["cursor_leaf"] == tail["leaf"] and s["cursor_byte"] == 0)
    command("key", "--clearmodifiers", "shift+Left")
    state = wait(lambda s: s["current"] and s["selection"] is not None and s["selection_quads"] > 0)
    record("14-whole-formula-selection", state)
    revision = state["revision"]
    type_text("Q")
    state = wait(lambda s: s["current"] and values(s) == ["English 中文 空格Q", "👩‍🔬x 🇦🇧y"])
    assert state["revision"] == revision + 1
    record("15-whole-formula-replaced", state)
    command("key", "--clearmodifiers", "ctrl+z")
    state = wait(lambda s: s["current"] and s["snapshot"] == original and s["selection"] is not None)
    assert state["selection_quads"] > 0
    record("16-undo-restores-selection", state)
    first, last = state["body_leaves"][0], state["body_leaves"][-1]
    state = drag(state, (last["leaf"], last["bytes"]), (first["leaf"], 0))
    record("17-reverse-drag", state)
    revision = state["revision"]
    paste("A👩‍🔬\n\nMiddle\nZ")
    state = wait(lambda s: s["current"] and values(s) == ["A👩‍🔬", "", "Middle", "Z", ""])
    assert state["revision"] == revision + 1
    record("18-multiline-paste", state)
    original = state["snapshot"]
    start_leaf = state["body_leaves"][2]["leaf"]
    end_leaf = state["body_leaves"][3]["leaf"]
    command("key", "--clearmodifiers", "shift+Home", "shift+Left", "shift+Home")
    state = wait(lambda s: s["current"] and s["selection"] is not None
                 and s["selection"]["start_leaf"] == start_leaf and s["selection"]["end_leaf"] == end_leaf
                 and s["selection"]["start_byte"] == 0 and s["selection_quads"] > 0)
    record("19-cross-paragraph-shift-selection", state)
    revision = state["revision"]
    paste("R\nS")
    wait(lambda s: s["current"] and values(s) == ["A👩‍🔬", "", "R", "S", "", ""])
    type_text("x")
    state = wait(lambda s: s["current"] and values(s) == ["A👩‍🔬", "", "R", "Sx", "", ""])
    assert state["revision"] == revision + 2
    record("20-range-replaced-and-continued", state)
    final_snapshot = state["snapshot"]
    command("key", "--clearmodifiers", "ctrl+z", "ctrl+z")
    state = wait(lambda s: s["current"] and s["snapshot"] == original and s["selection"] is not None)
    assert state["selection_quads"] > 0
    record("21-range-undo", state)
    command("key", "--clearmodifiers", "ctrl+shift+z", "ctrl+shift+z")
    state = wait(lambda s: s["current"] and s["snapshot"] == final_snapshot)
    record("22-range-redo", state)
    command("key", "--clearmodifiers", "ctrl+s")
    state = wait(lambda s: s["saved"])
    record("23-range-saved", state)
    close()
    state = start("selection-reopened")
    assert state["snapshot"] == final_snapshot
    record("24-range-reopened", state)
    close()
    all_events = []
    for name in ("first", "reopened", "unicode-reopened", "selection-reopened"):
        log_path = output / f"{name}.log"
        all_events.extend(entries())
    assert any(not event["current"] for event in all_events), "pending frames were observed"
    assert all(event["page_text_draws"] == 0 for event in all_events)
    result = {"binary": str(binary), "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "native_backend": "X11/Xvfb", "audit_frames": len(all_events), "pending_frames": sum(not s["current"] for s in all_events),
              "committed_egui_text_draws": 0, "saved_and_reopened": True, "selection_workflow_stages": 24,
              "selection_frames": sum(s["selection"] is not None for s in all_events)}
    (output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
finally:
    if app and app.poll() is None:
        app.terminate()
        app.wait(timeout=15)
