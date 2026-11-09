#!/usr/bin/env python3
"""Real-window chrome check: every control is found through the AUDIT `controls` map and clicked
by command id, in the interface language given by SCHOLIUM_LOCALE (window-checks.py runs it in
zh-Hans and en). Covers menus, submenus, the main toolbar and its overflow list, the focus
toolbar following the caret, the tab bar's new/close flows, the status bar, the theme and
language switches, and Ctrl+F1.

Run under Xvfb from the repository root:
  SCHOLIUM_LOCALE=en xvfb-run -a -s '-screen 0 1280x900x24' python3 \\
    spikes/typst-main-editor/chrome-check.py target/debug/scholium-app /tmp/out
"""
import json
import os
import re
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
env = dict(os.environ, SCHOLIUM_PROJECT_PATH=str(output / "project"), SCHOLIUM_EDITOR_AUDIT="1",
           SCHOLIUM_LOCALE=locale, WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR="1")
env.pop("WAYLAND_DISPLAY", None)
log_path = output / "app.log"
results = {}
MENUS = ["file", "edit", "insert", "format", "view", "help"]


def state():
    lines = [l for l in log_path.read_text().split("\n")[:-1] if l.startswith("AUDIT ")]
    return json.loads(lines[-1][6:]) if lines else None


def wait(predicate, what, timeout=30):
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


def kinds(s):
    return [b["kind"] for b in s["snapshot"]["blocks"]]


def texts(s):
    return [b["body"]["text"] for block in s["snapshot"]["blocks"] for b in block["content"]
            if b["body"]["kind"] == "text"]


CJK = re.compile("[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\U00020000-\U0002fa1f\u3040-\u30ff\uac00-\ud7af]")
LATIN_WORD = re.compile(r"[^\W_]+(?:['\u2019-][^\W_]+)*")


def words(values):
    """The status bar's rule: each CJK character is a word, so is each run of other letters/digits."""
    return sum(len(CJK.findall(v)) + len(LATIN_WORD.findall(CJK.sub(" ", v))) for v in values)


def facts(s):
    return s["chrome"]["facts"]


def drawn(s, command_id, surface):
    return any(e["surface"] == surface for e in s["controls"].get(command_id, []))


def close_menus():
    xdo("key", "Escape")
    xdo("key", "Escape")
    time.sleep(0.2)


def type_text(value):
    xdo("type", "--delay", "30", value)


def focus_page():
    """Click into the sheet, as a user does after leaving a menu."""
    r = state()["raster"]
    xdo("mousemove", "--window", window, round(r["page_left_px"] + r["displayed_width_px"] / 2),
        round(r["page_top_px"] + 200))
    xdo("click", 1)
    time.sleep(0.2)


with log_path.open("w") as log:
    app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
