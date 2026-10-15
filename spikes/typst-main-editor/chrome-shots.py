#!/usr/bin/env python3
"""Window-chrome screenshots under a private Xvfb: every size x scale x theme x locale for the
four states the design shows (text focus, math focus, source mode, open 格式 menu).

Not a check (the name has no `-check` suffix, so window-checks.py never runs it). It is how the
evidence under docs/log/evidence/window-chrome/ is produced; controls are clicked by command id
through chrome_ui, never by coordinate.

  xvfb-run -a -s '-screen 0 2300x1500x24' python3 spikes/typst-main-editor/chrome-shots.py \\
    target/debug/scholium-app docs/log/evidence/window-chrome \\
    [--sizes 1440x900 1024x700] [--scales 1 1.5] [--locales zh-Hans en] \\
    [--themes light dark] [--states text math source menu overflow tooltip] [--full]

`--full` keeps lossless PNGs; by default shots are quantized to 256 colors to keep the repository small.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parent))
import chrome_ui  # noqa: E402

parser = argparse.ArgumentParser()
parser.add_argument("binary")
parser.add_argument("outdir")
parser.add_argument("--sizes", nargs="+", default=["1440x900", "1024x700"])
parser.add_argument("--scales", nargs="+", default=["1", "1.5"])
parser.add_argument("--locales", nargs="+", default=["zh-Hans", "en"])
parser.add_argument("--themes", nargs="+", default=["light", "dark"])
parser.add_argument("--states", nargs="+", default=["text", "math", "source", "menu"])
parser.add_argument("--full", action="store_true")
args = parser.parse_args()
binary = Path(args.binary).resolve()
out = Path(args.outdir).resolve()
out.mkdir(parents=True, exist_ok=True)
# Session files and app logs stay out of the evidence directory.
WORK = Path(tempfile.mkdtemp(prefix="chrome-shots-"))

DOCUMENT = (
    "有限差分格式的稳定性分析\n王 明 · 计算数学讲义 · 2026 年 10 月\n引言\n"
    "有限差分方法把偏微分方程在网格上离散为代数方程组，是求解热传导、对流扩散等问题最常用的"
    "数值手段之一。一个格式是否稳定，决定了舍入误差在时间推进中被抑制还是被放大。\n"
    "本讲义以一维热传导方程为例，介绍 von Neumann 稳定性分析的基本思路，并约定全文记号："
    "行内公式与正文共用基线，例如 E = mc"
)


def run_session(locale, scale):
    work = WORK / f"{locale}-{scale}"
    work.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, SCHOLIUM_SESSION_FILE=str(work / "doc.sqlite"), SCHOLIUM_EDITOR_AUDIT="1",
               SCHOLIUM_LOCALE=locale, WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR=scale)
    env.pop("WAYLAND_DISPLAY", None)
    (work / "doc.sqlite").unlink(missing_ok=True)
    log_path = work / "app.log"

    def state():
        lines = [l for l in log_path.read_text().split("\n")[:-1] if l.startswith("AUDIT ")]
        return json.loads(lines[-1][6:]) if lines else None

    def xdo(*a):
        subprocess.run(["xdotool", *map(str, a)], check=True, env=env)

    with log_path.open("w") as log:
        app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
    try:
        end = time.monotonic() + 60
        while time.monotonic() < end and not (state() or {}).get("current"):
            time.sleep(0.1)
        window = subprocess.check_output(["xdotool", "search", "--pid", str(app.pid)], env=env, text=True).split()[0]
        xdo("windowfocus", window)
        chrome = chrome_ui.Chrome(state, xdo, window)
        yield env, state, xdo, window, chrome
    finally:
        app.terminate()
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired:
            app.kill()


def settle(state, seconds=0.6):
    time.sleep(seconds)
    return state()


def expect(state, context, what, timeout=15):
    """Wait until the status bar's context is `context` (the AUDIT chrome facts)."""
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        facts = ((state() or {}).get("chrome") or {}).get("facts") or {}
        if (facts.get("status") or {}).get("context") == context:
            time.sleep(0.5)
            return
        time.sleep(0.1)
    raise RuntimeError(f"{what}: status context never became {context}")


IMAGEMAGICK = shutil.which("magick") or "convert"


def snap(window, env, path):
    time.sleep(0.4)
    raw = path.with_suffix(".raw.png")
    subprocess.run(["import", "-window", window, str(raw)], check=True, env=env)
    if args.full:
        raw.rename(path)
    else:
        # No dithering: it would speckle the flat bar and canvas colors.
        subprocess.run([IMAGEMAGICK, str(raw), "-dither", "None", "-colors", "256", "-strip", str(path)], check=True)
        raw.unlink()


