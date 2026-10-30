"""Helpers shared by the E2 negative-control drivers."""

import os
from pathlib import Path

HERE = Path(__file__).resolve().parent


def write_project(project, body, values):
    """main.tex = shared negative preamble + one control body, placeholders filled."""
    fixtures = HERE / "fixtures/negative"
    text = (fixtures / "preamble.tex").read_text() + (fixtures / body).read_text() + "\\end{document}\n"
    for key, value in values.items():
        text = text.replace(key, value)
    project.mkdir(parents=True)
    (project / "main.tex").write_text(text)
    return text


def wait_quietly(session, predicate, seconds):
    """Like Session.wait, but a timeout or worker exit is an observation, not an exception."""
    try:
        return session.wait(predicate, seconds)
    except (TimeoutError, RuntimeError) as error:
        return {"harness_wait": str(error)}


def summarize(message):
    keys = ("event", "reply", "state", "reason", "status", "compile", "pages_total", "converged",
            "edit_id", "harness_wait", "message")
    return {k: message[k] for k in keys if k in message}


def own_lineage():
    pids, pid = set(), os.getpid()
    while pid > 1:
        pids.add(pid)
        stat = Path(f"/proc/{pid}/stat").read_text()
        pid = int(stat[stat.rindex(")") + 2:].split()[1])
    return pids


def stray_processes(markers):
    """Processes still alive with this run's paths in their command line; killed if found."""
    found, skip = [], own_lineage()
    for proc in Path("/proc").iterdir():
        if not proc.name.isdigit() or int(proc.name) in skip:
            continue
        try:
            cmdline = (proc / "cmdline").read_bytes().replace(b"\0", b" ").decode(errors="replace")
        except OSError:
            continue
        if any(marker in cmdline for marker in markers):
            found.append({"pid": int(proc.name), "cmdline": cmdline[:200]})
            try:
                os.kill(int(proc.name), 9)
            except ProcessLookupError:
                pass
    return found
