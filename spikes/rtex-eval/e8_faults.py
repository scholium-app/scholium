#!/usr/bin/env python3
"""E8 faults, all in the `rtex` sandbox: what the host observes, whether editing continues, and
how long recovery takes.

Scenarios: live engine killed (SIGKILL), live and background hang (round-1 runaway fixture) with
and without a host-side per-pass watchdog, preamble errors (missing package, undefined command),
malformed host commands, and `rtex serve` itself killed. Times are host monotonic seconds; event
timelines are rebuilt from the session transcripts (they keep every message with its receipt time).
"""

import argparse
import json
from pathlib import Path
import shutil
import time

from common import stray_processes, write_project
from faults import PassWatchdog, brief, drain, first, kill, live_engine, processes, rtex_server, timeline
from gen_fixtures import write as generate
from measure import CLONE_DEFAULT, HERE, body_units, edit_offset
from session import launch

LOOP = "\\directlua{while true do end}"


def open_session(args, name, project, seconds=400, cache=None):
    extra = {"SCHOLIUM_RTEX_FONT_CACHE": str(cache)} if cache else {}
    session = launch(args.clone, project, args.work / name, args.evidence, name, True, seconds=seconds,
                     redact=True, extra_env=extra)
    layout = session.wait(lambda e: e.get("event") == "LayoutUpdate", 120)
    first_layout_s = round(session.received_at - session.started_at, 3)
    while layout["convergence"]["state"] not in ("Converged", "PassLimitReached"):
        layout = session.wait(lambda e: e.get("event") == "LayoutUpdate", 120)
    return session, first_layout_s


def mixed_project(args, name):
    return generate("mixed", 1, args.work / "projects" / name).parent


def targets(session, project):
    session.send({"cmd": "spans", "path": "main.tex"})
    spans = session.wait(lambda e: e.get("reply") == "spans")["spans"]
    source = (project / "main.tex").read_text()
    found = [(u, edit_offset(source, u, cjk=True)) for u in body_units(spans)]
    return [(u, o) for u, o in found if o is not None]


def send_edit(session, start, end, text, seconds=10):
    """Writes one edit and waits only for its reply (immediate); returns (result, sent time)."""
    sent = session.send({"cmd": "edit", "path": "main.tex", "start": start, "end": end, "text": text})
    reply = session.wait(lambda e: e.get("reply") in ("edit", "error"), seconds)
    return {"reply": reply.get("reply"), **reply.get("result", {})}, sent


def keystroke(session, offset, text="字", seconds=10):
    """Insert then delete one character; reports the insert's own update and its latency.

    Waiting drops other messages from the queue, but the transcript keeps them; scenario
    timelines are rebuilt from the transcript afterwards.
    """
    result, sent = send_edit(session, offset, offset, text, seconds)
    out = {"routed": result.get("routed"), "edit_id": result.get("edit_id")}
    touched = result.get("outcome", {}).get("touched", [])
    try:
        update = session.wait(lambda e: (e.get("event") == "ParagraphUpdate" and e.get("edit_id") == out["edit_id"])
                              or (e.get("event") == "BackgroundScheduled" and e.get("par_id") in touched), seconds)
        out.update(update=update["event"], status=update.get("status"), reasons=update.get("reasons"),
                   latency_s=round(session.received_at - sent, 3))
    except TimeoutError:
        out["update"] = "timeout"
    send_edit(session, offset, offset + len(text.encode()), "", seconds)
    return out


def edit_until_ok(session, offset, since, limit=90):
    """Keep typing in one unit until a live update is ok again; seconds since `since`."""
    while time.monotonic() - since < limit:
        if keystroke(session, offset).get("status") == "ok":
            return round(session.received_at - since, 3)
        time.sleep(0.5)
    return None


def own_update(events, edit_id, touched):
    return first(events, lambda e: (e.get("event") == "ParagraphUpdate" and e.get("edit_id") == edit_id)
                 or (e.get("event") == "BackgroundScheduled" and e.get("par_id") in touched))


