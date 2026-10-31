"""Round-2 measurement helpers: percentiles, scripted keystrokes, the raster worker, sandbox runs."""

import json
import math
import os
from pathlib import Path
import subprocess
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
RASTER_BIN = HERE / "raster/target/release/rtex-eval-raster"
CLONE_DEFAULT = Path.home() / ".cache/scholium-eval/rtex-v0.0.5"


def percentiles(values):
    """Nearest-rank p50/p95 plus n/min/max; None for an empty sample."""
    if not values:
        return None
    ordered = sorted(values)
    rank = lambda q: ordered[max(0, math.ceil(q * len(ordered)) - 1)]
    return {"n": len(ordered), "p50": round(rank(0.5), 3), "p95": round(rank(0.95), 3),
            "min": round(ordered[0], 3), "max": round(ordered[-1], 3)}


def is_cjk(ch):
    return "一" <= ch <= "鿿"


def edit_offset(source, span, cjk):
    """A byte offset in the middle of a span's running text, outside inline math.

    CJK fixtures: before a CJK ideograph; otherwise before a lowercase word after a space.
    """
    data = source.encode()
    start, end = span["range"]["start"], span["range"]["end"]
    text = data[start:end].decode(errors="ignore")
    candidates, dollars, offset = [], 0, start
    for i, ch in enumerate(text):
        if ch == "$" and (i == 0 or text[i - 1] != "\\"):
            dollars += 1
        elif dollars % 2 == 0:
            if cjk and is_cjk(ch):
                candidates.append(offset)
            elif not cjk and ch.islower() and ch.isascii() and i > 0 and text[i - 1] == " ":
                candidates.append(offset)
        offset += len(ch.encode())
    return candidates[len(candidates) // 2] if candidates else None


class Raster:
    """The E4 prototype as a long-lived worker (`rtex-eval-raster serve`), warm font caches."""

    def __init__(self, dpi=150):
        self.process = subprocess.Popen([str(RASTER_BIN), "serve", "--dpi", str(dpi)],
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.seq = 0

    def draw(self, event):
        self.seq += 1
        self.process.stdin.write(json.dumps({"seq": self.seq, "event": event}) + "\n")
        self.process.stdin.flush()
        reply = json.loads(self.process.stdout.readline())
        return reply, time.monotonic()

    def close(self):
        self.process.stdin.close()
        self.process.wait(timeout=10)


def keystrokes(session, source, units, count, insert, gap=0.15, raster=None, timeout=10,
               on_edit=None):
    """`count` single-character edits, alternating insert/delete, round-robin over `units`.

    Each edit waits for its `ParagraphUpdate` (or `BackgroundScheduled`) before the next one.
    Times are host monotonic: write of the edit command → receipt of the update line
    (→ rasterized by the worker when `raster` is given). `source` is restored at the end.
    """
    records, size = [], len(insert.encode())
    for i in range(count):
        span, offset = units[(i // 2) % len(units)]
        if i % 2 == 0:
            command = {"cmd": "edit", "path": "main.tex", "start": offset, "end": offset, "text": insert}
        else:
            command = {"cmd": "edit", "path": "main.tex", "start": offset, "end": offset + size, "text": ""}
        sent = session.send(command)
        reply = session.wait(lambda e: e.get("reply") in ("edit", "error"), timeout)
        record = {"i": i, "par": span["id"], "reply_ms": round((session.received_at - sent) * 1000, 3)}
        if reply.get("reply") == "error":
            records.append({**record, "error": reply})
            continue
        result = reply["result"]
        record.update(routed=result["routed"], reasons=result["reasons"], host_us=result["host_us"])
        edit_id = result["edit_id"]
        try:
            update = session.wait(lambda e: (e.get("event") == "ParagraphUpdate" and e.get("edit_id") == edit_id)
                                  or (e.get("event") == "BackgroundScheduled"
                                      and e.get("par_id") in result["outcome"]["touched"]), timeout)
        except TimeoutError:
            records.append({**record, "update": "timeout"})
            continue
        record.update(update=update["event"], update_ms=round((session.received_at - sent) * 1000, 3))
        if update["event"] == "ParagraphUpdate":
            record.update(status=update["status"], timing=update.get("timing"),
                          pagination_stale=update.get("pagination_stale"),
                          context_stale=update.get("context_stale"))
            if raster is not None:
                drawn, done = raster.draw(update)
                record.update(raster_ms=round((done - sent) * 1000, 3), raster_us=drawn.get("raster_us"),
                              raster_glyphs_skipped=drawn.get("stats", {}).get("glyphs_skipped"))
        else:
            record["reasons_scheduled"] = update.get("reasons")
        if on_edit:
            on_edit(record, update)
        records.append(record)
        time.sleep(gap)
    return records


def summarize_keystrokes(records):
    fast = [r for r in records if r.get("status") == "ok"]
    return {
        "edits": len(records),
        "routed": {k: sum(1 for r in records if r.get("routed") == k) for k in ("fast", "background", "preamble")},
        "paragraph_ok": len(fast),
        "statuses": sorted({r.get("status") or r.get("update") or "error" for r in records}),
        "update_ms": percentiles([r["update_ms"] for r in fast]),
        "reply_ms": percentiles([r["reply_ms"] for r in records]),
        "rtex_total_ms": percentiles([r["timing"]["total_us"] / 1000 for r in fast if r.get("timing")]),
        "raster_ms": percentiles([r["raster_ms"] for r in fast if "raster_ms" in r]),
        "raster_only_ms": percentiles([r["raster_us"] / 1000 for r in fast if r.get("raster_us") is not None]),
    }


def run_sandboxed(project, work, seconds, argv, profile="rtex", clone=CLONE_DEFAULT, extra_env=None,
                  log=None):
    """One command inside `spikes/toolchain-sandbox.sh`; returns (exit status, wall seconds)."""
    env = dict(os.environ, SCHOLIUM_SANDBOX_PROFILE=profile)
    if profile == "rtex":
        env.update(SCHOLIUM_RTEX_BIN=str(clone / "target/release/rtex"), SCHOLIUM_RTEX_TEXDIR=str(clone / "tex"))
    env.update(extra_env or {})
    work.mkdir(parents=True, exist_ok=True)
    command = ["bash", str(ROOT / "spikes/toolchain-sandbox.sh"), str(project), str(work), str(seconds), *argv]
    started = time.monotonic()
    with (log.open("w") if log else open(os.devnull, "w")) as out:
        status = subprocess.run(command, env=env, cwd=ROOT, stdout=out, stderr=subprocess.STDOUT).returncode
    return status, round(time.monotonic() - started, 3)


def body_units(spans):
    """Spans that typeset content: everything except the preamble and `\\end{document}` trailer."""
    return [s for s in spans if s["kind"] not in ("Preamble", "Trailer")]


def warm_font_cache(cache, project, work, clone=CLONE_DEFAULT):
    """Trusted warm-up before a session: one plain LuaLaTeX run (a single process) fills the
    persisted font cache, so the session's three concurrent engines do not each build it."""
    cache.mkdir(parents=True, exist_ok=True)
    status, wall = run_sandboxed(project, work / f"warm-{project.name}", 600,
                                 ["/usr/bin/lualatex", "-interaction=nonstopmode", "-output-directory=/work",
                                  "/project/main.tex"], clone=clone,
                                 extra_env={"SCHOLIUM_RTEX_FONT_CACHE": str(cache)})
    return {"project": project.name, "exit": status, "wall_s": wall}
