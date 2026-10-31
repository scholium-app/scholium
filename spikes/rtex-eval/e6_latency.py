#!/usr/bin/env python3
"""E6 end-to-end latency, host side: keystroke → ParagraphUpdate → unit rasterized.

There is no LaTeX window yet, so the pipeline is measured without a GUI: the harness writes an
edit to a sandboxed `rtex serve` (the keystroke), reads the `ParagraphUpdate` line, and hands the
event to the E4 prototype running as a warm worker that rasterizes the unit's rows at 150 dpi.
Fixtures: `rtex gen-book` 10 and 100 pages. Two typing patterns per fixture:

- steady: one keystroke every 150 ms after the previous update (background passes stay
  debounced, the live engine runs alone);
- bursts: ten keystrokes, then a 1.5 s pause so a background pass starts, then the next burst
  types while that pass is still running (live and background compete for CPU).

The real-window measurement (texture upload, frame presentation) is deferred to L1.
"""

import argparse
import json
from pathlib import Path
import subprocess
import time

from e4_draw import converged_pages
from measure import (CLONE_DEFAULT, HERE, Raster, body_units, edit_offset, keystrokes, summarize_keystrokes,
                     warm_font_cache)
from session import MB, launch

EDITS = 120
BURST, PAUSE = 10, 1.5


def bursts(session, source, targets, raster):
    records = []
    for chunk in range(EDITS // BURST):
        rotated = targets[chunk * (BURST // 2) % len(targets):] + targets[:chunk * (BURST // 2) % len(targets)]
        part = keystrokes(session, source, rotated, BURST, "x", gap=0.05, raster=raster)
        records.extend({**r, "i": chunk * BURST + r["i"]} for r in part)
        time.sleep(PAUSE)
    return records


def measure(args, pages):
    project = args.work / "projects" / f"book{pages}"
    subprocess.run([str(args.clone / "target/release/rtex"), "gen-book", "--pages", str(pages), "--out", str(project)],
                   check=True, capture_output=True)
    run = f"e6-book{pages}"
    cache = args.work / "font-cache"
    warm = warm_font_cache(cache, project, args.work, args.clone)
    session = launch(args.clone, project, args.work / run, args.evidence, run, True, seconds=1800, redact=True,
                     line_limit=512 * MB, total_limit=8192 * MB, extra_env={"SCHOLIUM_RTEX_FONT_CACHE": str(cache)})
    raster = Raster(dpi=150)
    result = {"fixture": f"book{pages}", "font_cache_warmup": warm}
    try:
        page_lists, layout, _, _ = converged_pages(session, args.work / run, seconds=600)
        result.update(pages=layout["pages_total"], converged_after_s=round(session.received_at - session.started_at, 3),
                      converged_layout_wall_ms=layout.get("wall_ms"), passes=layout.get("passes"))
        warm, _ = raster.draw({"event": "LayoutUpdate", "pages_changed": page_lists[:2]})
        result["warmup_two_pages_ms"] = warm["raster_us"] / 1000
        session.send({"cmd": "spans", "path": "main.tex"})
        spans = session.wait(lambda e: e.get("reply") == "spans")["spans"]
        source = (project / "main.tex").read_text()
        targets = [(u, edit_offset(source, u, cjk=False)) for u in body_units(spans)]
        targets = [(u, o) for u, o in targets if o is not None]
        before = len(session.events)
        steady = keystrokes(session, source, targets, EDITS, "x", gap=0.15, raster=raster)
        result["steady"] = summarize_keystrokes(steady)
        mid = len(session.events)
        burst = bursts(session, source, targets, raster)
        result["bursts"] = summarize_keystrokes(burst)
        result["layouts_during_steady"] = sum(1 for e in session.events[before:mid] if e.get("event") == "LayoutUpdate")
        result["layouts_during_bursts"] = sum(1 for e in session.events[mid:] if e.get("event") == "LayoutUpdate")
        for name, records in (("steady", steady), ("bursts", burst)):
            (args.evidence / f"{run}-{name}.json").write_text(json.dumps(records, indent=1) + "\n")
    except Exception as error:
        result["error"] = repr(error)
    finally:
        raster.close()
        session.close()
        result.update(sandbox_exit=session.process.returncode,
                      memory_peak_mib=None if session.peak_bytes is None else round(session.peak_bytes / 2**20, 1))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--clone", type=Path, default=CLONE_DEFAULT)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--run-id", default="e6")
    parser.add_argument("--pages", default="10,100")
    args = parser.parse_args()
    args.clone, args.evidence = args.clone.resolve(), args.evidence.resolve()
    args.evidence.mkdir(parents=True, exist_ok=True)
    args.work = HERE / ".work" / args.run_id
    args.work.mkdir(parents=True, exist_ok=False)
    summary = []
    for pages in (int(p) for p in args.pages.split(",")):
        summary.append(measure(args, pages))
        print(json.dumps(summary[-1]), flush=True)
    (args.evidence / "e6-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
