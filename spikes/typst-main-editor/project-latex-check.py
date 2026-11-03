#!/usr/bin/env python3
"""Open a plain .tex project in the narrow placeholder; save/reopen without a Typst worker."""
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
try:
    app.start("open")
    state = app.source(lambda s: s["language"] == "Latex" and s["entry"] == "paper.tex")
    assert state["text"] == original
    assert not app.entries("AUDIT "), "LaTeX must not assemble the Typst page editor"
    app.capture("placeholder")
    app.key("ctrl+s")
    app.source(lambda s: not s["dirty"])
    app.kill()
    app.start("reopen")
    assert app.source()["text"] == original
    assert entry.read_text() == original
    (output / "summary.json").write_text(json.dumps({"latex_placeholder": True, "typst_worker": False,
                                                     "save_reopen_equal": True}, indent=2))
    print("PASS .tex placeholder → save → reopen; no Typst workspace")
finally:
    app.kill()
