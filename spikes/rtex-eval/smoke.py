#!/usr/bin/env python3
"""Run the frozen upstream smoke gate without modifying upstream sources."""

import argparse
import json
import os
from pathlib import Path
import shlex
import signal
import subprocess
import time


def run(command, env, cwd, log):
    started = time.monotonic()
    with log.open("w") as output:
        output.write(f"$ {shlex.join(command)}\n")
        output.write(f"cwd: {cwd}\n")
        for key in ("RTEX_TEXLIVE_BIN", "RTEX_TEXDIR"):
            output.write(f"{key}={env[key]}\n")
        output.flush()
        process = subprocess.Popen(command, cwd=cwd, env=env, stdout=output,
                                   stderr=subprocess.STDOUT, start_new_session=True)
        try:
            status = process.wait(timeout=180)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
            status = 124
        output.write(f"\nexit_status: {status}\n")
    return {"command": command, "cwd": str(cwd), "exit_status": status,
            "elapsed_seconds": round(time.monotonic() - started, 3),
            "log": log.name}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--clone", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    clone = args.clone.resolve()
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=True)
    work = Path(__file__).resolve().parent / ".work" / "smoke"
    work.mkdir(parents=True, exist_ok=False)
    binary = str(clone / "target/release/rtex")
    env = dict(os.environ, RTEX_TEXLIVE_BIN="/usr/bin", RTEX_TEXDIR=str(clone / "tex"))
    project = str(work / "book-10")
    commands = [
        ("doctor", [binary, "doctor"]),
        ("gen-book", [binary, "gen-book", "--pages", "10", "--out", project]),
        ("edit", [binary, "edit", "--project", project, "--find", "The", "--text", " edited"]),
        ("verify", [binary, "verify", "--project", project, "--build", str(work / "verify"),
                    "--json-out", str(evidence / "verify-result.json")]),
    ]
    results = []
    for name, command in commands:
        result = run(command, env, work, evidence / f"smoke-{name}.log")
        results.append(result)
        print(json.dumps(result), flush=True)
        # Upstream edit can exit zero even when no paragraph update arrived.
        if name == "edit" and result["exit_status"] == 0:
            result["paragraph_update"] = "paragraph ParaId(" in (evidence / "smoke-edit.log").read_text()
        if result["exit_status"] or result.get("paragraph_update") is False:
            break
    passed = len(results) == len(commands) and results[-1]["exit_status"] == 0
    (evidence / "smoke-summary.json").write_text(json.dumps(
        {"passed": passed, "results": results}, indent=2) + "\n")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
