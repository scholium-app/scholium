#!/usr/bin/env python3
"""E3 Chinese: live-unit share, per-keystroke latency, `rtex verify`, font-cache cost.

Fixtures: round-1 ctex and plain luatexja, plus the generated mixed article (~5 pages). All
sessions run in the `rtex` sandbox profile; one unsandboxed run of the mixed article is kept as
a reference. Latency is host monotonic time from writing the edit command to reading the
`ParagraphUpdate` line (rtex's own `timing.total_us` is recorded separately).
"""

import argparse
import json
from pathlib import Path
import shutil
import time

from gen_fixtures import write as generate
from measure import (CLONE_DEFAULT, HERE, body_units, edit_offset, keystrokes, run_sandboxed,
                     summarize_keystrokes, warm_font_cache)
from session import launch

EDITS = 120


def prepare(work, name):
    project = work / "projects" / name
    if name == "mixed":
        generate("mixed", 5, project)
    else:
        project.mkdir(parents=True)
        shutil.copyfile(HERE / f"fixtures/cjk/{name}.tex", project / "main.tex")
    return project


def wait_converged(session, seconds=180):
    layout = None
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        layout = session.wait(lambda e: e.get("event") == "LayoutUpdate", deadline - time.monotonic())
        if layout["convergence"]["state"] == "Converged":
            break
    return layout


def cache_env(cache):
    return {} if cache is None else {"SCHOLIUM_RTEX_FONT_CACHE": str(cache)}


def session_copy(args, warmed, run):
    """A per-session copy of a trusted warmed cache: documents only ever write the copy."""
    started = time.monotonic()
    copy = args.work / f"{run}-cache"
    shutil.copytree(warmed, copy)
    return copy, round((time.monotonic() - started) * 1000, 1)


def latency(args, name, project, sandbox, cache=None, tag=""):
    mode = ("sandbox" if sandbox else "host") + tag
    run = f"e3-{name}-{mode}"
    session = launch(args.clone, project, args.work / run, args.evidence, run, sandbox, seconds=900,
                     redact=True, extra_env=cache_env(cache))
    result = {"fixture": name, "mode": mode}
    try:
        started = session.started_at
        first = session.wait(lambda e: e.get("event") == "LayoutUpdate", 180)
        result["first_layout_s"] = round(session.received_at - started, 3)
        result["first_layout_wall_ms"] = first.get("wall_ms")
        layout = wait_converged(session) if first["convergence"]["state"] != "Converged" else first
        result.update(pages=layout["pages_total"], convergence=layout["convergence"]["state"])
        session.send({"cmd": "spans", "path": "main.tex"})
        spans = session.wait(lambda e: e.get("reply") == "spans")["spans"]
        units = body_units(spans)
        eligible = set(layout["eligible_paragraphs"])
        result["units"] = len(units)
        result["units_by_kind"] = {k: sum(1 for u in units if u["kind"] == k) for k in sorted({u["kind"] for u in units})}
        result["eligible"] = len([u for u in units if u["id"] in eligible])
        result["eligible_share"] = round(result["eligible"] / max(1, len(units)), 3)
        source = (project / "main.tex").read_text()
        targets = [(u, edit_offset(source, u, cjk=True)) for u in units]
        targets = [(u, o) for u, o in targets if o is not None]
        result["edit_targets"] = len(targets)
        records = keystrokes(session, source, targets, args.edits, "字")
        result["keystrokes"] = summarize_keystrokes(records)
        result["background_units"] = sorted({r["par"] for r in records if r.get("update") == "BackgroundScheduled"})
        result["background_reasons"] = sorted({reason for r in records for reason in (r.get("reasons_scheduled") or [])})[:20]
        (args.evidence / f"{run}-edits.json").write_text(json.dumps(records, indent=1, ensure_ascii=False) + "\n")
    except Exception as error:  # an observation for the log, not a harness crash
        result["error"] = repr(error)
    finally:
        session.close()
        result.update(exit=session.process.returncode, memory_peak_mib=mib(session.peak_bytes))
    return result


def mib(value):
    return None if value is None else round(value / 1024 / 1024, 1)


def verify(args, name, project):
    """`rtex verify` (allow-list eligibility, layers 1–2) inside the sandbox; host as reference."""
    out = {}
    work = args.work / f"verify-{name}"
    status, wall = run_sandboxed(project, work, 900, ["/rtex/bin/rtex", "verify", "--project", "/project",
                                 "--build", "/work/verify", "--json-out", "/work/verify.json"],
                                 clone=args.clone, log=args.evidence / f"e3-{name}-verify-sandbox.log")
    out["sandbox"] = {"exit": status, "wall_s": wall, **digest(work / "verify.json")}
    if (work / "verify.json").exists():
        shutil.copyfile(work / "verify.json", args.evidence / f"e3-{name}-verify-sandbox.json")
    return out


def digest(path):
    if not path.exists():
        return {"report": "missing"}
    report = json.loads(path.read_text())
    paragraphs = report.get("paragraphs", [])
    return {"units_total": report.get("paragraphs_total"), "units_eligible": report.get("paragraphs_eligible"),
            "glyphs_total": sum(p.get("glyphs_total", 0) for p in paragraphs),
            "glyphs_identical": sum(p.get("glyphs_identical", 0) for p in paragraphs),
            "pages": len(report.get("pages", [])),
            "pages_exact": sum(1 for p in report.get("pages", []) if p.get("exact")),
            "layer1_pass": report.get("layer1_pass"), "layer2_pass": report.get("layer2_pass"),
            "capture_pdf_equals_clean": report.get("capture_pdf_equals_clean"),
            "ineligible_reasons": report.get("ineligible_reasons")}


