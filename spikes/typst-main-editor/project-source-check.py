#!/usr/bin/env python3
"""Source draft survives SIGKILL; external bytes survive Save; reload resolves conflict."""
from pathlib import Path
import json
import sys
from project_ui import ProjectApp

binary, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
root = output / "source"
root.mkdir()
entry = root / "main.typ"
entry.write_text("Original source")
app = ProjectApp(binary, output, entry)
try:
    app.start("edit")
    app.source(lambda s: s["language"] == "Typst")
    app.control("source")
    app.key("ctrl+a")
    app.type("Private draft")
    app.source(lambda s: s["text"] == "Private draft" and s["dirty"])
    app.kill()
    app.start("reopen")
    app.source(lambda s: s["text"] == "Private draft" and s["dirty"])
    entry.write_text("External writer")
    app.key("ctrl+s")
    conflict = app.source(lambda s: bool(s["error"]))
    assert "外部已修改" in conflict["error"] or "Modified externally" in conflict["error"]
    assert entry.read_text() == "External writer"
    app.capture("conflict")
    app.control("reload")
    app.source(lambda s: s["text"] == "External writer" and not s["dirty"])
    app.key("ctrl+s")
    app.kill()
    app.start("saved-reopen")
    app.source(lambda s: s["text"] == "External writer" and not s["dirty"])
    (output / "summary.json").write_text(json.dumps({"draft_recovered": True, "conflict_visible": True,
                                                     "external_preserved": True, "reload_saved_reopened": True}, indent=2))
    print("PASS Typst source draft recovery and visible external conflict without overwrite")
finally:
    app.kill()
