#!/usr/bin/env python3
"""Native source workbench: explicit unavailable state; optional real TeX E6 pixels.

Discovered by scripts/window-checks.py. SCHOLIUM_LATEX_NATIVE_WINDOW=1 additionally
runs LuaLaTeX and XeLaTeX with configured SCHOLIUM_RTEX_* tools. No desktop access.
"""
import ctypes
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import time

binary = Path(sys.argv[1]).resolve()
out = Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=True)
assert not any(out.iterdir()), "output must be empty"
results = {}
SOURCE = ("\\documentclass{article}\n\\begin{document}\n"
          "First ordinary paragraph contains enough words for the live preview measurement.\n\n"
          "Second paragraph remains unchanged.\n\\end{document}\n")


def record(name, ok, **details):
    results[name] = {"pass": bool(ok), **details}
    (out / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
    print(("PASS " if ok else "FAIL ") + name, flush=True)
    assert ok, (name, details)


class XImage(ctypes.Structure):
    _fields_ = [("width", ctypes.c_int), ("height", ctypes.c_int), ("xoffset", ctypes.c_int),
                ("format", ctypes.c_int), ("data", ctypes.c_void_p), ("byte_order", ctypes.c_int),
                ("bitmap_unit", ctypes.c_int), ("bitmap_bit_order", ctypes.c_int),
                ("bitmap_pad", ctypes.c_int), ("depth", ctypes.c_int),
                ("bytes_per_line", ctypes.c_int), ("bits_per_pixel", ctypes.c_int)]


class Screen:
    """XGetImage reads the actual private Xvfb front buffer, after GPU presentation."""
    def __init__(self, window):
        self.x = ctypes.CDLL("libX11.so.6")
        self.x.XOpenDisplay.argtypes = [ctypes.c_char_p]
        self.x.XOpenDisplay.restype = ctypes.c_void_p
        self.x.XGetImage.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_int,
                                    ctypes.c_uint, ctypes.c_uint, ctypes.c_ulong, ctypes.c_int]
        self.x.XGetImage.restype = ctypes.POINTER(XImage)
        self.x.XDestroyImage.argtypes = [ctypes.POINTER(XImage)]
        self.x.XCloseDisplay.argtypes = [ctypes.c_void_p]
        self.display = self.x.XOpenDisplay(None)
        assert self.display
        self.window = int(window)

    def pixels(self, rect):
        x, y, right, bottom = map(round, rect)
        image = self.x.XGetImage(self.display, self.window, x, y, right-x, bottom-y, 0xffffffff, 2)
        assert image
        data = ctypes.string_at(image.contents.data, image.contents.bytes_per_line * image.contents.height)
        self.x.XDestroyImage(image)
        return hashlib.sha256(data).hexdigest()

    def close(self):
        self.x.XCloseDisplay(self.display)


