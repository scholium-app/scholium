#!/usr/bin/env python3
"""Real-window diagnostics: the status-bar warning jumps to the problem, the count opens the
docked panel, the panel's rows jump to the same place, Ctrl+J toggles it, Esc returns to the page.

Two formulas without operators cannot wrap and run past the text column, so each is a located
overflow diagnostic. Controls are found through the AUDIT `controls` map; locations are checked
against the AUDIT `diagnostics` list and the caret leaf, never against coordinates. Runs in the
interface language given by SCHOLIUM_LOCALE (window-checks.py runs zh-Hans and en).

  SCHOLIUM_LOCALE=en xvfb-run -a -s '-screen 0 1280x900x24' python3 \\
    spikes/typst-main-editor/diagnostics-check.py target/debug/scholium-app /tmp/out
"""
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
locale = os.environ.get("SCHOLIUM_LOCALE", "zh-Hans")
env = dict(os.environ, SCHOLIUM_SESSION_FILE=str(output / "doc.sqlite"), SCHOLIUM_EDITOR_AUDIT="1",
           SCHOLIUM_LOCALE=locale, WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR="1")
env.pop("WAYLAND_DISPLAY", None)
log_path = output / "app.log"
results = {}
ROW = "ui.diagnostics.row"
COUNT_TEXT = {"zh-Hans": "共 2 条", "en": "2 issues"}[chrome_ui.TAGS.get(locale, "zh-Hans")]
PANEL_TITLE = chrome_ui.catalog(locale)["diag-panel-problems"]


def state():
    lines = [l for l in log_path.read_text().split("\n")[:-1] if l.startswith("AUDIT ")]
    return json.loads(lines[-1][6:]) if lines else None


def wait(predicate, what, timeout=60):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        current = state()
        if current and predicate(current):
            return current
        time.sleep(0.05)
    raise RuntimeError(f"timeout: {what}")


def xdo(*args):
    subprocess.run(["xdotool", *map(str, args)], check=True, env=env)


def shot(name):
    time.sleep(0.3)
    subprocess.run(["import", "-window", window, str(output / f"{name}.png")], check=True, env=env)


def record(name, ok, **detail):
    results[name] = {"pass": bool(ok), **detail}
    print(("PASS " if ok else "FAIL ") + name, flush=True)
    if not ok:
        raise AssertionError(f"{name}: {detail}")


def settled(predicate):
    return lambda s: s["current"] and predicate(s)


def in_formula(s):
    return s["cursor_leaf"] not in {l["leaf"] for l in s["body_leaves"]}


def targets(s):
    return [d["target"] for d in s["diagnostics"] if d["target"]]


def at_target(s, index):
    if len(targets(s)) <= index:
        return False
    t = targets(s)[index]
    return s["cursor_leaf"] == t["leaf"] and s["cursor_byte"] == t["byte"] and in_formula(s)


def rows(s):
    return [e for e in s["controls"].get(ROW, []) if e["surface"] == "diagnostics"]


def click_rect(rect):
    x0, y0, x1, y1 = rect
    xdo("mousemove", "--window", window, round((x0 + x1) / 2), round((y0 + y1) / 2))
    time.sleep(0.05)
    xdo("click", 1)


def status_fact(s, name):
    return s["chrome"]["facts"]["status"][name]


with log_path.open("w") as log:
    app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
