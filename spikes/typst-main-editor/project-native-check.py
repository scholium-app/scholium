#!/usr/bin/env python3
"""Create Typst project, edit, checkpoint, SIGKILL, and reopen the same authority."""
import json
from pathlib import Path
import sys
from project_ui import ProjectApp

binary, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
app = ProjectApp(binary, output, output / "initial")
project = output / "created"
def create_project(path, language="latex"):
    app.key("ctrl+n")
    app.control("name")
    app.key("ctrl+a")
    app.type(path.name)
    app.control("path")
    app.key("ctrl+a")
    app.type(str(path.parent))
    app.control(language)
    app.control("submit")


try:
    app.start("create")
    before = app.state()["snapshot"]["document"]
    create_project(project, "typst")
    state = app.state(lambda s: s["current"] and s["snapshot"]["document"] != before)
    caret = next(c for c in state["carets"] if c["leaf"] == state["cursor_leaf"] and c["byte"] == 0)
    app.xdo("mousemove", "--window", app.window, round(caret["x"]), round(caret["y"]))
    app.xdo("click", 1)
    app.type("T39 durable native")
    state = app.state(lambda s: s["current"] and "T39 durable native" in json.dumps(s["snapshot"]))
    app.key("ctrl+s")
    app.state(lambda s: s["current"] and s["saved"])
    app.capture("saved")
    expected = state["snapshot"]
    app.kill()
    app.env["SCHOLIUM_PROJECT_PATH"] = str(project)
    app.start("reopen")
    assert app.state()["snapshot"] == expected
    app.capture("reopened")
    (output / "summary.json").write_text(json.dumps({"saved_sigkill_reopen_equal": True, "project": str(project)}, indent=2))
    print("PASS new Typst project → edit → save → SIGKILL → reopen")
finally:
    app.kill()
