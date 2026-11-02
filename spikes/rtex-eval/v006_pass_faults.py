#!/usr/bin/env python3
"""Configure upstream pass_timeout in an isolated evaluation process (CLI lacks the flag)."""
import argparse
import json
from pathlib import Path
import time

from common import write_project
from e8_faults import LOOP, send_edit
from faults import drain, kill, live_engine, processes
from measure import warm_font_cache
from session import launch
from v006_contracts import e8_verdict


def run(args, kind):
    name = f"e8-pass-{kind}"
    project = args.work / name / "project"
    text = write_project(project, "f-runaway.tex", {})
    cache = args.work / name / "cache"
    warm = warm_font_cache(cache, project, args.work / name, args.clone)
    if warm["exit"]:
        raise RuntimeError(f"warm-up: {warm}")
    session = launch(args.clone, project, args.work / name / "session", args.evidence, name, True,
                     binary=args.binary, serve_args=("--pass-timeout-ms", "8000"), seconds=180,
                     redact=True, extra_env={"SCHOLIUM_RTEX_FONT_CACHE": str(cache)})
    result = {"scenario": "background kill" if kind == "kill" else "upstream timeout",
              "pass_timeout_ms": 8000, "host_watchdog": False, "killed": [], "recovered": False}
    try:
        layout = session.wait(lambda e: e.get("event") == "LayoutUpdate", 90)
        while layout["convergence"]["state"] != "Converged":
            layout = session.wait(lambda e: e.get("event") == "LayoutUpdate", 30)
        at = len(text[:text.index("LOOPHERE")].encode())
        other_at = len(text[:text.index("untouched")].encode())
        _, start = send_edit(session, at, at, LOOP)
        time.sleep(0.5)
        other, other_sent = send_edit(session, other_at, other_at, "x")
        events = drain(session, 6.5)
        if kind == "kill":
            live = {p["pid"] for p in live_engine(session.process.pid)}
            candidates = [p for p in processes(session.process.pid)
                          if p["comm"].startswith("lua") and p["pid"] not in live and p["cpu"] > 1]
            result["killed"] = [p for p in candidates if kill(p["pid"])]
        events += drain(session, 18, until=lambda e: e.get("event") == "LayoutUpdate"
                        and e.get("compile", {}).get("state") == "Failed")
        result["layouts"] = [e for e in events if e.get("event") == "LayoutUpdate"]
        result["timeout_seen"] = any("timed out" in json.dumps(e).lower() or "timeout" in json.dumps(e).lower() or "time limit" in json.dumps(e).lower()
                                     for e in events)
        result["events"] = events
        result["other_unit_update"] = next((e for e in events if e.get("edit_id") == other.get("edit_id")
                                            and e.get("event") == "ParagraphUpdate"), None)
        result["other_sent_after_loop_s"] = round(other_sent - start, 3)
        send_edit(session, other_at, other_at + 1, "")
        send_edit(session, at, at + len(LOOP), "")
        fixed = time.monotonic()
        session.send({"cmd": "request_layout"})
        restored = session.wait(lambda e: e.get("event") == "LayoutUpdate"
                                and e.get("compile", {}).get("state") == "Ok"
                                and e.get("convergence", {}).get("state") == "Converged", 60)
        result["recovered"] = bool(restored)
        result["recovered_after_fix_s"] = round(session.received_at - fixed, 3)
    except Exception as error:
        result["error"] = repr(error)
    finally:
        session.close()
    result["verdict"] = e8_verdict(result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--clone", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    args = parser.parse_args()
    for attr in ("clone", "binary", "evidence", "work"):
        setattr(args, attr, getattr(args, attr).resolve())
    args.work.mkdir(parents=True, exist_ok=False)
    args.evidence.mkdir(parents=True, exist_ok=True)
    rows = []
    for kind in ("kill", "timeout"):
        rows.append(run(args, kind))
        print(json.dumps(rows[-1]), flush=True)
        (args.evidence / "pass-faults.json").write_text(json.dumps(rows, indent=2) + "\n")
    return 0 if all(row["verdict"]["pass"] for row in rows) else 1


if __name__ == "__main__":
    raise SystemExit(main())
