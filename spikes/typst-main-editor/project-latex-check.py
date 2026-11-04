#!/usr/bin/env python3
"""LaTeX project workbench: declared entry, durable drafts, file commands and conflict UI."""
from pathlib import Path
import json
import sys
from project_ui import ProjectApp

binary, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
root = output / "latex"
root.mkdir()
entry = root / "paper.tex"
original = "\\documentclass{article}\n\\begin{document}LaTeX\\end{document}\n"
entry.write_text(original)
app = ProjectApp(binary, output, entry)
for key in ("SCHOLIUM_RTEX_BIN", "SCHOLIUM_RTEX_TEXDIR", "SCHOLIUM_LATEX_NATIVE_WINDOW"):
    app.env.pop(key, None)

# This persistence check deliberately keeps compilation unavailable, even with a local bundle.
app.env["SCHOLIUM_RTEX_BIN"] = str(output / "missing-rtex")
app.env["SCHOLIUM_RTEX_TEXDIR"] = str(output / "missing-tex")


def workbench(entry_name="paper.tex"):
    return app.state(lambda s: s.get("kind") == "latex" and s["entry"] == entry_name
                     and s["toolchain_unavailable"] and s["source_rect"] and s["preview_rect"])


def edit(text):
    s = workbench()
    x, y, _, _ = s["source_rect"]
    app.xdo("mousemove", "--window", app.window, round(x + 40), round(y + 50))
    app.xdo("click", 1)
    app.key("ctrl+a")
    app.type(text)  # Plain text avoids completion; invalid TeX is still a durable draft.
    return app.source(lambda s: s["text"] == text and s["dirty"])


def open_path(path):
    app.key("ctrl+o")
    app.control("path")
    app.type(str(path))
    app.control("submit")


try:
    app.start("open-file")
    assert app.source(lambda s: s["language"] == "Latex" and s["entry"] == "paper.tex")["text"] == original
    assert workbench()["pages"] == 0
    assert all(s.get("kind") == "latex" for s in app.entries("AUDIT "))
    app.capture("workbench")
    edit("saved source")
    assert entry.read_text() == original, "edit must save a draft without materializing"
    app.key("ctrl+s")
    app.source(lambda s: not s["dirty"] and s["text"] == "saved source")
    assert entry.read_text() == "saved source"
    edit("draft survives SIGKILL")
    app.kill()  # SIGKILL, without app close/save handlers.
    assert entry.read_text() == "saved source"
    # Add a misleading conventional entry: the declaration must still select paper.tex.
    (root / "main.tex").write_text("wrong entry")
    app.env["SCHOLIUM_PROJECT_PATH"] = str(root)
    app.start("reopen-folder")
    assert app.source(lambda s: s["dirty"])["text"] == "draft survives SIGKILL"
    workbench()
    assert entry.read_text() == "saved source"
    app.capture("recovered-draft")
    app.key("ctrl+s")
    app.source(lambda s: not s["dirty"])
    assert entry.read_text() == "draft survives SIGKILL"
    edit("local conflict draft")
    entry.write_text("external writer")
    app.key("ctrl+s")
    app.source(lambda s: s["error"] and s["dirty"])
    assert entry.read_text() == "external writer"
    app.capture("external-conflict")
    app.control("reload")
    app.source(lambda s: s["text"] == "external writer" and not s["dirty"])
    app.create(output / "new-latex", "latex")
    workbench("main.tex")
    app.source(lambda s: s["entry"] == "main.tex")
    app.capture("new-workbench")
    open_path(entry)
    app.source(lambda s: s["entry"] == "paper.tex" and s["text"] == "external writer")
    workbench()
    app.create(output / "another-latex", "latex")
    workbench("main.tex")
    open_path(root)
    app.source(lambda s: s["entry"] == "paper.tex" and s["text"] == "external writer")
    workbench()
    (output / "summary.json").write_text(json.dumps({
        "latex_workbench": True, "toolchain_unavailable": True, "declared_entry": "paper.tex",
        "edit_draft_without_materialize": True, "save_materialized": True,
        "sigkill_draft_recovered": True, "external_conflict_reload": True,
        "new_latex_workbench": True, "file_open_tex_and_folder": True,
        "typst_worker": False}, indent=2) + "\n")
    print("PASS LaTeX workbench: New/Open file/folder, Save, SIGKILL draft recovery, conflict reload")
finally:
    app.kill()
