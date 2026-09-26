#!/usr/bin/env python3
"""Native acceptance probes, intended to run in a dedicated xvfb-run server.

Uses only fresh temporary session files inside OUT; never kills another app.
Writes screenshots and authoritative SQLite snapshots, continues after failures.
"""
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


class Window:
    def __init__(self, directory):
        self.directory = directory
        self.directory.mkdir(parents=True, exist_ok=True)
        self.database = directory / "session.sqlite"
        self.env = os.environ.copy()
        self.env.pop("WAYLAND_DISPLAY", None)
        self.env.update(WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR="1",
                        SCHOLIUM_SESSION_FILE=str(self.database))
        self.log = (directory / "app.log").open("a")
        self.app = subprocess.Popen([str(ROOT / "target/debug/scholium-app")], env=self.env,
                                    stdout=self.log, stderr=subprocess.STDOUT)
        try:
            self.window = self.wait_window()
            self.x("windowsize", self.window, 1200, 900)
            self.x("windowfocus", "--sync", self.window)
            time.sleep(0.5)
        except BaseException:
            self.close()
            raise

    def wait_window(self):
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            if self.app.poll() is not None:
                raise RuntimeError("application exited during startup")
            result = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(self.app.pid)],
                                    env=self.env, capture_output=True, text=True, timeout=5)
            if result.returncode == 0:
                return result.stdout.splitlines()[-1]
            time.sleep(0.1)
        raise TimeoutError("application window not found")

    def x(self, *args):
        return subprocess.check_output(["xdotool", *map(str, args)], env=self.env,
                                       text=True, timeout=10).strip()

    def key(self, value):
        self.x("key", "--clearmodifiers", value)
        time.sleep(0.15)

    def type(self, value):
        self.x("type", "--clearmodifiers", "--delay", "35", value)
        time.sleep(0.3)

    def seed(self, markup, ribbon=False):
        self.key("ctrl+n")
        time.sleep(0.5)
        subprocess.run(["xclip", "-selection", "clipboard"], env=self.env, input=markup.encode(),
                       check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5)
        if ribbon:
            # Home > Paste at fixed fixture geometry; recorded screenshots
            # show the control. This uses RequestPaste, a different event path.
            self.x("mousemove", "--window", self.window, 203, 98)
            self.x("click", "1")
        else:
            self.key("ctrl+v")
        self.wait_for_body()
        before = self.save("before")
        self.shot("before")
        return before

    def wait_for_body(self):
        # Fresh documents start blank. Wait for ink inside the white body,
        # excluding chrome and the page edge. A caret alone is 24 dark pixels
        # in this fixed-size fixture; alpha plus its caret is above 35.
        # Fixed fixture geometry is intentional (1200x900, scale factor 1).
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            pixels = subprocess.check_output(["magick", "import", "-window", self.window, "png:-"],
                                             env=self.env, timeout=10)
            ink = subprocess.check_output([
                "magick", "png:-", "-crop", "1000x240+90+330",
                "-colorspace", "Gray", "-threshold", "50%", "-format",
                "%[fx:240000*(1-mean)]", "info:"
            ], env=self.env, input=pixels, timeout=10)
            if float(ink) > 35:
                time.sleep(0.3)
                return
            time.sleep(0.2)
        self.shot("render-timeout")
        raise TimeoutError("body text never rendered; see render-timeout.png and app.log")

    def save(self, name):
        self.key("ctrl+s")
        time.sleep(0.4)
        with sqlite3.connect(f"file:{self.database}?mode=ro", uri=True) as db:
            row = db.execute("SELECT json FROM snapshots WHERE revision = "
                             "(SELECT value FROM meta WHERE key='head_revision')").fetchone()
        if row is None:
            raise RuntimeError("no saved snapshot")
        snapshot = json.loads(row[0])
        (self.directory / f"{name}.json").write_text(json.dumps(snapshot, ensure_ascii=False, indent=2))
        return snapshot

    def shot(self, name):
        subprocess.run(["magick", "import", "-window", self.window,
                        str(self.directory / f"{name}.png")], env=self.env, check=True, timeout=10)

    def close(self):
        self.app.terminate()
        try:
            self.app.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.app.kill()
            self.app.wait(timeout=5)
        self.log.close()