try:
    wait(lambda s: s["current"], "first scene")
    window = subprocess.check_output(["xdotool", "search", "--sync", "--onlyvisible", "--pid", str(app.pid)], env=env, text=True).split()[0]
    xdo("windowfocus", window)
    chrome = chrome_ui.Chrome(state, xdo, window)
    ppp = state()["chrome"]["pixels_per_point"]

    # 1. The audit map covers every bar, and the language and fonts are what the run asked for.
    s = wait(lambda s: drawn(s, "ui.menu.file", "menubar") and drawn(s, "view.mode.page", "focus"), "chrome drawn")
    ok = (all(drawn(s, f"ui.menu.{m}", "menubar") for m in MENUS)
          and drawn(s, "format.bold", "toolbar") and not drawn(s, "tools.search", "toolbar")
          and not drawn(s, "document.page_size", "focus") and drawn(s, "ui.tab", "tab")
          and drawn(s, "file.new", "tab") and drawn(s, "view.zoom.in", "status"))
    wanted = "ZhHans" if locale.startswith("zh") else "En"
    record("01-controls-map-covers-the-chrome", ok and facts(s)["locale"] == wanted and s["fonts"]["ui"],
           locale=facts(s)["locale"], fonts=s["fonts"], controls=len(s["controls"]))
    shot("01-chrome")

    # 2. Every menu opens from its title and closes again; submenus open on hover.
    for menu in MENUS:
        chrome.click(f"ui.menu.{menu}", "menubar")
        rows = [id for id, found in state()["controls"].items() if any(e["surface"] == "menu" for e in found)]
        assert rows, f"{menu} menu is empty"
        close_menus()
        assert chrome.gone(rows[0], "menu"), f"{menu} menu did not close"
    chrome.click("ui.menu.format", "menubar")
    chrome.hover("ui.menu.format.paragraph_style", "menu")
    chrome.wait("format.paragraph.heading1", "menu")
    shot("02-format-menu")
    close_menus()
    record("02-menus-open-and-close", True)

    # 3. Toolbar formatting: bold toggles through its control and its active state follows.
    focus_page()
    type_text("hello")
    xdo("key", "--clearmodifiers", "shift+Home")
    chrome.click("format.bold", "toolbar")
    s = wait(lambda s: any(e["checked"] for e in s["controls"]["format.bold"] if e["surface"] == "toolbar"),
             "bold shows active")
    s = wait(lambda s: [i["body"]["style"] for i in s["snapshot"]["blocks"][0]["content"]] == ["strong"],
             "the selection is bold")
    chrome.click("format.bold", "toolbar")
    s = wait(lambda s: not any(e["checked"] for e in s["controls"]["format.bold"] if e["surface"] == "toolbar"),
             "bold inactive again")
    record("03-toolbar-bold-toggles-and-shows-active", [i["body"]["style"] for i in s["snapshot"]["blocks"][0]["content"]] == ["plain"])

    # 4. The paragraph dropdown, then history through toolbar and Edit menu.
    xdo("key", "--clearmodifiers", "End")
    chrome.click("ui.toolbar.paragraph", "toolbar")
    chrome.click("format.paragraph.heading1", "menu")
    wait(lambda s: kinds(s) == ["Heading1"], "heading 1 applied")
    chrome.click("edit.undo", "toolbar")
    wait(lambda s: kinds(s) == ["Paragraph"], "undo through the toolbar")
    chrome.menu("ui.menu.edit", "edit.redo")
    wait(lambda s: kinds(s) == ["Heading1"], "redo through the Edit menu")
    record("04-paragraph-dropdown-and-history", True)

    # 5. Work mode through the View submenu and the focus toolbar's segmented control.
    chrome.menu("ui.menu.view", "ui.menu.view.work_mode", "view.mode.source")
    s = wait(lambda s: s["source_mode"] and s["preview"]["shown"], "source mode from the View menu")
    assert facts(s)["status"]["context"] == "chrome.status.context.source"
    shot("05-source")
    chrome.click("view.mode.page", "focus")
    wait(lambda s: not s["source_mode"], "Page editing from the segmented control")
    record("05-work-mode-from-menu-and-segmented-control", True)

    # 6. The focus toolbar follows the caret: formula commands inside a formula.
    focus_page()
    xdo("key", "--clearmodifiers", "ctrl+End")
    type_text(" $")
    s = wait(lambda s: drawn(s, "insert.exit_formula", "focus"), "formula focus bar")
    assert not drawn(s, "document.page_size", "focus"), "document properties stay hidden in a formula"
    assert drawn(s, "view.mode.source", "focus"), "the mode switch is always there"
    shot("06-math-focus")
    chrome.click("insert.exit_formula", "focus")
    s = wait(lambda s: not drawn(s, "insert.exit_formula", "focus"), "body focus bar")
    record("06-focus-bar-follows-the-caret", not drawn(s, "insert.exit_formula", "focus"))

    # 7. Removing placeholders leaves all implemented groups usable at the minimum width.
    # Overflow ordering at smaller widths remains covered by pure/headless chrome tests.
    xdo("windowsize", window, 480, 700)
    s = wait(lambda s: drawn(s, "edit.undo", "toolbar"), "minimum-width toolbar")
    assert not drawn(s, "tools.search", "toolbar") and drawn(s, "view.mode.page", "focus")
    assert not drawn(s, "ui.toolbar.overflow", "toolbar")
    shot("07-minimum-width")
    before = state()["snapshot"]
    chrome.click("edit.undo", "toolbar")
    wait(lambda s: s["snapshot"] != before, "undo at minimum width changes the document")
    xdo("windowsize", window, 1280, 900)
    record("07-minimum-width-keeps-implemented-groups-usable", True)

    # 8. Ctrl+F1 and the View menu both toggle the toolbars. F1 goes by keycode: xdotool would
    # add Alt to the keysym on xkeyboard-config >= 2.46 (see chrome_ui.keycode).
    xdo("key", "--clearmodifiers", f"ctrl+{chrome_ui.keycode(env['DISPLAY'], 'F1')}")
    s = wait(lambda s: not drawn(s, "format.bold", "toolbar") and not drawn(s, "view.mode.page", "focus"),
             "toolbars hidden")
    assert drawn(s, "ui.menu.file", "menubar") and drawn(s, "ui.tab", "tab")
    chrome.menu("ui.menu.view", "view.toolbars")
    wait(lambda s: drawn(s, "format.bold", "toolbar"), "toolbars shown again")
    record("08-toolbars-toggle", True)

    # 9. Theme and language switch at runtime through the View menu.
    chrome.menu("ui.menu.view", "ui.menu.view.theme", "view.theme.dark")
    s = wait(lambda s: facts(s)["theme"] == "Dark", "dark theme")
    shot("09-dark")
    other = "view.language.en" if wanted == "ZhHans" else "view.language.zh_hans"
    chrome.menu("ui.menu.view", "ui.menu.view.language", other)
    s = wait(lambda s: facts(s)["locale"] == ("En" if wanted == "ZhHans" else "ZhHans"), "language switched")
    shot("09-switched-language")
    back = "view.language.zh_hans" if wanted == "ZhHans" else "view.language.en"
    chrome.menu("ui.menu.view", "ui.menu.view.language", back)
    chrome.menu("ui.menu.view", "ui.menu.view.theme", "view.theme.light")
    s = wait(lambda s: facts(s)["theme"] == "Light" and facts(s)["locale"] == wanted, "light theme and language back")
    record("09-theme-and-language-switch-at-runtime", True)

    # 10. Status bar: words counted from the snapshot, page info only in source mode.
    focus_page()
    xdo("key", "--clearmodifiers", "ctrl+End")
    # CJK through the clipboard: xdotool types characters missing from the keymap by remapping a
    # spare keycode, which races with the app's keymap handling on some X servers (flaky on CI).
    subprocess.run(["xclip", "-selection", "clipboard"], input=" 中文 abc".encode(), env=env, check=True)
    xdo("key", "--clearmodifiers", "ctrl+v")
    s = wait(lambda s: "".join(texts(s)).endswith("中文 abc"), "text typed")
    s = wait(lambda s: facts(s)["status"]["words"] == words(texts(s)) and "".join(texts(s)).endswith("中文 abc"),
             "word count updates")
    assert facts(s)["status"]["words"] >= 3
    assert facts(s)["status"]["preview_page"] is None, "no page info in the continuous view"
    chrome.click("view.zoom.in", "status")
    chrome.click("view.zoom.fit_width", "status")
    record("10-status-bar-words-and-zoom", True, words=facts(s)["status"]["words"], texts=texts(s))

    # 11. New chooses a project/language; close discards and releases its project lock.
    chrome.click("file.new", "tab")
    from project_ui import ProjectApp
    controls = ProjectApp(binary, output, output / "new-project")
    controls.app, controls.log, controls.window, controls.env = app, log_path, window, env
    wait(lambda s: facts(s)["file_modal"], "new-project modal ready")
    controls.control("name")
    controls.type("new-project")
    controls.control("path")
    controls.key("ctrl+a")
    controls.type(str(output))
    controls.control("typst")
    controls.control("submit")
    controls.control("discard")
    s = wait(lambda s: s["current"] and s["raster"] is not None and texts(s) == [""], "new document replaced the old one")
    record("11a-tab-plus-runs-the-new-document-flow", True)
    focus_page()
    type_text("unsaved work")
    wait(lambda s: texts(s) == ["unsaved work"], "dirty again")
    chrome.click("file.close", "tab")
    controls.control("discard")
    end = time.monotonic() + 15
    while not any(json.loads(line[8:]).get("closed") for line in log_path.read_text().splitlines() if line.startswith("PROJECT ")) and time.monotonic() < end:
        time.sleep(0.1)
    record("11b-tab-close-discards-and-releases-the-project", any(json.loads(line[8:]).get("closed") for line in log_path.read_text().splitlines() if line.startswith("PROJECT ")))

finally:
    (output / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
    if app.poll() is None:
        app.terminate()
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired:
            app.kill()
