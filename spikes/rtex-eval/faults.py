"""E8 helpers: process discovery from the host, a host-side per-pass wall-clock watchdog, and
event collection with timestamps."""

import gzip
import json
import os
from pathlib import Path
import signal
import threading
import time

TICKS = os.sysconf("SC_CLK_TCK")


def processes(root_pid):
    """Live descendants of `root_pid`: pid, comm, cmdline, cpu seconds (host /proc view)."""
    children, info = {}, {}
    for proc in Path("/proc").iterdir():
        if not proc.name.isdigit():
            continue
        try:
            stat = (proc / "stat").read_text()
            cmdline = (proc / "cmdline").read_bytes().replace(b"\0", b" ").decode(errors="replace")
        except OSError:
            continue
        fields = stat[stat.rindex(")") + 2:].split()
        pid, ppid = int(proc.name), int(fields[1])
        children.setdefault(ppid, []).append(pid)
        info[pid] = {"pid": pid, "comm": stat[stat.index("(") + 1:stat.rindex(")")], "cmdline": cmdline[:300],
                     "cpu": (int(fields[11]) + int(fields[12])) / TICKS}
    found, stack = [], list(children.get(root_pid, []))
    while stack:
        pid = stack.pop()
        found.append(info[pid])
        stack.extend(children.get(pid, []))
    return found


def is_tex(proc):
    return proc["comm"].startswith("lua")


def live_engine(root_pid):
    return [p for p in processes(root_pid) if is_tex(p) and "rtex-serve" in p["cmdline"]]


def rtex_server(root_pid):
    return [p for p in processes(root_pid) if p["comm"] == "rtex"]


def kill(pid):
    try:
        os.kill(pid, signal.SIGKILL)
        return True
    except ProcessLookupError:
        return False


class PassWatchdog(threading.Thread):
    """Host-side per-pass wall-clock timeout (rtex has none for background passes, upstream #22).

    A background LuaLaTeX (any TeX process that is not the live `rtex-serve` engine) that has been
    burning CPU continuously for longer than `limit` seconds is killed. A standby engine waiting
    for its body uses no CPU, so only a released pass is timed.
    """

    def __init__(self, root_pid, limit, period=0.25):
        super().__init__(daemon=True)
        self.root_pid, self.limit, self.period = root_pid, limit, period
        self.seen, self.kills, self.stopped = {}, [], threading.Event()
        self.origin = time.monotonic()

    def run(self):
        while not self.stopped.wait(self.period):
            now = time.monotonic()
            for proc in processes(self.root_pid):
                if not is_tex(proc) or "rtex-serve" in proc["cmdline"]:
                    continue
                state = self.seen.setdefault(proc["pid"], {"cpu": proc["cpu"], "active_since": None})
                busy = proc["cpu"] - state["cpu"] >= 0.5 * self.period
                state["cpu"] = proc["cpu"]
                if not busy:
                    state["active_since"] = None
                    continue
                state["active_since"] = state["active_since"] or now - self.period
                if now - state["active_since"] > self.limit and kill(proc["pid"]):
                    self.kills.append({"t": round(now - self.origin, 2), "pid": proc["pid"],
                                       "active_s": round(now - state["active_since"], 2), "cpu_s": proc["cpu"]})
                    state["active_since"] = None

    def stop(self):
        self.stopped.set()
        self.join(timeout=2)


def drain(session, seconds, until=None):
    """Every message for `seconds` (or until `until(message)`), with times relative to now."""
    started, out = time.monotonic(), []
    while True:
        left = seconds - (time.monotonic() - started)
        if left <= 0:
            return out
        try:
            message = session.wait(lambda e: True, left)
        except TimeoutError:
            return out
        except RuntimeError as error:
            out.append({"t": round(time.monotonic() - started, 3), "harness": str(error)})
            return out
        out.append({"t": round(session.received_at - started, 3), **brief(message)})
        if until and until(message):
            return out


def brief(message):
    keys = ("event", "reply", "state", "reason", "status", "par_id", "edit_id", "compile", "convergence",
            "wall_ms", "pages_total", "reasons", "message", "source")
    out = {k: message[k] for k in keys if k in message}
    if "result" in message:
        out["result"] = {k: message["result"].get(k) for k in ("edit_id", "routed", "reasons", "source_revision")}
    if "items" in message:
        out["items"] = [{k: i.get(k) for k in ("severity", "line", "message")} for i in message["items"][:3]]
    if "versions" in message:
        out["source_revision"] = message["versions"].get("source_revision")
        out["engine_generation"] = message["versions"].get("engine_generation")
    for key in ("reason", "message"):
        if isinstance(out.get(key), str):
            out[key] = out[key][:300]
    return out


def timeline(evidence, name, origin):
    """Every command and message of a session transcript, seconds relative to `origin`.

    `Session.wait` drops messages that do not match its predicate, but the transcript keeps them
    all with their receipt time, so timelines come from here.
    """
    out = []
    with gzip.open(evidence / f"{name}.jsonl.gz", "rt") as transcript:
        for line in transcript:
            row = json.loads(line)
            t = round(row["monotonic"] - origin, 3)
            if row["direction"] == "stdin":
                value = row["value"]
                out.append({"t": t, "sent": value.get("cmd"), **{k: value[k] for k in ("start", "end") if k in value},
                            "text": (value.get("text") or "")[:40]})
            elif row["direction"] == "stdout":
                out.append({"t": t, **brief(row["value"])})
    return out


def first(events, predicate, after=0.0):
    return next((e for e in events if e["t"] >= after and predicate(e)), None)
