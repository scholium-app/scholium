#!/usr/bin/env python3
"""Shared diagnostic batches: real compiler errors, row selection, keymap cycling and clears."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from project_ui import ProjectApp
from chrome_ui import Chrome
import chrome_ui

binary, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
results = {}


def record(name, condition, **facts):
    results[name] = {"pass": bool(condition), **facts}
    print(("PASS " if condition else "FAIL ") + name, flush=True)
    assert condition, (name, facts)


def latest(app, prefix, predicate):
    return app.wait(lambda: next((s for s in app.entries(prefix)[-1:] if predicate(s)), None))


def clipboard(app, text):
    subprocess.run(["xclip", "-selection", "clipboard"], input=text.encode(), env=app.env, check=True)
    app.key("ctrl+v")


def rows(app):
    return app.source()["controls"].get("ui.diagnostics.row", [])


def capture_themes(app, name):
    controller = Chrome(lambda: app.source(), app.xdo, app.window)
    for theme in ["light", "dark"]:
        controller.menu("ui.menu.view", "ui.menu.view.theme", "view.theme." + theme)
        time.sleep(0.3)
        app.capture(name + "-" + theme)


def case(kind, source, fixed, member=None):
    folder = output / kind
    folder.mkdir()
    project = folder / "paper"
    project.mkdir()
    entry = project / ("main.tex" if kind.startswith("latex") else "main.typ")
    entry.write_text(source)
    if member is not None:
        (project / "chapter.tex").write_text(member)
    resource = "chapter.tex" if member else entry.name
    app = ProjectApp(binary, folder, entry)
    app.start(kind)
    try:
        app.source()
        prefix = "AUDIT " if kind.startswith("latex") else "SOURCE_PREVIEW "
        def diagnostic(predicate):
            return latest(app, prefix, lambda s: (not kind.startswith("latex") or s.get("kind") == "latex") and "diagnostics" in s and predicate(s))
        s = diagnostic(lambda s: bool(s["diagnostics"]["items"]))
        app.key("ctrl+j")
        app.source(lambda s: bool(s["controls"].get("ui.diagnostics.row")))
        issues = s["diagnostics"]["items"]
        record(kind + "-compiler-rows", all(d["resource"] in (None, resource) for d in issues), diagnostics=issues)
        if kind.startswith("latex"):
            record(kind + "-error-and-citation", all(any(message in d["message"] and d["resource"] == resource and d["range"] for d in issues) for message in ["Undefined control sequence", "Citation"]))
        # Opening puts the keyboard on the first problem. Enter selects its source range.
        time.sleep(0.3)
        app.key("Return")
        selected = diagnostic(lambda s: bool(s["selection"]))
        record(kind + "-enter-selects-range", selected["selection"]["end"] > selected["selection"]["start"], selection=selected["selection"])
        if member is not None:
            record(kind + "-opens-member-without-changing-entry", selected["active_resource"] == "chapter.tex" and app.source()["entry"] == "main.tex")
        capture_themes(app, "problems")
        # F8 follows the loaded keymap and wraps in both directions.
        before = selected["selection"]
        app.key(str(chrome_ui.keycode(app.env["DISPLAY"], "F8")))
        if len(issues) > 1:
            diagnostic(lambda s: s["selection"] and s["selection"] != before)
        app.key("shift+" + str(chrome_ui.keycode(app.env["DISPLAY"], "F8")))
        diagnostic(lambda s: s["selection"] == before)
        record(kind + "-f8-and-shift-f8-wrap", True)
        controller = Chrome(lambda: app.source(), app.xdo, app.window)
        controller.click("ui.problems.output", "problem-header")
        app.source(lambda s: not s["controls"].get("ui.diagnostics.row"))
        app.capture("build-output")
        controller.click("ui.problems.tab", "problem-header")
        app.source(lambda s: bool(s["controls"].get("ui.diagnostics.row")))
        # Whole groups collapse, with no orphaned rows.
        count = len(rows(app))
        controller.click("ui.problems.group", "problem-header")
        app.source(lambda s: len(s["controls"].get("ui.diagnostics.row", [])) < count)
        controller.click("ui.problems.group", "problem-header")
        app.source(lambda s: bool(s["controls"].get("ui.diagnostics.row")))
        record(kind + "-tabs-and-group-collapse", True)
        app.control("source")
        app.key("ctrl+a")
        clipboard(app, fixed)
        diagnostic(lambda s: s["revision"] > selected["revision"] and not s["diagnostics"]["items"] and s["shown_revision"] == s["revision"])
        app.source(lambda s: not s["controls"].get("ui.diagnostics.row"))
        record(kind + "-good-build-clears-problems", True)
    finally:
        app.kill()


try:
    # Full compilation works without optional rtex in CI. Live is an explicit native extension.
    engine = "lualatex" if os.environ.get("SCHOLIUM_T47_LIVE") else "xelatex"
    latex = "% !TEX program = " + engine + "\n\\documentclass{article}\n\\begin{document}\n\\cite{missing}\n\\foo\n\\end{document}\n"
    fixed = "% !TEX program = " + engine + "\n\\documentclass{article}\n\\begin{document}\nFixed\n\\end{document}\n"
    # CI has no TeX installation. The native case must be explicitly reported, never faked.
    if os.environ.get("SCHOLIUM_T47_NATIVE"):
        case("latex-live" if engine == "lualatex" else "latex-full", latex, fixed)
        case("latex-member", "% !TEX program = " + engine + "\n\\documentclass{article}\n\\begin{document}\n\\input{chapter}\n\\end{document}\n", "Fixed\n", "\\cite{missing}\n\\foo\n")
    else:
        results["latex-native"] = {"skip": "requires TeX Live and product namespaces; run SCHOLIUM_T47_NATIVE=1"}
    case("typ-source", '= Source\n#unknown_function()', '= Source\nFixed')
finally:
    (output / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
