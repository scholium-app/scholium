"""Native Ribbon smoke review. Build scholium-app, then run under xvfb-run.

Requires xdotool, xclip and ImageMagick. Uses an isolated temporary session.
Screenshots and the application log remain in the printed output directory.
"""

import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
OUT = Path(tempfile.mkdtemp(prefix="scholium-ribbon-review-"))
ENV = os.environ.copy()
ENV.pop("WAYLAND_DISPLAY", None)
ENV.update(
    WINIT_UNIX_BACKEND="x11",
    WINIT_X11_SCALE_FACTOR="1",
    SCHOLIUM_SESSION_FILE=str(OUT / "session.sqlite"),
)
CHECKS = []


def x(*args):
    return subprocess.check_output(
        ["xdotool", *map(str, args)], env=ENV, text=True
    ).strip()


def key(value):
    x("key", "--clearmodifiers", value)
    time.sleep(0.35)


def click(px, py):
    x("mousemove", "--window", WINDOW, px, py)
    x("click", 1)
    time.sleep(0.4)


def clipboard(text):
    subprocess.run(
        ["xclip", "-selection", "clipboard"], input=text.encode(), env=ENV,
        check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )


def copied():
    return subprocess.check_output(
        ["xclip", "-selection", "clipboard", "-o"], env=ENV, text=True
    )


def shot(name):
    x("mousemove", "--window", WINDOW, 1200, 850)
    time.sleep(0.25)
    subprocess.run(
        ["magick", "import", "-window", WINDOW, str(OUT / f"{name}.png")],
        env=ENV, check=True,
    )


def expect_copy(expected, label):
    key("ctrl+a")
    clipboard("clipboard sentinel")
    click(275, 113)  # Home > Clipboard > Copy, logical pixels at 1x DPI.
    actual = copied()
    assert actual == expected, f"{label}: {actual!r} != {expected!r}"
    CHECKS.append(label)


def saved_snapshot():
    with sqlite3.connect(OUT / "session.sqlite") as database:
        row = database.execute(
            "SELECT json FROM snapshots WHERE revision = "
            "(SELECT value FROM meta WHERE key='head_revision')"
        ).fetchone()
    assert row is not None, "Save must persist a snapshot"
    return json.loads(row[0])


def review():
    x("windowsize", WINDOW, 1280, 900)
    x("windowfocus", WINDOW)
    time.sleep(1)
    key("ctrl+n")
    time.sleep(4)
    fixture = "Scholium 科学写作\n现在可以在页面上直接编辑正文与公式。\n频率比 $alpha/2 + sqrt(T/rho)$ 决定振动模态。"
    clipboard(fixture)
    click(203, 98)  # Home > Paste.
    time.sleep(1.5)
    shot("home")
    expect_copy(fixture, "Ribbon paste and copy preserve document text")
    click(275, 82)  # Cut the selected document.
    assert copied() == fixture, "Cut must copy the full selection"
    click(203, 98)
    time.sleep(1)
    expect_copy(fixture, "Ribbon cut and paste restore the selected text")
    key("Right")  # Collapse the document selection to its end, beyond hidden markup.
    key("Return")
    click(170, 49)  # Insert tab.
    shot("insert")
    click(120, 98)  # Display formula.
    x("type", "--delay", 100, "alpha/2")
    time.sleep(1)
    shot("formula")
    click(100, 49)  # Home.
    key("ctrl+1")  # Restore page focus after a tab change.
    expected = fixture + "\n$ alpha/2 $"
    expect_copy(expected, "Display formula button restores caret for typing")
    key("Right")
    key("Return")
    click(604, 98)  # Heading 1 style.
    x("type", "--delay", 70, "Ribbon heading")
    time.sleep(1)
    shot("heading")
    click(120, 98)  # Save.
    shot("saved")
    saved = saved_snapshot()
    assert len(saved["blocks"]) == 5, saved
    assert saved["blocks"][3]["content"] == [{"Math": " alpha/2 "}], saved
    assert saved["blocks"][4]["kind"] == "Heading1", saved
    assert saved["blocks"][4]["content"] == [{"Text": "Ribbon heading"}], saved
    CHECKS.append("Ribbon Save persists the formula and Heading1 block")
    review_views()


def review_views():
    click(240, 49)
    shot("layout")
    click(310, 49)
    shot("view")
    key("ctrl+2")
    click(100, 49)
    shot("source")
    key("ctrl+1")
    x("windowsize", WINDOW, 640, 480)
    time.sleep(0.5)
    shot("narrow")
    # Drag the visible horizontal scrollbar to the rightmost groups.
    x("mousemove", "--window", WINDOW, 290, 163)
    x("mousedown", 1)
    time.sleep(0.2)
    x("mousemove", "--window", WINDOW, 620, 163)
    time.sleep(0.2)
    x("mouseup", 1)
    time.sleep(0.5)
    shot("narrow-scrolled")
    key("ctrl+F1")
    shot("collapsed")
    click(170, 49)
    shot("narrow-expanded")
    CHECKS.append("All tabs, source mode, narrow scroll and collapse captured")


if __name__ == "__main__":
    print(OUT, flush=True)
    with (OUT / "app.log").open("w") as log:
        app = subprocess.Popen(
            [str(ROOT / "target/debug/scholium-app")], env=ENV,
            stdout=log, stderr=subprocess.STDOUT,
        )
        try:
            for _ in range(100):
                time.sleep(0.1)
                result = subprocess.run(
                    ["xdotool", "search", "--onlyvisible", "--pid", str(app.pid)],
                    env=ENV, capture_output=True, text=True,
                )
                if result.returncode == 0:
                    break
            else:
                raise RuntimeError(f"No application window; see {OUT / 'app.log'}")
            WINDOW = result.stdout.splitlines()[-1]
            review()
        finally:
            app.terminate()
            app.wait(timeout=5)
            (OUT / "checks.json").write_text(json.dumps(CHECKS, ensure_ascii=False, indent=2))
    print(json.dumps(CHECKS, ensure_ascii=False, indent=2), flush=True)