def run(name, source, native=False):
    root = out / name
    project = root / "project"
    project.mkdir(parents=True)
    (project / "main.tex").write_text(source)
    env = dict(os.environ, SCHOLIUM_LATEX_PROJECT=str(project), SCHOLIUM_SESSION_FILE=str(root / "doc.sqlite"),
               SCHOLIUM_EDITOR_AUDIT="1", WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR="1")
    env.pop("WAYLAND_DISPLAY", None)
    if not native:
        env.pop("SCHOLIUM_RTEX_BIN", None)
        env.pop("SCHOLIUM_RTEX_TEXDIR", None)
    log_path = root / "app.log"

    def state():
        lines = [l for l in log_path.read_text().split("\n")[:-1] if l.startswith("AUDIT ")]
        return json.loads(lines[-1][6:]) if lines else None

    def wait(predicate, what, timeout=80):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            current = state()
            if current and predicate(current):
                return current
            time.sleep(0.01)
        raise AssertionError((what, state()))

    def xdo(*args):
        subprocess.run(["xdotool", *map(str, args)], check=True, env=env)

    with log_path.open("w") as log:
        app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
    screen = None
    try:
        wait(lambda s: s.get("kind") == "latex" and s["source_rect"], "source workbench")
        window = subprocess.check_output(["xdotool", "search", "--sync", "--onlyvisible", "--pid", str(app.pid)], env=env, text=True).split()[0]
        xdo("windowfocus", window)
        if not native:
            s = wait(lambda s: s["toolchain_unavailable"] and s["error"], "explicit missing toolchain")
            record("unavailable-state", s["pages"] == 0 and "SCHOLIUM_RTEX_BIN" in s["error"], error=s["error"])
            # Append a shared math-table completion; clipboard bypass remains literal source.
            xdo("key", "--clearmodifiers", "ctrl+End")
            xdo("type", "--delay", "30", "\\alpha")
            xdo("key", "--clearmodifiers", "Tab")
            s = wait(lambda s: s["text"].endswith("\\alpha"), "LaTeX source completion")
            record("source-completion", s["revision"] > 0 and s["pages"] == 0)
            xdo("key", "--clearmodifiers", "ctrl+z")
            wait(lambda s: s["text"] == source, "completion undo")
            record("source-completion-undo", True)
        else:
            s = wait(lambda s: s["pages"] > 0 and s["shown_revision"] == s["revision"], "native preview")
            expected = "FullCompilation" if name == "xelatex" else "DisplayList"
            record(name + "-preview", expected in s["origin"] and not s["toolchain_unavailable"], origin=s["origin"], engine=s["engine"])
            if name == "live":
                xdo("key", "--clearmodifiers", "ctrl+Home", "Down", "Down", "End")
                screen = Screen(window)
                rect = s["preview_rect"]
                rect = [rect[0]+20, rect[1]+30, rect[2]-25, min(rect[3]-25, 800)]
                samples = []
                clock_delta = time.clock_gettime(time.CLOCK_BOOTTIME) - time.monotonic()
                for i in range(20):
                    old = screen.pixels(rect)
                    revision = state()["revision"]
                    start = time.monotonic()
                    xdo("key", "--clearmodifiers", "x")
                    end = start + 20
                    while time.monotonic() < end and screen.pixels(rect) == old:
                        time.sleep(0.002)
                    elapsed = (time.monotonic() - start) * 1000
                    assert time.monotonic() < end, "no presented pixel change"
                    current = wait(lambda s: s["revision"] > revision and s["shown_revision"] == s["revision"], "revision rendered")
                    samples.append(elapsed)
                    assert current["paragraph"], "E6 must observe live paragraph pixels"
                    time.sleep(0.15)
                suspended = abs(time.clock_gettime(time.CLOCK_BOOTTIME) - time.monotonic() - clock_delta)
                assert suspended <= 1, "discard timing: machine suspended"
                ordered = sorted(samples)
                percentile = lambda q: ordered[math.ceil(q*len(ordered))-1]
                record("e6-key-to-present", True, n=len(samples), p50_ms=percentile(.5), p95_ms=percentile(.95),
                       samples_ms=samples, suspend_seconds=suspended, method="xdotool key dispatch to first XGetImage preview pixel change; 2ms polling; private Xvfb, software GL")
        time.sleep(.15)
        subprocess.run(["import", "-window", window, str(root / "window.png")], env=env, check=True)
    finally:
        if screen:
            screen.close()
        if app.poll() is None:
            app.terminate()
            try:
                app.wait(timeout=15)
            except subprocess.TimeoutExpired:
                app.kill()
                app.wait()


run("unavailable", SOURCE)
if os.environ.get("SCHOLIUM_LATEX_NATIVE_WINDOW") == "1":
    run("live", SOURCE, True)
    run("xelatex", "% !TeX program = xelatex\n" + SOURCE, True)
else:
    results["native-preview"] = {"status": "not-run", "reason": "optional local toolchain acceptance; CI asserts unavailable state"}
    (out / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
