#!/usr/bin/env python3
"""E7 non-LuaLaTeX fallback ("整编预览"): full-compile times in the existing `full` profile.

Fixtures from gen_fixtures: the ctex article compiled by XeLaTeX and the English article
compiled by pdfLaTeX, at about 1, 10 and 50 pages. Per fixture:

- cold: fresh output directory, first run (no aux), then the second run that resolves
  references (what opening a project costs);
- warm: three times, insert one word into a paragraph and compile once with the aux files of the
  previous run (what one preview refresh after an edit costs).

Times are wall clock of the whole sandbox invocation; the cost of an empty sandbox start is
measured separately. The `full` profile mounts xelatex but not pdflatex, so the trusted
`SCHOLIUM_SPIKE_TOOLS` directory supplies a copy of the system pdfTeX binary behind a small
wrapper that points kpathsea at the system texmf.cnf (no profile change).
"""

import argparse
import json
from pathlib import Path
import re
import shutil
import statistics
import subprocess

from gen_fixtures import write as generate
from measure import HERE, run_sandboxed

PAGES = (1, 10, 50)
WARM_RUNS = 3


WRAPPER = """#!/bin/bash
# Trusted evaluation wrapper (E7 only): kpathsea derives texmf.cnf and TEXMFROOT from its own
# location, which is /toolchain here, so point both at the system TeX Live explicitly.
export TEXMFCNF=/usr/share/texmf-dist/web2c TEXMFROOT=/usr/share
exec -a /toolchain/pdflatex /toolchain/pdflatex-bin "$@"
"""


def toolchain(work):
    tools = work / "toolchain"
    tools.mkdir()
    shutil.copy2("/usr/bin/pdftex", tools / "pdflatex-bin")
    (tools / "pdflatex").write_text(WRAPPER)
    (tools / "pdflatex").chmod(0o755)
    return tools


def compile_once(args, engine, project, out, log):
    binary = "/usr/bin/xelatex" if engine == "xelatex" else "/toolchain/pdflatex"
    extra = {} if engine == "xelatex" else {"SCHOLIUM_SPIKE_TOOLS": str(args.tools)}
    status, wall = run_sandboxed(project, out, 300, [binary, "-interaction=nonstopmode", "-halt-on-error",
                                 "-output-directory=/work", "/project/main.tex"], profile="full",
                                 extra_env=extra, log=log)
    text = (out / "main.log").read_text(errors="replace") if (out / "main.log").exists() else ""
    return {"exit": status, "wall_s": wall, "rerun_requested": "Rerun to get" in text or "Label(s) may have changed" in text,
            "errors": text.count("\n! ")}


def pages_of(pdf):
    if not pdf.exists():
        return None
    info = subprocess.run(["pdfinfo", str(pdf)], capture_output=True, text=True).stdout
    match = re.search(r"Pages:\s+(\d+)", info)
    return int(match.group(1)) if match else None


def edit_word(project, k):
    """Insert one word into the paragraph that carries marker QZab (a mid-document edit)."""
    path = project / "main.tex"
    text = path.read_text()
    path.write_text(text.replace("QZab", f"QZab edit{k}", 1))


def measure(args, engine, pages):
    kind = "mixed" if engine == "xelatex" else "pdflatex"
    name = f"e7-{engine}-{pages}"
    project = generate(kind, pages, args.work / name / "project").parent
    out = args.work / name / "out"
    logs = args.evidence / name
    logs.mkdir(exist_ok=True)
    first = compile_once(args, engine, project, out, logs / "cold-1.log")
    second = compile_once(args, engine, project, out, logs / "cold-2.log")
    result = {"engine": engine, "requested_pages": pages, "pages": pages_of(out / "main.pdf"),
              "cold_first": first, "cold_second": second,
              "cold_open_s": round(first["wall_s"] + second["wall_s"], 3), "warm": []}
    for k in range(1, WARM_RUNS + 1):
        edit_word(project, k)
        result["warm"].append(compile_once(args, engine, project, out, logs / f"warm-{k}.log"))
    result["warm_median_s"] = statistics.median(r["wall_s"] for r in result["warm"])
    result["edit_in_pdf"] = f"edit{WARM_RUNS}" in subprocess.run(
        ["pdftotext", str(out / "main.pdf"), "-"], capture_output=True, text=True).stdout
    return result


def sandbox_overhead(args):
    project = args.work / "empty"
    project.mkdir()
    walls = [run_sandboxed(project, args.work / f"overhead-{i}", 60, ["/usr/bin/kpsewhich", "--version"],
                           profile="full")[1] for i in range(3)]
    return {"command": "kpsewhich --version", "wall_s": walls, "median_s": statistics.median(walls)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--run-id", default="e7")
    args = parser.parse_args()
    args.evidence = args.evidence.resolve()
    args.evidence.mkdir(parents=True, exist_ok=True)
    args.work = HERE / ".work" / args.run_id
    args.work.mkdir(parents=True, exist_ok=False)
    args.tools = toolchain(args.work)
    summary = {"sandbox_overhead": sandbox_overhead(args), "runs": []}
    for engine in ("xelatex", "pdflatex"):
        for pages in PAGES:
            summary["runs"].append(measure(args, engine, pages))
            print(json.dumps(summary["runs"][-1]), flush=True)
    (args.evidence / "e7-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
