#!/usr/bin/env python3
"""Kernel assembly, modal keyboard policy and before/after visual evidence (private Xvfb)."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

sys.path.insert(0, str(Path.cwd() / "spikes/typst-main-editor"))
from project_ui import ProjectApp
from chrome_ui import Chrome

binary, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
baseline = os.environ.get("SCHOLIUM_FOUNDATION_RECORD_ONLY") == "1"
results = {}


def latest(app, prefix, predicate=lambda s: True):
    return next((s for s in reversed(app.entries(prefix)) if predicate(s)), None)


def facts(state):
    return state["chrome"]["facts"]


def native():
    app = ProjectApp(binary, output, output / "Research notes")
    app.start("native")
    try:
        app.state()
        chrome = Chrome(lambda: app.state(), app.xdo, app.window)
        app.type("Research notes\nA short paragraph about mathematical writing.")
        app.state(lambda s: s["current"] and s["revision"] > 0)
        app.key("ctrl+s")
        app.capture("typst-page-light")
        state = app.state()
        if not baseline:
            assert "tools.search" not in state["controls"]
            assert not any(k.startswith("document.") for k in state["controls"])
            assert "ui.status.count" in state["controls"]
            assert facts(state)["tab_title"] == "Research notes"
        chrome.menu("ui.menu.view", "ui.menu.view.theme", "view.theme.dark")
        app.state(lambda s: facts(s)["theme"] == "Dark")
        app.capture("typst-page-dark")
        chrome.menu("ui.menu.view", "ui.menu.view.theme", "view.theme.light")
        app.state(lambda s: facts(s)["theme"] == "Light")
        raster = app.state()["raster"]
        app.xdo("mousemove", "--window", app.window, round(raster["page_left_px"] + 100), round(raster["page_top_px"] + 50))
        app.xdo("click", 1)
        app.key("ctrl+End")
        app.type(" $x")
        app.state(lambda s: s["current"] and "insert.exit_formula" in s["controls"])
        app.capture("formula-focus")
        app.key("Escape")
        chrome.click("ui.menu.format", "menubar")
        app.capture("menu")
        app.key("Escape")
        app.key("ctrl+j")
        app.capture("diagnostics")
        app.key("ctrl+j")
        app.key("ctrl+s")
        if not baseline:
            chrome.menu("ui.menu.help", "help.shortcuts")
            app.state(lambda s: facts(s)["file_modal"])
            revision = app.state()["revision"]
            app.type("must not enter the document")
            app.key("Escape")
            state = app.state(lambda s: not facts(s)["file_modal"])
            assert state["revision"] == revision
        app.key("ctrl+n")
        app.wait(lambda: latest(app, "PROJECT_UI ", lambda s: s["control"] == "path"))
        app.capture("dialog-new")
        if not baseline:
            app.key("Escape")
            app.state(lambda s: not facts(s)["file_modal"])
            app.key("ctrl+o")
            app.wait(lambda: latest(app, "DIALOG_FRAME "))
            app.key("Escape")
            app.state(lambda s: not facts(s)["file_modal"])
            chrome.menu("ui.menu.file", "file.save_as")
            app.key("Escape")
            app.state(lambda s: not facts(s)["file_modal"])
        else:
            app.control("cancel") if latest(app, "PROJECT_UI ", lambda s: s["control"] == "cancel") else None
            # Baseline has no Escape handler: click its visible Cancel text by a new file shortcut
            # is not reliable; close/reopen the throwaway app for the narrow capture.
            app.kill()
            app.start("native-narrow")
            app.state()
        app.xdo("windowsize", app.window, 480, 480)
        time.sleep(0.3)
        app.capture("window-480")
        app.key("ctrl+n")
        app.wait(lambda: latest(app, "PROJECT_UI ", lambda s: s["control"] == "path"))
        app.capture("dialog-480")
        if not baseline:
            frame = latest(app, "DIALOG_FRAME ")
            x0, y0, x1, y1 = frame["rect"]
            assert x0 >= 15 and x1 <= 465 and y0 >= 0 and y1 <= 480, frame
            app.control("name")
            app.type("Created with Enter")
            app.control("path")
            app.key("ctrl+a")
            app.type(str(output))
            app.control("typst")
            app.key("Return")
            app.state(lambda s: facts(s)["tab_title"] == "Created with Enter" and not facts(s)["file_modal"])
            assert (output / "Created with Enter").exists()
            chrome.menu("ui.menu.file", "file.recent")
            app.state(lambda s: facts(s)["file_modal"])
            time.sleep(0.25)
            frame = latest(app, "DIALOG_FRAME ")
            assert frame["rect"][2] - frame["rect"][0] <= 449, frame
            app.key("Escape")
            app.state(lambda s: not facts(s)["file_modal"])
        results["native-modal-and-chrome"] = True
    finally:
        app.kill()


def source(kind):
    root = output / ("Typst source" if kind == "typ" else "LaTeX manuscript")
    root.mkdir()
    entry = root / ("main." + kind)
    entry.write_text("Source paragraph\nSecond line" if kind == "typ" else
                     "\\documentclass{article}\n\\begin{document}\nA paragraph.\n\\end{document}\n")
    app = ProjectApp(binary, output, entry)
    # Exercise the missing-toolchain state independently of optional local toolchains.
    for key in list(app.env):
        if key.startswith("SCHOLIUM_RTEX") or key.startswith("SCHOLIUM_TEXLIVE"):
            app.env.pop(key)
    app.start(kind)
    try:
        app.source()
        if not baseline:
            state = app.source(lambda s: "chrome" in s)
            chrome = Chrome(lambda: app.source(lambda s: "chrome" in s), app.xdo, app.window)
            controls = state["controls"]
            assert not any(k.startswith(("format.", "insert.")) for k in controls)
            assert {k for k in controls if k.startswith("document.")} == {"document.project_settings"}
            assert "ui.menu.format" not in controls and "ui.menu.insert" not in controls
            assert facts(state)["tab_title"] == root.name
            assert facts(state)["kernel"] == ("TypstSource" if kind == "typ" else "Latex")
            if kind == "typ":
                app.control("source")
            app.key("ctrl+End")
            app.type(" edit")
            app.source(lambda s: s["text"].endswith(" edit"))
            # History buttons use TextEdit's history and return focus to its source caret.
            app.source(lambda s: any(e["enabled"] for e in s["controls"].get("edit.undo", [])))
            chrome.click("edit.undo", "toolbar")
            app.source(lambda s: not s["text"].endswith(" edit"))
            app.source(lambda s: any(e["enabled"] for e in s["controls"].get("edit.redo", [])))
            chrome.click("edit.redo", "toolbar")
            app.source(lambda s: s["text"].endswith(" edit"))
        app.capture("typ-project" if kind == "typ" else "latex-workbench")
        if kind == "typ":
            app.key("ctrl+s")
            app.control("source")
            app.key("ctrl+End")
            app.type(" local draft")
            app.source(lambda s: s["dirty"])
            entry.write_text("External content")
            app.key("ctrl+s")
            app.source(lambda s: bool(s["error"]))
            app.capture("external-conflict")
            if not baseline:
                app.key("Return")
                app.control("path")
                app.key("Escape")
                assert entry.read_text() == "External content"
        results[kind + "-assembly-and-history"] = True
    finally:
        app.kill()


native()
source("typ")
source("tex")
(output / "summary.json").write_text(json.dumps(results, indent=2) + "\n")
print("PASS foundation kernel assembly, history, modal keys and bounded layout")
