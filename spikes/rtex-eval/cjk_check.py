#!/usr/bin/env python3
"""E3 preparation: are luatexja/ctex usable at run time, and do the CJK fixtures compile?

Not the E3 verdict (live-unit share, p50/p95 and per-glyph `rtex verify` belong to the next
round); it records availability, a plain LuaLaTeX compile and one preliminary `rtex verify`.
"""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
PACKAGES = ("luatexja.sty", "ctex.sty", "ctexart.cls", "FandolSong-Regular.otf")


def run(command, cwd, env, log, seconds=300):
    with log.open("w") as out:
        out.write("$ " + " ".join(command) + "\n")
        out.flush()
        status = subprocess.run(command, cwd=cwd, env=env, stdout=out, stderr=subprocess.STDOUT,
                                timeout=seconds).returncode
        out.write(f"\nexit_status: {status}\n")
    return status


def check(name, clone, work, evidence, env):
    project = work / name
    project.mkdir(parents=True)
    shutil.copyfile(HERE / f"fixtures/cjk/{name}.tex", project / "main.tex")
    plain = project / "plain"
    plain.mkdir()
    status = run(["lualatex", "-interaction=nonstopmode", "-halt-on-error", f"-output-directory={plain}",
                  str(project / "main.tex")], project, env, evidence / f"e3-{name}-lualatex.log")
    log = (plain / "main.log").read_text(errors="replace") if (plain / "main.log").exists() else ""
    text = subprocess.run(["pdftotext", str(plain / "main.pdf"), "-"], capture_output=True,
                          text=True).stdout if (plain / "main.pdf").exists() else ""
    verify = run([str(clone / "target/release/rtex"), "verify", "--project", str(project), "--build",
                  str(project / "verify"), "--json-out", str(evidence / f"e3-{name}-verify.json")],
                 project, env, evidence / f"e3-{name}-verify.log")
    return {"fixture": name, "lualatex_exit": status, "missing_characters": log.count("Missing character"),
            "cjk_in_pdf_text": "中文段落" in text, "rtex_verify_exit": verify}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--clone", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    clone, evidence = args.clone.resolve(), args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=True)
    work = HERE / ".work" / "e3"
    work.mkdir(parents=True, exist_ok=False)
    env = dict(os.environ, RTEX_TEXLIVE_BIN="/usr/bin", RTEX_TEXDIR=str(clone / "tex"))
    found = {name: subprocess.run(["kpsewhich", name], capture_output=True, text=True).stdout.strip()
             for name in PACKAGES}
    owners = {name: subprocess.run(["pacman", "-Qo", path], capture_output=True, text=True).stdout.strip()
              for name, path in found.items() if path}
    summary = {"kpsewhich": found, "owners": owners, "available": all(found.values()), "fixtures": []}
    if summary["available"]:
        summary["fixtures"] = [check(name, clone, work, evidence, env) for name in ("ctex", "luatexja")]
    (evidence / "e3-summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps(summary, ensure_ascii=False))
    return 0 if summary["available"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