def crash(args):
    name = "e8-crash"
    project = mixed_project(args, "crash")
    session, _ = open_session(args, name, project)
    result = {"scenario": "live engine SIGKILL"}
    try:
        units = targets(session, project)
        result["baseline"] = keystroke(session, units[0][1])
        result["killed"] = [p["comm"] for p in live_engine(session.process.pid) if kill(p["pid"])]
        killed = time.monotonic()
        # the user keeps typing: this keystroke meets the dead engine
        after, _ = send_edit(session, units[1][1], units[1][1], "字")
        drain(session, 15)
        result["recovered_live_s"] = edit_until_ok(session, units[2][1], killed)
        session.send({"cmd": "request_layout"})
        result["layout_after"] = brief(session.wait(lambda e: e.get("event") == "LayoutUpdate", 120))
        result["sandbox_alive"] = session.process.poll() is None
    finally:
        session.close()
    events = timeline(args.evidence, name, killed)
    after_kill = [e for e in events if e["t"] >= 0]
    result["engine_states"] = [{"t": e["t"], "state": e["state"], "reason": e.get("reason")}
                               for e in after_kill if e.get("event") == "EngineState"]
    update = own_update(after_kill, after["edit_id"], after.get("outcome", {}).get("touched", []))
    result["edit_after_kill"] = {"edit_id": after["edit_id"], "routed": after.get("routed"),
                                 "own_update": update and {k: update.get(k) for k in ("t", "event", "status")}}
    ok = first(after_kill, lambda e: e.get("event") == "ParagraphUpdate" and e.get("status") == "ok")
    result["first_ok_update_after_kill_s"] = ok and ok["t"]
    return result


def hang(args, limit):
    """Round-1 runaway fixture; `limit` is the host watchdog's per-pass wall clock (None: off)."""
    tag = f"watchdog{limit}" if limit else "nowatchdog"
    name = f"e8-hang-{tag}"
    project = args.work / "projects" / f"hang-{tag}"
    text = write_project(project, "f-runaway.tex", {})
    session, _ = open_session(args, name, project, seconds=600)
    watchdog = PassWatchdog(session.process.pid, limit) if limit else None
    result = {"scenario": "live and background hang", "watchdog_s": limit}
    loop_at = len(text[:text.index("LOOPHERE")].encode())
    after_at = len(text[:text.index("AFTERHERE") + len("AFTERHERE")].encode())
    other_at = len(text[:text.index("untouched")].encode())
    try:
        if watchdog:
            watchdog.start()
        _, looped = send_edit(session, loop_at, loop_at, LOOP)
        time.sleep(0.5)
        other, other_sent = send_edit(session, other_at, other_at, "x")
        drain(session, 20, until=lambda e: e.get("event") == "BackgroundScheduled" or (
            e.get("event") == "ParagraphUpdate" and e.get("edit_id") == other["edit_id"]))
        time.sleep(3)  # the loop stays in the source until a background pass has taken it
        send_edit(session, other_at, other_at + 1, "")
        send_edit(session, loop_at, loop_at + len(LOOP), "")
        fixed = time.monotonic()
        follow, _ = send_edit(session, after_at, after_at, "x")
        drain(session, 150, until=lambda e: e.get("event") == "LayoutUpdate"
              and e["compile"]["state"] == "Ok" and e["convergence"]["state"] == "Converged")
        result["tex_processes_at_end"] = [round(p["cpu"], 2) for p in processes(session.process.pid)
                                          if p["comm"].startswith("lua")]
    finally:
        if watchdog:
            watchdog.stop()
            result["watchdog_kills"] = watchdog.kills
        session.close()
    events = timeline(args.evidence, name, looped)
    fix_t = round(fixed - looped, 3)
    result["loop_removed_at_s"] = fix_t
    update = own_update(events, other["edit_id"], other.get("outcome", {}).get("touched", []))
    result["other_unit_edit"] = update and {"latency_s": round(update["t"] - (other_sent - looped), 3),
                                            "event": update["event"], "reasons": update.get("reasons")}
    result["engine_states"] = [{"t": e["t"], "state": e["state"], "reason": (e.get("reason") or "")[:160]}
                               for e in events if e.get("event") == "EngineState"]
    result["follow_up_edit"] = {"routed": follow.get("routed"), "reasons": follow.get("reasons")}
    result["layouts"] = [{"t": e["t"], "t_after_fix": round(e["t"] - fix_t, 2), "compile": e["compile"],
                          "convergence": e["convergence"]["state"], "wall_ms": e.get("wall_ms"),
                          "source_revision": e.get("source_revision")}
                         for e in events if e.get("event") == "LayoutUpdate" and e["t"] >= 0]
    result["diagnostics"] = [e for e in events if e.get("event") == "Diagnostics" and e["t"] >= 0]
    done = first(events, lambda e: e.get("event") == "LayoutUpdate" and e["compile"]["state"] == "Ok"
                 and e["convergence"]["state"] == "Converged", after=fix_t)
    result["layout_recovered_after_fix_s"] = done and round(done["t"] - fix_t, 2)
    return result


