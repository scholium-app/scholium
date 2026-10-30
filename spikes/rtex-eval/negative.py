#!/usr/bin/env python3
"""E2 negative controls: every probe runs unsandboxed first (it must work there), then sandboxed.

Configurations: `default` is rtex as shipped (LuaTeX restricted shell escape, no LuaSocket);
`lifted` runs the same rtex through lift/lualatex.c (`--shell-escape`), because LuaTeX already
refuses the shell and network probes by default and a probe that fails unsandboxed proves nothing.
"""

import argparse
import json
from pathlib import Path
import subprocess
import time

import canary
from common import HERE, stray_processes, summarize, wait_quietly, write_project
from runaway import runaway
from session import launch

CONTROLS = (("a", "a-read.tex", ("default",)), ("b", "b-input.tex", ("default",)),
            ("c", "c-shell.tex", ("default", "lifted")), ("d-lua", "d-write.tex", ("default",)),
            ("d-openout", "d-openout.tex", ("default",)), ("e", "e-network.tex", ("default", "lifted")))


def build_launcher(work):
    texbin = work / "texbin"
    texbin.mkdir()
    subprocess.run(["cc", "-O2", "-Wall", "-Werror", "-o", str(texbin / "lualatex"),
                    str(HERE / "lift/lualatex.c")], check=True)
    return texbin


def drive(session, text, sandbox, build):
    """Initial layout, one edit inside the first probe paragraph, then a PDF export."""
    steps = {"layout": wait_quietly(session, lambda e: e.get("event") == "LayoutUpdate", 60)}
    offset = len(text[:text.index("Probe") + len("Probe")].encode())
    session.send({"cmd": "edit", "start": offset, "end": offset, "text": " E2EDIT"})
    steps["edit"] = wait_quietly(session, lambda e: e.get("reply") in ("edit", "error"), 20)
    steps["after_edit"] = wait_quietly(session, lambda e: e.get("event") in (
        "ParagraphUpdate", "LayoutUpdate", "BackgroundScheduled"), 30)
    session.send({"cmd": "export_pdf", "out": "/work/export.pdf" if sandbox else str(build / "export.pdf")})
    steps["export"] = wait_quietly(session, lambda e: e.get("event") == "PdfExported", 60)
    return {key: summarize(value) for key, value in steps.items()}


def run_one(args, canaries, control, body, config, sandbox):
    mode = "sandbox" if sandbox else "host"
    name = f"negative-{control}-{config}-{mode}"
    run_dir = args.work / name
    project = run_dir / "project"
    text = write_project(project, body, canaries.placeholders(project))
    canaries.reset(project)
    build = run_dir / "build-dir"
    started = time.monotonic()
    session = launch(args.clone, project, build, args.evidence, name, sandbox,
                     extra_env=canaries.env(), texbin=args.texbin if config == "lifted" else None)
    try:
        steps = drive(session, text, sandbox, build)
    finally:
        session.close()
    time.sleep(1)
    found = canary.channels(build, build / "export.pdf", args.evidence, name)
    result = {"name": name, "control": control, "config": config, "mode": mode,
              "elapsed_seconds": round(time.monotonic() - started, 1), "steps": steps,
              "probe_output": canary.probe_lines(found["pdf_text"]),
              "leaks": canary.leaks(found, canaries.secrets), **canaries.side_effects(project),
              "stray_processes": stray_processes([str(run_dir)] + (["/rtex/bin/rtex"] if sandbox else []))}
    result["attack_effective"] = bool(result["leaks"] or result["files_outside_build"]
                                      or result["listener_connections"])
    (args.evidence / f"{name}.pdf.txt").write_text(found["pdf_text"])
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--clone", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--run-id", default="negative")
    parser.add_argument("--only", default="abcdef", help="subset of control letters")
    args = parser.parse_args()
    args.clone = args.clone.resolve()
    args.evidence = args.evidence.resolve()
    args.evidence.mkdir(parents=True, exist_ok=True)
    args.work = HERE / ".work" / args.run_id
    args.work.mkdir(parents=True, exist_ok=False)
    args.texbin = build_launcher(args.work)
    canaries = canary.Canaries()
    results = []
    try:
        for control, body, configs in CONTROLS:
            if control[0] not in args.only:
                continue
            for config in configs:
                pair = [run_one(args, canaries, control, body, config, sandbox) for sandbox in (False, True)]
                results.append(verdict(control, config, pair))
                print(json.dumps(results[-1]["verdict"]), flush=True)
        if "f" in args.only:
            results.append(verdict("f", "default", [runaway(args, sandbox) for sandbox in (False, True)]))
            print(json.dumps(results[-1]["verdict"]), flush=True)
    finally:
        canaries.close()
    (args.evidence / "negative-summary.json").write_text(json.dumps(results, indent=2) + "\n")
    return 0 if all(r["verdict"]["result"] != "Fail" for r in results) else 1


def verdict(control, config, pair):
    host, sandbox = pair
    if not host["attack_effective"]:
        result = "Inconclusive (probe blocked unsandboxed)"
    elif sandbox["attack_effective"] or sandbox["stray_processes"]:
        result = "Fail"
    else:
        result = "Pass"
    return {"verdict": {"control": control, "config": config, "result": result,
                        "host_effective": host["attack_effective"],
                        "sandbox_effective": sandbox["attack_effective"]},
            "runs": pair}


if __name__ == "__main__":
    raise SystemExit(main())