def content(snapshot):
    return [block["content"] for block in snapshot["blocks"]]


def formula_enter(window, display=False):
    source = " alpha/2 " if display else "alpha"
    before = window.seed(f"${source}$")
    assert content(before) == [[{"Math": source}]], before
    # Visual End lands before the hidden closing delimiter, unlike Ctrl+A/Right.
    window.key("Home")
    window.key("End")
    window.key("Return")
    window.type("tail")
    actual = window.save("after")
    window.shot("after")
    assert content(actual) == [[{"Math": source}], [{"Text": "tail"}]], content(actual)


def formula_backspace(window):
    before = window.seed("$alpha$")
    assert content(before) == [[{"Math": "alpha"}]], before
    window.key("ctrl+a")
    window.key("Right")
    window.key("BackSpace")
    actual = window.save("after")
    window.shot("after")
    assert not any("$" in inline.get("Text", "") for block in content(actual)
                   for inline in block), content(actual)


def plain_split(window):
    before = window.seed("abc")
    assert content(before) == [[{"Text": "abc"}]], before
    window.key("End")
    window.key("Return")
    window.type("def")
    actual = window.save("after")
    window.shot("after")
    assert content(actual) == [[{"Text": "abc"}], [{"Text": "def"}]], content(actual)


def undo(window):
    window.seed("before")
    window.type(" after")
    changed = window.save("changed")
    assert content(changed) == [[{"Text": "before after"}]], changed
    window.key("ctrl+z")
    actual = window.save("after")
    window.shot("after")
    assert content(actual) == [[{"Text": "before"}]], content(actual)


def crlf_paste(window, ribbon=False):
    actual = window.seed("first\r\nsecond\r\n", ribbon=ribbon)
    assert content(actual) == [[{"Text": "first"}], [{"Text": "second"}], []], content(actual)


def views_and_restore(window):
    original = window.seed("中文 $alpha$\nsecond")
    window.key("ctrl+2")
    window.shot("source")
    window.key("ctrl+1")
    after = window.save("after")
    assert after == original, "view switch mutated document"
    window.close()
    restored = Window(window.directory)
    try:
        restored.shot("restored")
        actual = restored.save("restored")
        assert actual == original, "reopen changed saved snapshot"
        # Verify restoration in the live app, not only the already-existing DB.
        restored.key("ctrl+a")
        restored.key("ctrl+c")
        copied = subprocess.check_output(["xclip", "-selection", "clipboard", "-o"],
                                         env=restored.env, text=True, timeout=5)
        assert copied == "中文 $alpha$\nsecond", copied
    finally:
        restored.close()


def main():
    out = Path(sys.argv[1]).resolve()
    out.mkdir(parents=True, exist_ok=True)
    scenarios = [("inline-enter", formula_enter),
                 ("display-enter", lambda w: formula_enter(w, display=True)),
                 ("formula-backspace", formula_backspace), ("plain-split", plain_split),
                 ("undo", undo), ("views-save-restore", views_and_restore),
                 ("keyboard-crlf-paste", crlf_paste),
                 ("ribbon-crlf-paste", lambda w: crlf_paste(w, ribbon=True))]
    results = []
    for name, scenario in scenarios:
        window = None
        try:
            window = Window(out / name)
            scenario(window)
            result = {"name": name, "status": "passed"}
        except AssertionError as exc:
            result = {"name": name, "status": "failed", "actual": str(exc)}
        except Exception as exc:
            result = {"name": name, "status": "error", "error": repr(exc)}
        finally:
            if window and window.app.poll() is None:
                window.close()
        results.append(result)
        print(json.dumps(result, ensure_ascii=False), flush=True)
        (out / "results.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
    return 2 if any(r["status"] == "error" for r in results) else int(any(r["status"] == "failed" for r in results))


if __name__ == "__main__":
    raise SystemExit(main())