def resize(xdo, window, size, scale):
    width, height = (float(v) for v in size.split("x"))
    xdo("windowsize", window, round(width * float(scale)), round(height * float(scale)))
    time.sleep(0.8)


def set_theme(chrome, theme):
    chrome.menu("ui.menu.view", "ui.menu.view.theme", f"view.theme.{theme}")


def new_document(chrome, work, xdo, window, scale):
    """File > New through the "+" tab button, confirming the discard dialog by its audited button."""
    log = work / "app.log"
    seen = log.read_text().count("DIALOG ")
    chrome.click("file.new", "tab")
    end = time.monotonic() + 10
    while time.monotonic() < end:
        dialogs = [l for l in log.read_text().split("\n") if l.startswith("DIALOG ")]
        if len(dialogs) > seen:
            x, y = json.loads(dialogs[-1][7:])["proceed"]
            time.sleep(0.3)
            xdo("mousemove", "--window", window, round(x * float(scale)), round(y * float(scale)))
            xdo("click", 1)
            time.sleep(0.8)
            return
        time.sleep(0.05)
    raise RuntimeError("the discard dialog never showed")


def reset_document(chrome, env, state, xdo, window, scale, work):
    """The sample paper again: title and section as headings, caret in the first body paragraph."""
    xdo("key", "Escape")
    xdo("key", "Escape")
    new_document(chrome, work, xdo, window, scale)
    subprocess.run(["xclip", "-selection", "clipboard"], input=DOCUMENT.encode(), env=env, check=True)
    xdo("key", "--clearmodifiers", "ctrl+v")
    time.sleep(1.0)
    xdo("key", "--clearmodifiers", "ctrl+Home")
    chrome.click("ui.toolbar.paragraph", "toolbar")
    chrome.click("format.paragraph.heading1", "menu")
    xdo("key", "Down", "Down")
    chrome.click("ui.toolbar.paragraph", "toolbar")
    chrome.click("format.paragraph.heading2", "menu")
    # Caret in the first body paragraph with the page scrolled to its top, like the design images.
    xdo("key", "Down")
    xdo("key", "--clearmodifiers", "ctrl+Home")
    xdo("key", "Down", "Down", "Down")
    settle(state, 1.0)


def main():
    shots = 0
    for locale in args.locales:
        for scale in args.scales:
            session = run_session(locale, scale)
            env, state, xdo, window, chrome = next(session)
            work = WORK / f"{locale}-{scale}"
            try:
                for theme in args.themes:
                    set_theme(chrome, theme)
                    for size in args.sizes:
                        resize(xdo, window, size, scale)
                        shots += capture(chrome, env, state, xdo, window, work, (locale, scale, theme, size))
            finally:
                session.close()
    print(f"{shots} screenshots in {out}")


def capture(chrome, env, state, xdo, window, work, config):
    locale, scale, theme, size = config
    tag = f"{size}-s{scale}-{theme}-{'zh' if locale.startswith('zh') else locale}"
    taken = 0
    for name in args.states:
        reset_document(chrome, env, state, xdo, window, scale, work)
        path = out / f"{tag}-{name}.png"
        if name == "math":
            xdo("key", "End")
            xdo("type", "--delay", "40", " $")
            xdo("type", "--delay", "40", "x")
            xdo("key", "--clearmodifiers", "ctrl+slash")
            xdo("type", "--delay", "40", "1")
            expect(state, "chrome.status.context.math", "formula focus")
        elif name == "source":
            xdo("key", "--clearmodifiers", "ctrl+shift+m")
            expect(state, "chrome.status.context.source", "source mode")
            time.sleep(2.5)
        elif name == "tooltip":
            chrome.hover("format.italic", "toolbar")
            time.sleep(1.5)
        elif name == "overflow":
            chrome.click("ui.toolbar.overflow", "toolbar")
            chrome.wait("edit.redo", "overflow")
            settle(state)
        elif name == "menu":
            chrome.click("ui.menu.format", "menubar")
            chrome.hover("ui.menu.format.paragraph_style", "menu")
            settle(state)
        snap(window, env, path)
        if name == "source":
            xdo("key", "--clearmodifiers", "ctrl+shift+m")
            time.sleep(0.6)
        xdo("key", "Escape")
        xdo("key", "Escape")
        taken += 1
    return taken


main()