try:
    wait(lambda s: s["current"], "first scene")
    window = subprocess.check_output(["xdotool", "search", "--pid", str(app.pid)], env=env, text=True).split()[0]
    xdo("windowfocus", window)
    chrome = chrome_ui.Chrome(state, xdo, window)

    for first in ("a", "k"):
        xdo("type", "--delay", "20", "$" + "".join(chr(ord(first) + i) for i in range(10)) * 10)
        xdo("key", "Right")
        xdo("type", "--delay", "20", " tail ")
    s = wait(settled(lambda s: len(targets(s)) == 2 and not in_formula(s)), "two overflow diagnostics")
    shot("01-two-overflows")
    record("01-overflows-are-located-diagnostics",
           all(d["key"] == "diag.formula_overflow" and d["excerpt"] for d in s["diagnostics"]),
           diagnostics=s["diagnostics"])

    # The status bar shows the message and the count; the panel is closed.
    s = wait(lambda s: status_fact(s, "diagnostic_count") == 2 and "ui.status.count" in s["controls"], "count drawn")
    record("02-status-bar-shows-the-warning-and-the-count",
           status_fact(s, "warning") and not rows(s) and PANEL_TITLE not in json.dumps(s["controls"]),
           warning=status_fact(s, "warning"), count=COUNT_TEXT)

    # Clicking the warning moves the caret into the first overflowing formula.
    chrome.click("ui.status.warning", "status")
    s = wait(settled(lambda s: at_target(s, 0)), "the warning jumps to the first formula")
    record("03-warning-click-lands-in-the-formula", at_target(s, 0), cursor=[s["cursor_leaf"], s["cursor_byte"]])

    # Ctrl+J opens the docked panel with one row per diagnostic and no layout stats in the status bar.
    xdo("key", "--clearmodifiers", "ctrl+End")
    wait(settled(lambda s: not in_formula(s)), "caret back in the body")
    xdo("key", "--clearmodifiers", "ctrl+j")
    s = wait(lambda s: len(rows(s)) == 2, "panel rows")
    record("04-ctrl-j-opens-the-panel", len(rows(s)) == 2)
    shot("04-panel-light")

    # Clicking each row lands in its own formula.
    for index in (1, 0):
        xdo("key", "--clearmodifiers", "ctrl+End")
        wait(settled(lambda s: not in_formula(s)), "caret back in the body")
        click_rect(rows(state())[index]["rect"])
        s = wait(settled(lambda s, i=index: at_target(s, i)), f"row {index} jumps")
    record("05-row-click-lands-in-the-same-formula", at_target(s, 0))

    # Keyboard: the panel row has the focus after Ctrl+J, Down/Enter jumps to the next, Esc returns to the page.
    xdo("key", "--clearmodifiers", "ctrl+End")
    wait(settled(lambda s: not in_formula(s)), "caret back in the body")
    xdo("key", "--clearmodifiers", "ctrl+j")
    wait(lambda s: not rows(s), "panel closed by Ctrl+J")
    xdo("key", "--clearmodifiers", "ctrl+j")
    wait(lambda s: len(rows(s)) == 2, "panel reopened")
    time.sleep(0.4)
    xdo("key", "Down")
    time.sleep(0.4)
    xdo("key", "Return")
    s = wait(settled(lambda s: at_target(s, 1)), "Down then Enter jumps to the second diagnostic")
    xdo("key", "ctrl+End")
    time.sleep(0.2)
    xdo("key", "Down")
    time.sleep(0.2)
    xdo("key", "Escape")
    time.sleep(0.2)
    before = state()["revision"]
    xdo("type", "z")
    s = wait(lambda s: s["revision"] > before, "typing reaches the page after Esc")
    record("06-keyboard-rows-and-escape", True, revision=s["revision"])

    # The count toggles the panel; Ctrl+J closes it.
    chrome.click("ui.status.count", "status")
    s = wait(lambda s: not rows(s), "count closes the open panel")
    chrome.click("ui.status.count", "status")
    s = wait(lambda s: len(rows(s)) == 2, "count opens the panel")
    record("07-count-toggles-the-panel", len(rows(s)) == 2)

    chrome.menu("ui.menu.view", "ui.menu.view.theme", "view.theme.dark")
    wait(lambda s: s["chrome"]["facts"]["theme"] == "Dark", "dark theme")
    wait(lambda s: len(rows(s)) == 2, "panel stays open")
    shot("08-panel-dark")
    xdo("key", "--clearmodifiers", "ctrl+j")
    s = wait(lambda s: not rows(s), "panel closed")
    record("08-ctrl-j-closes-the-panel", not rows(s))
finally:
    (output / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
    if app.poll() is None:
        app.terminate()
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired:
            app.kill()
    time.sleep(0.2)
