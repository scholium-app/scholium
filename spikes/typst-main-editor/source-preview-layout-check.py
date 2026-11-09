#!/usr/bin/env python3
"""Shared source panes: focus, isolated preview, last-good pages, splitter and zoom."""
import json
import os
from pathlib import Path
import subprocess
import shutil
import sys
import time

from project_ui import ProjectApp
from chrome_ui import Chrome

binary, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
baseline = os.environ.get("SCHOLIUM_T46_BASELINE") == "1"
results = {}


def record(name, condition, **facts):
    results[name] = {"pass": bool(condition), **facts}
    print(("PASS " if condition else "FAIL ") + name, flush=True)
    assert condition, (name, facts)


def latest(app, prefix, predicate=lambda s: True):
    return app.wait(lambda: next((s for s in app.entries(prefix)[-1:] if predicate(s)), None))


def preview(app, predicate=lambda s: True):
    return latest(app, "SOURCE_PREVIEW ", predicate)


def clipboard(app, text):
    subprocess.run(["xclip", "-selection", "clipboard"], input=text.encode(), env=app.env, check=True)
    app.key("ctrl+v")


def chrome(app):
    return Chrome(lambda: app.source(), app.xdo, app.window)


def themes(app, prefix, controller=None):
    controller = controller or chrome(app)
    for theme in ["light", "dark"]:
        controller.menu("ui.menu.view", "ui.menu.view.theme", "view.theme." + theme)
        time.sleep(0.3)
        app.capture(prefix + "-" + theme)


def split_x(s):
    return s["source_rect"][2] + 4


def source_case():
    folder = output / "typ-source"
    folder.mkdir()
    project = folder / "paper"
    project.mkdir()
    entry = project / "main.typ"
    initial = '#set page(width: 300pt, height: 400pt)\n= Source preview\nHello 中文'
    entry.write_text(initial)
    app = ProjectApp(binary, folder, entry)
    app.start("source")
    try:
        app.source()
        if baseline:
            themes(app, "source")
            return
        first = preview(app, lambda s: s["pages"] > 0 and s["shown_revision"] == s["revision"])
        app.type("X")
        app.source(lambda s: s["text"] == initial + "X")
        edited = preview(app, lambda s: s["revision"] > first["revision"] and s["shown_revision"] == s["revision"])
        record("source-opens-focused-and-updates-preview", True, elapsed_ms=edited["elapsed_ms"])
        source_rect = edited["source_rect"]
        record("editor-fills-workspace-height", source_rect[3] - source_rect[1] > 650, rect=source_rect)
        themes(app, "source")

        s = preview(app)
        x, y = split_x(s) + 1, 450
        app.xdo("mousemove", "--window", app.window, round(x), y)
        app.xdo("mousedown", 1)
        time.sleep(0.15)
        app.xdo("mousemove", "--window", app.window, round(x + 130), y)
        time.sleep(0.15)
        app.xdo("mouseup", 1)
        dragged = preview(app, lambda p: split_x(p) > x + 100)
        app.key("Left")
        keyed = preview(app, lambda p: split_x(p) < split_x(dragged) - 10)
        record("splitter-drag-and-keyboard", True, drag=split_x(dragged), key=split_x(keyed))
        app.kill()
        app.start("reopened")
        reopened = preview(app, lambda p: p["pages"] > 0)
        record("splitter-persists-per-project", abs(split_x(reopened) - split_x(keyed)) < 2)
        x = split_x(reopened) + 1
        app.xdo("mousemove", "--window", app.window, round(x), 450)
        app.xdo("click", "--repeat", 2, "--delay", 100, 1)
        reset = preview(app, lambda p: abs(split_x(p) - 640) < 3)
        record("splitter-double-click-resets", True, x=split_x(reset))

        c = chrome(app)
        c.click("view.zoom.actual", "status")
        actual = preview(app, lambda p: abs((p["page_rect"][2] - p["page_rect"][0]) - 400) < 3)
        c.click("view.zoom.in", "status")
        zoomed = preview(app, lambda p: p["page_rect"][2] - p["page_rect"][0] > 410)
        c.click("view.zoom.fit_width", "status")
        fitted = preview(app, lambda p: p["page_rect"][2] - p["page_rect"][0] > 590)
        record("preview-follows-status-zoom-and-fit", True, actual=actual["page_rect"], zoom=zoomed["page_rect"], fit=fitted["page_rect"])
        app.control("source")
        app.key("ctrl+a")
        clipboard(app, initial + '\n#unknown_function()')
        failed = preview(app, lambda p: p["error"] and p["shown_revision"] < p["revision"])
        record("syntax-error-retains-last-good-pages-and-diagnostics", failed["pages"] > 0 and bool(failed["diagnostics"]["items"]), preview=failed)
        app.capture("source-error")
        app.key("ctrl+a")
        clipboard(app, initial + " repaired")
        preview(app, lambda p: not p["error"] and p["shown_revision"] == p["revision"])
        record("source-preview-recovers", True)
        app.key("ctrl+a")
        clipboard(app, initial + '\\n#pagebreak()\\nSecond page')
        preview(app, lambda p: p["pages"] == 2 and p["shown_revision"] == p["revision"])
        app.control("preview.next")
        preview(app, lambda p: p["page"] == 1)
        app.control("preview.previous")
        preview(app, lambda p: p["page"] == 0)
        record("preview-page-navigation", True)
        app.control("source")
        app.key("ctrl+a")
        clipboard(app, '#read("/etc/hostname")')
        denied = preview(app, lambda p: p["error"] and p["diagnostics"]["revision"] == p["revision"])
        record("isolated-preview-denies-host-files", denied["pages"] == 2 and bool(denied["diagnostics"]["items"]))
        app.key("ctrl+a")
        # Typst rejects endless while loops itself. Finite, deliberately expensive
        # evaluation exercises the worker's wall-clock limit without that guard.
        clipboard(app, '#for n in range(1000000) { let values = range(1000).map(x => calc.sqrt(x + n)) }')
        bounded = preview(app, lambda p: p["error"] and p["revision"] > denied["revision"] and p["diagnostics"]["revision"] == p["revision"])
        record("isolated-preview-bounds-expensive-evaluation", "10 second budget" in bounded["error"] and bounded["elapsed_ms"] < 15000 and bounded["pages"] == 2, error=bounded["error"], elapsed_ms=bounded["elapsed_ms"])
        app.key("ctrl+a")
        clipboard(app, initial)
        preview(app, lambda p: not p["error"] and p["shown_revision"] == p["revision"] and p["pages"] == 1)
        record("source-preview-recovers-after-budget", True)
    finally:
        app.kill()


