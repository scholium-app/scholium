"""Real app harness: ordinary project open, native keys/clipboard, audit observations."""
import json
import os
from pathlib import Path
import signal
import subprocess
import time

from latex_exit_screen import Screen


class Window:
    def __init__(self, binary, root, source):
        self.binary, self.root = binary, root
        self.project = root / "project"
        self.project.mkdir(parents=True)
        self.path = self.project / "paper.tex"
        self.path.write_text(source)
        self.env = dict(os.environ, SCHOLIUM_PROJECT_PATH=str(self.path),
                        SCHOLIUM_RECENT_FILE=str(root / "recent.json"),
                        SCHOLIUM_EDITOR_AUDIT="1", WINIT_UNIX_BACKEND="x11",
                        WINIT_X11_SCALE_FACTOR="1", SCHOLIUM_LATEX_FRAME_AUDIT="1")
        for key in ("WAYLAND_DISPLAY", "SCHOLIUM_RTEX_BIN", "SCHOLIUM_RTEX_TEXDIR"):
            self.env.pop(key, None)
        self.serial = 0
        self.screen = None
        self.frames = []
        self.engine_timings = []
        self.controls = {}
        self.start()

    def start(self):
        self.serial += 1
        self.log = self.root / f"app-{self.serial}.log"
        self.offset, self.partial, self.events, self.current = 0, "", [], None
        with self.log.open("w") as output:
            self.app = subprocess.Popen([str(self.binary)], env=self.env, stdout=output,
                                        stderr=subprocess.STDOUT)
        self.wait(lambda s: s.get("source_rect"), "workbench")
        self.window = subprocess.check_output(
            ["xdotool", "search", "--sync", "--onlyvisible", "--pid", str(self.app.pid)],
            env=self.env, text=True).split()[0]
        self.xdo("windowfocus", self.window)
        self.screen = Screen(self.window)

    def state(self):
        with self.log.open() as stream:
            stream.seek(self.offset)
            data = stream.read()
            self.offset = stream.tell()
        lines = (self.partial + data).split("\n")
        self.partial = lines.pop()
        for line in lines:
            if line.startswith("PROJECT_UI "):
                value = json.loads(line[11:])
                self.controls[value["control"]] = value["center"]
            if line.startswith("AUDIT "):
                value = json.loads(line[6:])
                if value.get("kind") == "latex-frame":
                    self.frames.append(value)
                if value.get("kind") == "latex-engine-timing":
                    self.engine_timings.append(value)
                if value.get("kind") == "latex":
                    self.current = value
                    self.events.append(value)
        return self.current

    def wait(self, predicate, what, timeout=100):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            value = self.state()
            if value and predicate(value):
                return value
            if self.app.poll() is not None:
                raise AssertionError((what, "app exited", self.app.returncode))
            time.sleep(.01)
        value = dict(self.current or {})
        value.pop("text", None)
        raise AssertionError((what, value, str(self.log)))

    def ready(self, revision=None, full=False, timeout=100):
        return self.wait(lambda s: s["pages"] > 0 and s["shown_revision"] == s["revision"]
                         and (revision is None or s["revision"] >= revision)
                         and (not full or not s["paragraph"]), "current preview", timeout)

    def xdo(self, *args):
        subprocess.run(["xdotool", *map(str, args)], env=self.env, check=True)

    def key(self, *keys):
        self.xdo("key", "--clearmodifiers", *keys)

    def replace(self, text):
        # Paste uses the real editor event path and bypasses command completion, like user paste.
        subprocess.run(["xclip", "-selection", "clipboard"], input=text.encode(),
                       env=self.env, check=True)
        old = self.state()["revision"]
        self.key("ctrl+a", "ctrl+v")
        return self.wait(lambda s: s["revision"] > old and s["text"] == text,
                         "source paste")["revision"]

    def click(self, control):
        self.state()
        x, y = self.controls[control]
        self.xdo("mousemove", round(x), round(y), "click", "1")

    def body(self):
        self.key("ctrl+Home")
        self.xdo("key", "--clearmodifiers", "--repeat", "6", "Down")
        self.key("End")

    def pixels(self):
        s = self.state()
        r = s["page_rect"]
        assert r, "visible page"
        pane = s["preview_rect"]
        # Intersect the visible sheet, excluding page controls and source/caret pixels.
        return self.screen.pixels([max(r[0]+2, pane[0]+2), max(r[1]+2, pane[1]+35),
                                   min(r[2]-2, pane[2]-18), min(r[3]-2, pane[3]-18)])

    def settled_pixels(self, unlike=None, timeout=5):
        """Front-buffer hash once it is stable for 200 ms.

        With `unlike`, keep waiting for a stable frame that differs from it: the audit can report
        a revision before the frame showing its texture is presented. If none arrives before the
        timeout the stable frame is returned and the caller's change assertion fails as before.
        """
        previous, since = None, time.monotonic()
        deadline = since + timeout
        stable = None
        while time.monotonic() < deadline:
            current = self.pixels()
            if current != previous:
                previous, since = current, time.monotonic()
            elif time.monotonic() - since >= .2:
                if unlike is None or current != unlike:
                    return current
                stable = current
            time.sleep(.02)
        if stable is not None:
            return stable
        raise AssertionError("front buffer did not settle")

    def snapshot(self, name):
        subprocess.run(["import", "-window", self.window, str(self.root / name)],
                       env=self.env, check=True)

    def kill(self):
        if self.screen:
            self.screen.close()
            self.screen = None
        if self.app.poll() is None:
            self.app.send_signal(signal.SIGKILL)
            self.app.wait(timeout=15)

    def close(self):
        if self.screen:
            self.screen.close()
            self.screen = None
        if self.app.poll() is None:
            self.app.terminate()
            try:
                self.app.wait(timeout=15)
            except subprocess.TimeoutExpired:
                self.app.kill()
                self.app.wait()

    def descendants(self):
        table = subprocess.check_output(["ps", "-eo", "pid=,ppid=,comm="], text=True)
        rows = [line.split(None, 2) for line in table.splitlines()]
        children, frontier = [], {self.app.pid}
        while frontier:
            found = [(int(pid), name) for pid, parent, name in rows if int(parent) in frontier]
            children += found
            frontier = {pid for pid, _ in found}
        return children
