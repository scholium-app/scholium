#!/usr/bin/env python3
"""Replay shared scenarios in a real X11 window and compare every frozen expected field.

Run through scripts/window-checks.py (private Xvfb and machine-wide lock).
--scenarios accepts an isolated fixture directory for the deliberate negative control.
No expected values are generated or updated by this checker.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

from chrome_ui import keycode

ROOT = Path(__file__).resolve().parent
TIMEOUT = 20
KEYS = {"ArrowLeft": "Left", "ArrowRight": "Right", "ArrowUp": "Up", "ArrowDown": "Down",
        "Enter": "Return", "Escape": "Escape", "Delete": "Delete", "Backspace": "BackSpace",
        "Home": "Home", "End": "End", "Tab": "Tab", "Slash": "slash"}


def identity(value):
    return value.removeprefix("NodeId(").removesuffix(")")


def projection(state):
    # Both runners call EditorCore::semantic_projection: there is one summary algorithm.
    value = state["semantic"]
    if set(value) != {"snapshot", "cursor", "anchor", "undo_depth"}:
        raise ValueError(f"invalid semantic projection: {value}")
    return value


def differences(expected, actual):
    result = {key: {"expected": value, "actual": actual.get(key)}
              for key, value in expected.items() if key not in actual or value != actual[key]}
    missing = sorted(set(actual) - set(expected))
    if missing:
        result["missing_expected_fields"] = missing
    return result


class Replay:
    def __init__(self, binary, output):
        self.output = output
        self.log = output / "app.log"
        self.env = dict(os.environ, SCHOLIUM_SESSION_FILE=str(output / "doc.sqlite"),
                        SCHOLIUM_EDITOR_AUDIT="1", WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR="1")
        self.env.pop("WAYLAND_DISPLAY", None)
        self.writer = self.log.open("w")
        self.proc = subprocess.Popen([str(binary)], env=self.env, stdout=self.writer, stderr=subprocess.STDOUT)
        self.window = None

    def close(self):
        self.proc.terminate()
        try:
            self.proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait(timeout=5)
        self.writer.close()

    def xdo(self, *args):
        return subprocess.run(["xdotool", *map(str, args)], env=self.env, check=True,
                              capture_output=True, text=True, timeout=TIMEOUT).stdout.strip()

    def state(self):
        if self.proc.poll() is not None:
            raise RuntimeError(f"app exited: {self.proc.returncode}; see {self.log}")
        lines = [line[6:] for line in self.log.read_text().split("\n")[:-1] if line.startswith("AUDIT ")]
        return json.loads(lines[-1]) if lines else None

    def ready(self):
        deadline = time.monotonic() + TIMEOUT
        previous = None
        while time.monotonic() < deadline:
            state = self.state()
            if state and state.get("layout_error"):
                raise RuntimeError(f"layout failed: {state['layout_error']}")
            if state and state["current"] and state["deferred"] == 0:
                if state == previous:
                    return state
                previous = state
            time.sleep(0.1)
        raise RuntimeError(f"timed out waiting for current AUDIT; last={state}")

    def focus(self):
        deadline = time.monotonic() + TIMEOUT
        while time.monotonic() < deadline:
            found = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(self.proc.pid)],
                                   env=self.env, capture_output=True, text=True, timeout=TIMEOUT)
            if found.returncode == 0 and found.stdout.split():
                self.window = found.stdout.split()[0]
                self.xdo("windowfocus", "--sync", self.window)
                return self.ready()
            self.state()
            time.sleep(0.1)
        raise RuntimeError(f"window not found for pid {self.proc.pid}")

    def key(self, event):
        name = event["key"]
        symbol = event["value"] if name == "Character" else KEYS.get(name, name)
        # Numeric keycodes prevent xdotool from adding Alt to function-key chords.
        if re.fullmatch(r"F\d+", symbol):
            symbol = str(keycode(self.env["DISPLAY"], symbol))
        mods = event.get("mods", {})
        if name == "Character" and not any(mods.values()):
            self.xdo("type", "--clearmodifiers", "--delay", 1, "--", symbol)
            return
        chord = "+".join([v for k, v in (("command", "ctrl"), ("shift", "shift"), ("alt", "alt"))
                          if mods.get(k)] + [symbol])
        self.xdo("key", "--clearmodifiers", chord)

    def pointer(self, event, state):
        target = event["target"]
        leaves = state["body_leaves"]
        if target.startswith("end of leaf "):
            leaf = leaves[int(target.removeprefix("end of leaf "))]
            node, byte = leaf["leaf"], leaf["bytes"]
        elif target.startswith("word "):
            leaf = next(block["content"][0] for block in state["snapshot"]["blocks"]
                        if block["content"][0]["body"]["kind"] == "text")
            text = leaf["body"]["text"]
            word = list(re.finditer(r"\S+", text))[int(target.removeprefix("word "))]
            node, byte = leaf["node"], len(text[:word.start()].encode()) + 1
        else:
            raise ValueError(f"unknown pointer target: {target}")
        caret = next(c for c in state["carets"] if c["exact"] and identity(c["leaf"]) == identity(node)
                     and c["byte"] == byte)
        # AUDIT carets already include page placement and pt->window-pixel conversion.
        self.xdo("mousemove", "--window", self.window, round(caret["x"]), round(caret["y"]))
        clicks, kind = event.get("clicks", 1), event["kind"]
        if clicks > 1:
            completed = clicks - 1 if kind == "Press" else clicks
            self.xdo("click", "--repeat", completed, "--delay", 70, 1)
            if kind == "Press":
                self.xdo("mousedown", 1)
        elif kind == "Press":
            self.xdo("mousedown", 1)
        elif kind == "Release":
            self.xdo("mouseup", 1)
        elif kind != "Move":
            raise ValueError(f"unknown pointer kind: {kind}")

    def input(self, event, state):
        if "text" in event:
            text = event["text"]
            # One scenario Text is one commit; typing words separately creates undo groups.
            if text.isascii() and not any(c.isspace() for c in text):
                self.xdo("type", "--clearmodifiers", "--delay", 1, "--", text)
            else:
                subprocess.run(["xclip", "-selection", "clipboard"], input=text.encode(), env=self.env,
                               check=True, timeout=TIMEOUT)
                self.xdo("key", "--clearmodifiers", "ctrl+v")
        elif "key" in event:
            self.key(event)
        elif "pointer" in event:
            self.pointer(event["pointer"], state)
        elif event.get("command") == "Bold":
            self.key({"key": "Character", "value": "b", "mods": {"command": True}})
        else:
            raise ValueError(f"unknown input: {event}")
        time.sleep(0.15)
        return self.ready()


def cursor_caret(state):
    candidates = [c for c in state["carets"] if identity(c["leaf"]) == identity(state["cursor_leaf"])
                  and c["byte"] == state["cursor_byte"]]
    return next((c for c in candidates if c["affinity"] == state["cursor_affinity"]), next(iter(candidates), None))


def check_step(event, state, states):
    actual = projection(state)
    expected = event.get("expected", {})
    diff = {k: {"expected": v, "actual": actual.get(k)} for k, v in expected.items() if actual.get(k) != v}
    if diff:
        raise AssertionError(json.dumps(diff, ensure_ascii=False, indent=2))
    if "vertical" not in event:
        return
    check = event["vertical"]
    goal = cursor_caret(states[check["goal_step"] - 1])["x"]
    previous = cursor_caret(states[check["previous_step"] - 1])
    caret = cursor_caret(state)
    dy = caret["y"] - previous["y"]
    if not (dy > 0 if check["direction"] == "down" else dy < 0):
        raise AssertionError(f"must cross a soft wrap: previous={previous}, actual={caret}")
    stops = [c for c in state["carets"] if c["exact"] and c["leaf"] == caret["leaf"]]
    if any(min(caret["y"], previous["y"]) < c["y"] < max(caret["y"], previous["y"]) for c in stops):
        raise AssertionError("must move to the adjacent visual line")
    nearest = min(abs(c["x"] - goal) for c in stops if abs(c["y"] - caret["y"]) < 1e-4)
    if abs(abs(caret["x"] - goal) - nearest) >= 1e-4:
        raise AssertionError(f"lost original column goal {goal}: caret={caret}, nearest distance={nearest}")
    if check.get("distinguish_previous_x"):
        prior_nearest = min(abs(c["x"] - previous["x"]) for c in stops if abs(c["y"] - caret["y"]) < 1e-4)
        if abs(caret["x"] - previous["x"]) <= prior_nearest + 1e-4:
            raise AssertionError("fixture must distinguish original goal from previous line's caret x")


def scenario(binary, path, output):
    value = json.loads(path.read_text())
    output.mkdir(parents=True, exist_ok=False)
    replay = Replay(binary, output)
    step = "startup"
    rows = []
    states = []
    try:
        state = replay.focus()
        for index, event in enumerate(value["inputs"]):
            step = f"input {index + 1}: {event}"
            state = replay.input(event, state)
            rows.append({"step": step, "actual": projection(state), "caret": cursor_caret(state)})
            (output / "steps.json").write_text(json.dumps(rows, ensure_ascii=False, indent=2) + "\n")
            check_step(event, state, states)
            states.append(state)
        actual = projection(replay.ready())
        diff = differences(value["expected"], actual)
        (output / "result.json").write_text(json.dumps({"expected": value["expected"], "actual": actual,
                                                       "diff": diff}, ensure_ascii=False, indent=2) + "\n")
        if diff:
            raise AssertionError(json.dumps(diff, ensure_ascii=False, indent=2))
        print(f"PASS {value['name']}", flush=True)
    except Exception as error:
        print(f"FAIL {value['name']} at {step}: {error}", file=sys.stderr, flush=True)
        raise
    finally:
        replay.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--scenarios", type=Path, default=ROOT / "scenarios")
    args = parser.parse_args()
    display = os.environ.get("DISPLAY", "")
    if not display or re.fullmatch(r"(?:[^:]*):0(?:\.\d+)?", display):
        parser.error("a private Xvfb DISPLAY is required")
    files = sorted(args.scenarios.glob("*.json"))
    if not files:
        parser.error("no scenario JSON files found")
    for path in files:
        scenario(args.binary.resolve(), path, args.output.resolve() / path.stem)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:
        print(f"parity-check failed: {error}", file=sys.stderr)
        sys.exit(1)