def preamble(args, kind):
    insert = {"missing-package": "\\usepackage{scholiumnosuchpkg}\n",
              "undefined-command": "\\scholiumundefinedmacro\n"}[kind]
    name = f"e8-preamble-{kind}"
    project = mixed_project(args, f"preamble-{kind}")
    session, _ = open_session(args, name, project)
    result = {"scenario": f"preamble error: {kind}"}
    try:
        units = targets(session, project)
        at = len((project / "main.tex").read_text().split("\\begin{document}")[0].encode())
        _, broke = send_edit(session, at, at, insert)
        drain(session, 40, until=lambda e: e.get("event") == "LayoutUpdate")
        shift = len(insert.encode())
        result["body_edit_while_broken"] = keystroke(session, units[0][1] + shift)
        drain(session, 8)
        _, fixed = send_edit(session, at, at + shift, "")
        drain(session, 60, until=lambda e: e.get("event") == "EngineState" and e.get("state") == "Ready")
        result["recovered_live_s"] = edit_until_ok(session, units[1][1], fixed)
        drain(session, 30, until=lambda e: e.get("event") == "LayoutUpdate" and e["convergence"]["state"] == "Converged")
    finally:
        session.close()
    events = timeline(args.evidence, name, broke)
    fix_t = round(fixed - broke, 3)
    result["fixed_at_s"] = fix_t
    result["engine_states"] = [{"t": e["t"], "state": e["state"], "reason": (e.get("reason") or "")[:120]}
                               for e in events if e.get("event") == "EngineState" and e["t"] >= 0]
    result["layouts"] = [{"t": e["t"], "compile": e["compile"], "convergence": e["convergence"]["state"],
                          "pages_total": e.get("pages_total")}
                         for e in events if e.get("event") == "LayoutUpdate" and e["t"] >= 0]
    result["diagnostics"] = [e.get("items") for e in events if e.get("event") == "Diagnostics" and e["t"] >= 0][:3]
    ok = first(events, lambda e: e.get("event") == "LayoutUpdate" and e["compile"]["state"] == "Ok", after=fix_t)
    result["layout_ok_after_fix_s"] = ok and round(ok["t"] - fix_t, 3)
    return result


def malformed(args):
    project = mixed_project(args, "malformed")
    session, _ = open_session(args, "e8-malformed", project)
    original = (project / "main.tex").read_text()
    size = len(original.encode())
    cjk = next(i for i, ch in enumerate(original) if "一" <= ch <= "鿿")
    mid = len(original[:cjk].encode()) + 1
    ascii_at = len(original[:original.index("QZaa")].encode())
    cases = [
        ("start>end", {"cmd": "edit", "path": "main.tex", "start": ascii_at + 2, "end": ascii_at, "text": "x"}),
        ("end beyond file", {"cmd": "edit", "path": "main.tex", "start": size - 3, "end": size + 1000, "text": ""}),
        ("start beyond file", {"cmd": "edit", "path": "main.tex", "start": size + 100, "end": size + 100, "text": "x"}),
        ("negative start", {"cmd": "edit", "path": "main.tex", "start": -5, "end": -5, "text": "x"}),
        ("unknown file", {"cmd": "edit", "path": "nope.tex", "start": 0, "end": 0, "text": "x"}),
        ("path traversal edit", {"cmd": "edit", "path": "../../etc/passwd", "start": 0, "end": 0, "text": "x"}),
        ("set_document outside project", {"cmd": "set_document", "path": "../../e8-escape.tex", "text": "escape\n"}),
        ("set_document absolute", {"cmd": "set_document", "path": "/tmp/e8-escape-abs.tex", "text": "escape\n"}),
        ("invalid json", "{not json"),
        ("unknown cmd", {"cmd": "format_disk"}),
        ("mid-codepoint", {"cmd": "edit", "path": "main.tex", "start": mid, "end": mid, "text": "x"}),
    ]
    rows = []
    try:
        for label, command in cases:
            rows.append(malformed_case(session, label, command, original))
            if rows[-1].get("alive") is False:
                break
        result = {"scenario": "malformed host commands", "cases": rows,
                  "written_outside_snapshot": sorted(str(p.relative_to(args.work)) for p in
                                                     (args.work / "e8-malformed").rglob("e8-escape*"))}
    finally:
        session.close()
        result["exit"] = session.process.returncode
    return result


