#!/usr/bin/env python3
"""Compare a source pane after a theme switch with a fresh light-theme source pane."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parent))
import chrome_ui  # noqa: E402

binary = Path(sys.argv[1]).resolve()
output = Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
if any(output.iterdir()):
    raise SystemExit("output directory must be empty")

env_base = dict(os.environ, SCHOLIUM_EDITOR_AUDIT="1", WINIT_UNIX_BACKEND="x11",
                WINIT_X11_SCALE_FACTOR="1")
env_base.pop("WAYLAND_DISPLAY", None)


def run_case(folder, switch):
    folder.mkdir()
    env = dict(env_base, SCHOLIUM_SESSION_FILE=str(folder / "doc.sqlite"))
    log_path = folder / "app.log"

    def state():
        lines = [line for line in log_path.read_text().split("\n")[:-1] if line.startswith("AUDIT ")]
        return json.loads(lines[-1][6:]) if lines else None

    def wait(predicate, what):
        end = time.monotonic() + 60
        while time.monotonic() < end:
            current = state()
            if current and predicate(current):
                return current
            time.sleep(0.05)
        raise RuntimeError(f"timeout: {what}")

    def xdo(*args):
        subprocess.run(["xdotool", *map(str, args)], check=True, env=env)

    def paste(value):
        subprocess.run(["xclip", "-selection", "clipboard"], input=value.encode(),
                       check=True, env=env)
        xdo("key", "--clearmodifiers", "ctrl+v")

    with log_path.open("w") as log:
        app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
    try:
        wait(lambda s: s["current"], "first scene")
        window = subprocess.check_output(["xdotool", "search", "--pid", str(app.pid)],
                                         env=env, text=True).split()[0]
        xdo("windowfocus", window)
        paste("主题切换 source text\n第二行 readable glyphs")
        wait(lambda s: [b["body"]["text"] for block in s["snapshot"]["blocks"]
                        for b in block["content"] if b["body"]["kind"] == "text"]
             == ["主题切换 source text", "第二行 readable glyphs"], "pasted document")
        chrome = chrome_ui.Chrome(state, xdo, window)
        if switch:
            chrome.menu("ui.menu.view", "ui.menu.view.theme", "view.theme.dark")
            wait(lambda s: s["chrome"]["facts"]["theme"] == "Dark", "dark theme")
        chrome.click("view.mode.source", "focus")
        wait(lambda s: s["source_mode"] and s["preview"]["shown"], "source view")
        if switch:
            chrome.menu("ui.menu.view", "ui.menu.view.theme", "view.theme.light")
            wait(lambda s: s["chrome"]["facts"]["theme"] == "Light", "light theme")
        xdo("mousemove", "--window", window, 1200, 800)
        time.sleep(0.3)
        screenshot = folder / "window.png"
        subprocess.run(["import", "-window", window, str(screenshot)], check=True, env=env)
        return screenshot
    finally:
        if app.poll() is None:
            app.terminate()
            try:
                app.wait(timeout=15)
            except subprocess.TimeoutExpired:
                app.kill()
                app.wait(timeout=15)


switched = run_case(output / "switched", True)
fresh = run_case(output / "fresh", False)
# Window logical and physical pixels coincide at the pinned X11 scale. Compare only
# source text below its header, away from preview timings, chrome and scrollbars.
def source_pixels(path):
    # ImageMagick is already required for the window screenshots; decode RGB8 here
    # so the comparison has no extra Python image-library dependency.
    pixels = subprocess.check_output(
        ["convert", str(path), "-crop", "612x670+8+150", "+repage", "-depth", "8", "rgb:-"],
        stderr=subprocess.DEVNULL,
    )
    assert len(pixels) == 612 * 670 * 3, "source pane capture has the wrong size"
    return pixels


delta = bytes(abs(a - b) for a, b in zip(source_pixels(switched), source_pixels(fresh)))
mean = sum(delta) / len(delta)
changed = sum(max(delta[i:i + 3]) > 8 for i in range(0, len(delta), 3)) / (len(delta) / 3)
result = {"mean_delta": mean, "changed_fraction": changed,
          "region": [8, 150, 620, 820], "pass": mean <= 0.1 and changed <= 0.001}
(output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
print(result, flush=True)
assert result["pass"], "theme-switched source pane differs from fresh light source pane"
print("PASS source-theme-repaint")
