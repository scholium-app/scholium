"""E2 control (f): a runaway unit is cut off and the session keeps accepting edits."""

import os
from pathlib import Path
import time

from common import stray_processes, summarize, wait_quietly, write_project
from session import launch

LOOP = "\\directlua{while true do end}"
HOLD_SECONDS = 5
TICKS = os.sysconf("SC_CLK_TCK")


def descendants_cpu(root_pid):
    """CPU seconds of every live descendant process (host view, also inside the sandbox)."""
    children, info = {}, {}
    for proc in Path("/proc").iterdir():
        if not proc.name.isdigit():
            continue
        try:
            stat = (proc / "stat").read_text()
        except OSError:
            continue
        name = stat[stat.index("(") + 1:stat.rindex(")")]
        fields = stat[stat.rindex(")") + 2:].split()
        pid, ppid = int(proc.name), int(fields[1])
        children.setdefault(ppid, []).append(pid)
        info[pid] = {"pid": pid, "comm": name, "cpu_seconds": (int(fields[11]) + int(fields[12])) / TICKS}
    found, stack = [], list(children.get(root_pid, []))
    while stack:
        pid = stack.pop()
        found.append(info[pid])
        stack.extend(children.get(pid, []))
    return found


def runaway(args, sandbox):
    """One run; the verdict compares the host run (attack must bite) with the sandbox run."""
    mode = "sandbox" if sandbox else "host"
    name = f"negative-f-default-{mode}"
    run_dir = args.work / name
    project = run_dir / "project"
    text = write_project(project, "f-runaway.tex", {})
    build = run_dir / "build-dir"
    # Each runaway pass may burn the full per-process CPU limit; leave room for two of them.
    session = launch(args.clone, project, build, args.evidence, name, sandbox, seconds=300)
    started = time.monotonic()
    timeline = []

    def mark(label, message):
        timeline.append({"t": round(time.monotonic() - started, 1), "label": label, **summarize(message)})
        return message

    def edit(start, end, replacement):
        session.send({"cmd": "edit", "start": start, "end": end, "text": replacement})
        return mark("edit reply", wait_quietly(session, lambda e: e.get("reply") in ("edit", "error"), 20))

    try:
        mark("initial layout", wait_quietly(session, lambda e: e.get("event") == "LayoutUpdate", 60))
        loop_at = len(text[:text.index("LOOPHERE")].encode())
        edit(loop_at, loop_at, LOOP)
        mark("live unit cut", wait_quietly(session, lambda e: (
            e.get("event") == "EngineState" and e.get("state") in ("Restarting", "Failed"))
            or (e.get("event") == "ParagraphUpdate" and e.get("status") != "ok"), 20))
        mark("background scheduled", wait_quietly(session, lambda e: e.get("event") == "BackgroundScheduled", 10))
        # Keep the loop in the source until the background pass has taken its snapshot.
        time.sleep(HOLD_SECONDS)
        edit(loop_at, loop_at + len(LOOP), "")
        after_at = len(text[:text.index("AFTERHERE") + len("AFTERHERE")].encode())
        reply = edit(after_at, after_at, " E2AFTER")
        followup = mark("follow-up paragraph", wait_quietly(session, lambda e: (
            e.get("event") == "ParagraphUpdate" and e.get("edit_id") == reply.get("result", {}).get("edit_id")), 40))
        layout = mark("layout after fix", wait_quietly(session, lambda e: (
            e.get("event") == "LayoutUpdate" and e.get("compile", {}).get("state") == "Ok"
            and e.get("versions", {}).get("source_revision", 0) >= 4), 150))
        processes = descendants_cpu(session.process.pid)
        out = "/work/export.pdf" if sandbox else str(build / "export.pdf")
        session.send({"cmd": "export_pdf", "out": out})
        export = mark("export", wait_quietly(session, lambda e: e.get("event") == "PdfExported", 60))
    finally:
        session.close()
    time.sleep(1)
    result = {"name": name, "control": "f", "config": "default", "mode": mode, "timeline": timeline,
              "processes_before_export": processes,
              "stray_processes": stray_processes([str(run_dir)] + (["/rtex/bin/rtex"] if sandbox else []))}
    result["followup_ok"] = followup.get("status") == "ok"
    result["layout_recovered"] = layout.get("event") == "LayoutUpdate"
    result["export_ok"] = export.get("status", {}).get("state") == "Ok"
    result["max_tex_cpu_seconds"] = max([p["cpu_seconds"] for p in processes
                                         if p["comm"].startswith("lua")] or [0])
    # Effective unsandboxed means the runaway unit blocked the background layout without bound.
    # Inside the sandbox the same unit must be cut off and the session must keep editing.
    contained = result["followup_ok"] and result["layout_recovered"]
    result["attack_effective"] = not contained
    return result
