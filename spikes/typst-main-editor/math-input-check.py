#!/usr/bin/env python3
"""Real-window shared math input, command overlay and Tab-context acceptance."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

from check_evidence import evidence_enabled

ROOT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("parity", ROOT / "parity-check.py")
parity = importlib.util.module_from_spec(spec)
spec.loader.exec_module(parity)


def run_case(binary, output, name, check):
    out = output / name
    out.mkdir(parents=True, exist_ok=False)
    pixels = out / "pixels"
    pixels.mkdir()
    replay = parity.Replay(binary, out, {
        "SCHOLIUM_EDITOR_RASTER_AUDIT": str(pixels), "SCHOLIUM_RASTER_PX_PER_PT": "2",
    })
    rows = []
    try:
        replay.focus()
        def act(*args):
            replay.xdo(*args)
            state = replay.ready()
            assert state["page_text_draws"] == 0, "document text must only be drawn by Typst"
            assert state["layout_error"] is None, state["layout_error"]
            rows.append(state)
            (out / "audit.json").write_text(json.dumps(rows, ensure_ascii=False, indent=2) + "\n")
            return state
        check(replay, act)
        if evidence_enabled():
            time.sleep(0.15)
            subprocess.run(["import", "-window", replay.window, str(out / "window.png")],
                           env=replay.env, check=True, timeout=20)
    finally:
        replay.close()
    print(f"PASS math input {name}", flush=True)


def formula(state):
    content = state["semantic"]["snapshot"]["blocks"][0]["content"]
    return next(i["body"]["root"]["body"] for i in content if i["body"]["kind"] == "math")


def structures(replay, act):
    act("key", "--clearmodifiers", "alt+m")
    state = act("key", "--clearmodifiers", "alt+f")
    assert formula(state)["kind"] == "fraction"
    act("type", "--clearmodifiers", "1")
    state = act("key", "Tab")
    assert state["semantic"]["cursor"]["path"].endswith("/denominator")
    act("type", "--clearmodifiers", "2")
    state = act("key", "Tab")
    assert state["semantic"]["cursor"]["path"].endswith("/root")
    assert state["cursor_byte"] == 1
    assert state["math_input"]["tab"] == "NextSlot"


def composition(replay, act):
    state = act("type", "--clearmodifiers", "--delay", "40", "$RR")
    assert formula(state)["text"] == "ℝ"
    assert state["math_input"]["tab"] == "Cycle"
    state = act("key", "Tab")
    assert formula(state)["text"] == "ℛ"
    state = act("key", "shift+Tab")
    assert formula(state)["text"] == "ℝ"
    act("key", "Left")
    state = act("key", "Right")
    assert state["math_input"]["tab"] == "NextSlot"
    state = act("type", "--clearmodifiers", "--delay", "40", "<=")
    assert formula(state)["text"] == "ℝ⩽"
    state = act("key", "Tab")
    assert formula(state)["text"] == "ℝ≤"


def commands(replay, act):
    state = act("type", "--clearmodifiers", "before")
    before, revision = state["semantic"]["snapshot"], state["revision"]
    state = act("type", "--clearmodifiers", "--delay", "40", "\\sqrt")
    assert state["math_input"]["query"] == "sqrt"
    assert state["math_input"]["tab"] == "Complete"
    assert state["semantic"]["snapshot"] == before and state["revision"] == revision
    if evidence_enabled():
        time.sleep(0.15)
        subprocess.run(["import", "-window", replay.window, str(replay.output / "command-overlay.png")],
                       env=replay.env, check=True, timeout=20)
    state = act("key", "Tab")
    assert formula(state)["kind"] == "root"
    assert state["semantic"]["cursor"]["path"].endswith("/radicand")
    act("type", "--clearmodifiers", "--delay", "40", "\\alpha")
    state = act("key", "Return")
    assert formula(state)["radicand"]["body"]["text"] == "α"
    before, revision = state["semantic"]["snapshot"], state["revision"]
    act("type", "--clearmodifiers", "--delay", "40", "\\frac")
    state = act("key", "Escape")
    assert state["math_input"]["query"] is None
    assert state["semantic"]["snapshot"] == before and state["revision"] == revision


def brackets(replay, act):
    state = act("type", "--clearmodifiers", "--delay", "40", "$([{|x")
    node = formula(state)
    for a, b in [("(", ")"), ("[", "]"), ("{", "}"), ("|", "|")]:
        assert node["kind"] == "delimited" and node["open"] == a and node["close"] == b, node
        node = node["body"]["body"]
    assert node == {"kind": "text", "text": "x"}, node


def stock_pixels(replay, state, name):
    expected = json.loads((ROOT / "reference/delimiter-pixels.json").read_text())[name]
    path = replay.output / "pixels" / str(state["revision"])
    deadline = time.monotonic() + parity.TIMEOUT
    while time.monotonic() < deadline:
        if path.with_suffix(".json").exists():
            meta = json.loads(path.with_suffix(".json").read_text())
            if meta["px_per_pt"] == 2:
                pixels = path.with_suffix(".rgba").read_bytes()
                actual = {"width": meta["width"], "height": meta["height"],
                          "sha256": hashlib.sha256(pixels).hexdigest()}
                assert actual == expected, (name, actual, expected)
                return
        time.sleep(0.05)
    raise AssertionError(f"no uploaded 2 px/pt raster for revision {state['revision']}")


def delimiter_shortcut(key, open_char, close_char, name):
    def check(replay, act):
        act("type", "--clearmodifiers", "root ")
        act("type", "--clearmodifiers", "--delay", "40", "$|")
        state = act("type", "--clearmodifiers", key)
        assert formula(state) == {"kind": "delimited", "open": open_char, "close": close_char,
                                  "body": {"body": {"kind": "hole"}}}
        assert state["semantic"]["cursor"]["path"].endswith("/root/body/body")
        assert state["cursor_byte"] == 0
        state = act("type", "--clearmodifiers", "x")
        assert formula(state)["body"]["body"] == {"kind": "text", "text": "x"}
        assert state["cursor_byte"] == 1
        assert parity.cursor_caret(state)["exact"]
        stock_pixels(replay, state, name)
    return check


def half_open(replay, act):
    act("type", "--clearmodifiers", "root ")
    state = act("type", "--clearmodifiers", "--delay", "40", "$[0, 1")
    assert formula(state)["body"]["body"] == {"kind": "text", "text": "0, 1"}
    state = act("key", "Tab")
    assert state["semantic"]["cursor"]["path"].endswith("/root") and state["cursor_byte"] == 1
    state = act("key", "BackSpace")
    assert formula(state)["open"] == "[" and formula(state)["close"] == "\0"
    assert state["semantic"]["cursor"]["path"].endswith("/root/body/body") and state["cursor_byte"] == 4
    assert parity.cursor_caret(state)["exact"]
    stock_pixels(replay, state, "delim-open-interval")
    state = act("type", "--clearmodifiers", ")")
    assert formula(state) == {"kind": "delimited", "open": "[", "close": ")",
                              "body": {"body": {"kind": "text", "text": "0, 1"}}}
    assert state["semantic"]["cursor"]["path"].endswith("/root") and state["cursor_byte"] == 1
    assert parity.cursor_caret(state)["exact"]
    stock_pixels(replay, state, "delim-half-open")


def main():
    display = os.environ.get("DISPLAY", "")
    assert display and not re.fullmatch(r"(?:[^:]*):0(?:\.\d+)?", display), "private Xvfb required"
    binary, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
    for name, check in [("structures", structures), ("composition", composition),
                        ("commands", commands), ("brackets", brackets),
                        ("norm", delimiter_shortcut("|", "‖", "‖", "delim-input-norm")),
                        ("floor", delimiter_shortcut(".", "⌊", "⌋", "delim-input-floor")),
                        ("ceil", delimiter_shortcut("'", "⌈", "⌉", "delim-input-ceil")),
                        ("half-open", half_open)]:
        run_case(binary, output, name, check)


if __name__ == "__main__":
    main()
