"""Observed project controls for real-window checks; all coordinates come from UI audits."""
import json
import os
from pathlib import Path
import subprocess
import time


class ProjectApp:
    def __init__(self, binary, output, project):
        self.binary, self.output = str(binary), Path(output)
        self.env = dict(os.environ, SCHOLIUM_PROJECT_PATH=str(project), SCHOLIUM_EDITOR_AUDIT="1",
                        SCHOLIUM_RECENT_FILE=str(self.output / "recent-projects.json"),
                        WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR="1")
        self.env.pop("WAYLAND_DISPLAY", None)
        self.app = None
        self.window = None
        self.log = None

    def entries(self, prefix):
        return [json.loads(l[len(prefix):]) for l in self.log.read_text().split("\n")[:-1]
                if l.startswith(prefix)]

    def wait(self, predicate, timeout=60):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            assert self.app.poll() is None, self.log.read_text()
            found = predicate()
            if found:
                return found
            time.sleep(0.04)
        raise AssertionError("project UI timeout: " + self.log.read_text()[-4000:])

    def state(self, predicate=lambda s: s["current"]):
        return self.wait(lambda: next((s for s in self.entries("AUDIT ")[-1:] if predicate(s)), None))

    def source(self, predicate=lambda s: True):
        return self.wait(lambda: next((s for s in self.entries("PROJECT ")[-1:] if predicate(s)), None))

    def xdo(self, *args):
        subprocess.run(["xdotool", *map(str, args)], env=self.env, check=True)

    def start(self, name):
        self.log = self.output / f"{name}.log"
        with self.log.open("w") as log:
            self.app = subprocess.Popen([self.binary], env=self.env, stdout=log, stderr=subprocess.STDOUT)
        self.wait(lambda: subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(self.app.pid)],
                                        env=self.env, capture_output=True).returncode == 0)
        self.window = subprocess.check_output(["xdotool", "search", "--onlyvisible", "--pid", str(self.app.pid)],
                                              env=self.env, text=True).splitlines()[0]
        self.xdo("windowfocus", self.window)

    def control(self, name):
        self.wait(lambda: any(s["control"] == name for s in self.entries("PROJECT_UI ")))
        time.sleep(0.25)  # egui's modal has a sizing pass before stable placement.
        last = next(s for s in reversed(self.entries("PROJECT_UI ")) if s["control"] == name)
        x, y = last["center"]
        self.xdo("mousemove", "--window", self.window, round(x), round(y))
        self.xdo("click", 1)

    def type(self, text):
        self.xdo("type", "--clearmodifiers", "--delay", "8", text)

    def key(self, chord):
        self.xdo("key", "--clearmodifiers", chord)

    def create(self, path, language="typst"):
        self.key("ctrl+n")
        self.control("path")
        self.type(str(path))
        self.control(language)
        self.control("submit")

    def capture(self, name):
        time.sleep(0.15)
        subprocess.run(["import", "-window", self.window, str(self.output / f"{name}.png")], env=self.env, check=True)

    def kill(self):
        if self.app and self.app.poll() is None:
            self.app.kill()
            self.app.wait(timeout=15)