def latex_case():
    folder = output / "latex"
    folder.mkdir()
    project = folder / "paper"
    project.mkdir()
    entry = project / "main.tex"
    entry.write_text('\\documentclass{article}\n\\begin{document}Preview\\end{document}\n')
    app = ProjectApp(binary, folder, entry)
    # Use real detector failure, without affecting the worker's fixed system executable paths.
    app.env["SCHOLIUM_RTEX_BIN"] = str(folder / "missing-rtex")
    app.env["SCHOLIUM_RTEX_TEXDIR"] = str(folder / "missing-tex")
    (folder / "xdotool").symlink_to(shutil.which("xdotool"))
    app.env["PATH"] = str(folder)
    # xdotool and screenshot helpers run with the host PATH; only the app inherits the missing tools.
    host_path = os.environ["PATH"]
    app.start("latex")
    app.env["PATH"] = host_path
    try:
        s = latest(app, "AUDIT ", lambda s: s.get("kind") == "latex" and s["toolchain_unavailable"])
        if not baseline:
            record("latex-missing-toolchain-centred-state", s["pages"] == 0 and bool(s["error"]))
        themes(app, "latex")
        if baseline:
            return
        before = latest(app, "AUDIT ", lambda s: s.get("kind") == "latex")
        c = chrome(app)
        c.click("view.mode.page", "focus")
        page = latest(app, "AUDIT ", lambda s: s.get("kind") == "latex" and s["source_rect"] is None)
        record("latex-page-mode-full-preview", page["preview_rect"][2] - page["preview_rect"][0] > 1200)
        app.capture("latex-page-mode")
        c.click("view.mode.source", "focus")
        restored = latest(app, "AUDIT ", lambda s: s.get("kind") == "latex" and s["source_rect"] is not None)
        record("latex-source-mode-restores-split", restored["source_rect"] == before["source_rect"])
    finally:
        app.kill()


def generated_case():
    folder = output / "generated"
    folder.mkdir()
    app = ProjectApp(binary, folder, folder / "paper")
    app.start("generated")
    try:
        app.state()
        clipboard(app, "Generated source 中文\nSecond paragraph")
        app.state(lambda s: s["current"] and s["snapshot"]["blocks"][0]["content"][0]["body"]["text"] == "Generated source 中文")
        controller = Chrome(lambda: app.state(), app.xdo, app.window)
        controller.click("view.mode.source", "focus")
        app.state(lambda s: s["source_mode"] and s["preview"]["shown"])
        themes(app, "generated", controller)
        if not baseline:
            before = app.state()["snapshot"]
            app.type("must remain read only")
            record("generated-source-stays-read-only", app.state()["snapshot"] == before)
    finally:
        app.kill()


try:
    source_case()
    latex_case()
    generated_case()
finally:
    (output / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