def malformed_case(session, label, command, original):
    if isinstance(command, str):
        session.process.stdin.write((command + "\n").encode())
        session.process.stdin.flush()
    else:
        session.send(command)
    row = {"case": label, "command": command if isinstance(command, str) else
           {k: v for k, v in command.items() if k != "text" or len(v) < 20}}
    try:
        reply = session.wait(lambda e: "reply" in e, 5)
        row["reply"] = brief(reply)
        row["events"] = drain(session, 3)
        session.send({"cmd": "status"})
        session.wait(lambda e: e.get("reply") == "status", 5)
        row["alive"] = True
    except (TimeoutError, RuntimeError, BrokenPipeError) as error:
        row.update(alive=False, error=str(error)[:200], exit=session.process.wait(timeout=10))
        return row
    if row["reply"].get("reply") == "edit":  # restore the buffer for the next case
        session.send({"cmd": "set_document", "path": "main.tex", "text": original})
        session.wait(lambda e: e.get("reply") in ("set_document", "error"), 5)
        drain(session, 2)
    return row


def serve_killed(args):
    project = mixed_project(args, "killed")
    cache = args.work / "killed-font-cache"
    cache.mkdir()
    session, first = open_session(args, "e8-killed", project, cache=cache)
    result = {"scenario": "rtex serve SIGKILL", "first_layout_s": first}
    try:
        result["killed"] = [p["pid"] for p in rtex_server(session.process.pid) if kill(p["pid"])]
        killed = time.monotonic()
        result["events"] = drain(session, 10)
        result["eof_after_s"] = next((e["t"] for e in result["events"] if "harness" in e), None)
        session.process.wait(timeout=20)
        result["sandbox_exit"] = session.process.returncode
        result["detected_s"] = round(time.monotonic() - killed, 3)
    finally:
        session.close()
    time.sleep(1)
    result["stray_processes"] = stray_processes([str(args.work / "e8-killed"), "/rtex/bin/rtex"])
    for label, extra in (("tmpfs", None), ("cache-copy", "copy")):
        copy = None
        if extra:
            copy = args.work / "killed-cache-copy"
            shutil.copytree(cache, copy)
        again, first_layout_s = open_session(args, f"e8-killed-restart-{label}", project, cache=copy)
        result[f"restart_{label}_first_layout_s"] = first_layout_s
        again.close()
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--clone", type=Path, default=CLONE_DEFAULT)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--run-id", default="e8")
    parser.add_argument("--only", default="crash,hang,preamble,malformed,killed")
    args = parser.parse_args()
    args.clone, args.evidence = args.clone.resolve(), args.evidence.resolve()
    args.evidence.mkdir(parents=True, exist_ok=True)
    args.work = HERE / ".work" / args.run_id
    args.work.mkdir(parents=True, exist_ok=False)
    runs = {"crash": lambda: [crash(args)], "hang": lambda: [hang(args, None), hang(args, 8)],
            "preamble": lambda: [preamble(args, "missing-package"), preamble(args, "undefined-command")],
            "malformed": lambda: [malformed(args)], "killed": lambda: [serve_killed(args)]}
    results = []
    for name in args.only.split(","):
        for result in runs[name]():
            results.append(result)
            print(json.dumps(result, ensure_ascii=False)[:1500], flush=True)
            (args.evidence / f"e8-summary-{args.run_id}.json").write_text(
                json.dumps(results, indent=1, ensure_ascii=False) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