def first_layout(args, project, run, sandbox=True, cache=None):
    session = launch(args.clone, project, args.work / run, args.evidence, run, sandbox,
                     extra_env=cache_env(cache), seconds=300, redact=True)
    row = {"run": run}
    try:
        layout = session.wait(lambda e: e.get("event") == "LayoutUpdate", 240)
        row.update(first_layout_s=round(session.received_at - session.started_at, 3),
                   wall_ms=layout.get("wall_ms"), compile=layout["compile"]["state"], pages=layout["pages_total"])
    except Exception as error:
        row["error"] = repr(error)
    finally:
        session.close()
    row.update(exit=session.process.returncode, memory_peak_mib=mib(session.peak_bytes))
    return row


def font_cache(args, name, project):
    """Fresh tmpfs TEXMFVAR (default) vs a persisted cache: shared cold/warm, per-session copy."""
    cache = args.work / f"font-cache-{name}"
    cache.mkdir()
    rows = [first_layout(args, project, f"e3-cache-{name}-tmpfs-{i}") for i in (1, 2)]
    rows.append(first_layout(args, project, f"e3-cache-{name}-rw-cold", cache=cache))
    size = sum(f.stat().st_size for f in cache.rglob("*") if f.is_file())
    rows.append(first_layout(args, project, f"e3-cache-{name}-rw-warm", cache=cache))
    copy, copy_ms = session_copy(args, cache, f"e3-cache-{name}-copy-warm")
    rows.append({**first_layout(args, project, f"e3-cache-{name}-copy-warm", cache=copy), "copy_ms": copy_ms})
    rows.append(first_layout(args, project, f"e3-cache-{name}-host", sandbox=False))
    return {"fixture": name, "cache_bytes_after_cold": size, "runs": rows}


def followup(args):
    """After the first run: luatexja was OOM-killed (2 GiB scope) building font caches on a fresh
    tmpfs, and a read-only cache mount failed (luaotfload needs a writable cache). Repeat luatexja
    on tmpfs, run it on a per-session copy of a warmed cache, and redo the cache matrix."""
    project = prepare(args.work, "luatexja")
    warmed = args.work / "font-cache-luatexja"
    warmed.mkdir()
    rows = [latency(args, "luatexja", project, True, tag="-tmpfs-repeat"),
            first_layout(args, project, "e3-cache-luatexja-rw-cold", cache=warmed)]
    copy, copy_ms = session_copy(args, warmed, "e3-luatexja-copy")
    rows.append({**latency(args, "luatexja", project, True, cache=copy, tag="-cache-copy"), "copy_ms": copy_ms})
    rows.append(first_layout(args, project, "e3-cache-luatexja-host", sandbox=False))
    out = {"luatexja": rows, "font_cache": [font_cache(args, n, prepare(args.work, n)) for n in ("ctex", "mixed")]}
    (args.evidence / "e3-followup.json").write_text(json.dumps(out, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps(out, ensure_ascii=False)[:3000], flush=True)
    return 0


def warmed_luatexja(args):
    """luatexja with the trusted warm-up (one plain LuaLaTeX run fills the cache first), then a
    session on a per-session copy of that cache: the setting proposed for the product."""
    project = prepare(args.work, "luatexja")
    warmed = args.work / "font-cache-luatexja"
    warm = warm_font_cache(warmed, project, args.work, args.clone)
    copy, copy_ms = session_copy(args, warmed, "e3-luatexja-warmed")
    row = {**latency(args, "luatexja", project, True, cache=copy, tag="-warmed-copy"), "warmup": warm,
           "copy_ms": copy_ms, "cache_bytes": sum(f.stat().st_size for f in warmed.rglob("*") if f.is_file())}
    (args.evidence / "e3-luatexja-warmed.json").write_text(json.dumps(row, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps(row, ensure_ascii=False)[:1500], flush=True)
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--clone", type=Path, default=CLONE_DEFAULT)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--run-id", default="e3")
    parser.add_argument("--edits", type=int, default=EDITS)
    parser.add_argument("--followup", action="store_true",
                        help="luatexja again (tmpfs, per-session cache copy) and the font-cache matrix")
    parser.add_argument("--warmed-luatexja", action="store_true",
                        help="luatexja after a trusted single-process cache warm-up")
    args = parser.parse_args()
    args.clone, args.evidence = args.clone.resolve(), args.evidence.resolve()
    args.evidence.mkdir(parents=True, exist_ok=True)
    args.work = HERE / ".work" / args.run_id
    args.work.mkdir(parents=True, exist_ok=False)
    if args.followup:
        return followup(args)
    if args.warmed_luatexja:
        return warmed_luatexja(args)
    summary = {"latency": [], "verify": {}, "font_cache": []}
    for name in ("ctex", "luatexja", "mixed"):
        project = prepare(args.work, name)
        summary["latency"].append(latency(args, name, project, sandbox=True))
        print(json.dumps(summary["latency"][-1], ensure_ascii=False)[:600], flush=True)
        summary["verify"][name] = verify(args, name, project)
        print(name, json.dumps(summary["verify"][name])[:400], flush=True)
        if name == "mixed":
            summary["latency"].append(latency(args, name, project, sandbox=False))
        if name in ("ctex", "mixed"):
            summary["font_cache"].append(font_cache(args, name, project))
            print(json.dumps(summary["font_cache"][-1])[:800], flush=True)
    (args.evidence / "e3-summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
