#!/usr/bin/env python3
"""Exercise display-formula insertion and block-boundary keys in the real window."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

binary, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
if any(output.iterdir()):
    raise SystemExit("output directory must be empty")
env = dict(os.environ, SCHOLIUM_SESSION_FILE=str(output / "doc.sqlite"),
           SCHOLIUM_EDITOR_AUDIT="1", WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR="1")
env.pop("WAYLAND_DISPLAY", None)
log_path = output / "app.log"


def state():
    lines = [line for line in log_path.read_text().splitlines() if line.startswith("AUDIT ")]
    return json.loads(lines[-1][6:]) if lines else None


def wait(predicate, description):
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        current = state()
        if current and predicate(current):
            return current
        time.sleep(0.05)
    raise RuntimeError(f"timeout: {description}")


def blocks(snapshot):
    return snapshot["snapshot"]["blocks"]


with log_path.open("w") as log:
    app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
try:
    wait(lambda current: current["current"], "initial scene")
    window = subprocess.check_output(["xdotool", "search", "--pid", str(app.pid)], env=env, text=True).split()[0]
    run = lambda *args: subprocess.run(["xdotool", *args], env=env, check=True)
    run("windowfocus", window)
    run("type", "--delay", "20", "alpha")
    wait(lambda current: current["current"] and "alpha" in json.dumps(blocks(current)), "body text")
    run("key", "ctrl+shift+e")
    equation = wait(lambda current: current["current"] and any(block["kind"] == "Equation" for block in blocks(current)),
                    "display equation command")
    assert len(blocks(equation)) == 2, blocks(equation)
    equation_index = next(i for i, block in enumerate(blocks(equation)) if block["kind"] == "Equation")
    math = blocks(equation)[equation_index]["content"]
    assert len(math) == 1 and math[0]["body"]["kind"] == "math", math
    assert equation["cursor_leaf"] == f'NodeId({math[0]["body"]["root"]["node"]})'
    time.sleep(0.15)
    subprocess.run(["import", "-window", window, str(output / "display-equation.png")], env=env, check=True)

    run("key", "Return")
    below = wait(lambda current: current["current"] and len(blocks(current)) == 3, "Enter creates paragraph below")
    assert blocks(below)[equation_index + 1]["kind"] == "Paragraph"
    run("key", "BackSpace")
    returned = wait(lambda current: current["current"] and current["cursor_leaf"] == f'NodeId({math[0]["body"]["root"]["node"]})',
                    "Backspace from following paragraph returns to equation")
    assert len(blocks(returned)) == 3
    print("PASS display equation command, geometry, Enter and Backspace boundaries", flush=True)
finally:
    app.terminate()
    try:
        app.wait(timeout=10)
    except subprocess.TimeoutExpired:
        app.kill()
        app.wait()


import importlib.util
spec = importlib.util.spec_from_file_location("parity", Path(__file__).with_name("parity-check.py"))
parity = importlib.util.module_from_spec(spec)
spec.loader.exec_module(parity)
parity.scenario(binary, Path(__file__).with_name("scenarios") / "display-formula.json", output / "menu-parity")
