#!/usr/bin/env python3
"""Shared source find/replace: real keymap chords, match stepping, one-step undo and Esc focus."""
import json
import os
from pathlib import Path
import subprocess
import sys
from project_ui import ProjectApp
from chrome_ui import keycode

binary, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
project = output / "paper"
project.mkdir()
initial = "alpha 中 alpha\nalpha\n"
entry = project / "main.typ"
entry.write_text(initial)
app = ProjectApp(binary, output, entry)
app.start("find")


def find(predicate):
    return app.wait(lambda: next((s for s in app.entries("SOURCE_FIND ")[-1:] if predicate(s)), None))


def paste(text):
    subprocess.run(["xclip", "-selection", "clipboard"], input=text.encode(), env=app.env, check=True)
    app.key("ctrl+v")


try:
    app.source(lambda s: s["text"] == initial)
    app.key("ctrl+f")
    app.control("source.find.query")
    paste("alpha")
    first = find(lambda s: s["query"] == "alpha" and s["matches"] == 3 and not s["scanning"])
    app.key(str(keycode(app.env["DISPLAY"], "F3")))
    second = find(lambda s: s["current"] == (first["current"] + 1) % 3)
    app.key("shift+" + str(keycode(app.env["DISPLAY"], "F3")))
    find(lambda s: s["current"] == first["current"])
    app.key("ctrl+h")
    app.control("source.find.replacement")
    paste("beta")
    app.control("source.find.replace_all")
    app.source(lambda s: s["text"] == initial.replace("alpha", "beta"))
    app.key("ctrl+z")
    app.source(lambda s: s["text"] == initial)
    app.capture("source-find")
    app.key("Escape")
    app.type("X")
    restored = app.source(lambda s: s["text"].count("X") == 1)
    (output / "result.json").write_text(json.dumps({"pass": True, "matches": 3, "stepped": second["current"], "undo_restored_all": True, "escape_returns_editor_focus": True}, indent=2))
    print("PASS source find, next/previous, replace all, single undo, Esc focus", flush=True)
finally:
    app.kill()
