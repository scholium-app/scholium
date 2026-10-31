#!/usr/bin/env python3
"""E4 drawing: display lists rasterized by the prototype vs independent PDF rasterizers.

For each fixture a sandboxed session runs to a converged layout; its page display lists are
rasterized by `raster/` (tiny-skia + skrifa) and compared with the layout's own PDF rendered by
poppler (`pdftoppm`, splash backend: the reference), poppler's cairo backend (second reference)
and hayro (the candidate PDF fallback). Criterion (upstream `rtex verify --raster`): unmatched
ink ≤ 1 % with 1-pixel tolerance and no skipped glyph; degraded pages (TikZ) are judged on the
hayro fallback instead.
"""

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import time

from gen_fixtures import write as generate
from measure import CLONE_DEFAULT, HERE, RASTER_BIN, percentiles, warm_font_cache
from session import MB, launch

DPIS = (150, 300)
LIMIT = 0.01


def project_for(args, name):
    project = args.work / "projects" / name
    if name == "book10":
        subprocess.run([str(args.clone / "target/release/rtex"), "gen-book", "--pages", "10", "--out",
                        str(project)], check=True, capture_output=True)
    elif name in ("mixed", "tikz"):
        generate(name, 5, project)
    else:
        project.mkdir(parents=True)
        source = HERE / ("fixtures/article.tex" if name == "article" else f"fixtures/cjk/{name}.tex")
        shutil.copyfile(source, project / "main.tex")
    return project


def converged_pages(session, build, seconds=240):
    """Page lists of the converged layout (pages accumulated over the convergence passes)."""
    pages, deadline, layout = {}, time.monotonic() + seconds, None
    while time.monotonic() < deadline:
        layout = session.wait(lambda e: e.get("event") == "LayoutUpdate", deadline - time.monotonic())
        for page in layout["pages_changed"]:
            pages[page["page"]] = page
        if layout["convergence"]["state"] == "Converged":
            break
    version = layout["versions"]["layout_version"]
    pdf = build / "build" / "bg" / f"layout-{version}.pdf"
    kept = build / "reference.pdf"
    shutil.copyfile(pdf, kept)
    fallback = layout.get("pdf_fallback")
    pages = [pages[n] for n in sorted(pages) if n <= layout["pages_total"]]
    return pages, layout, kept, fallback


def to_pgm(png, pgm):
    subprocess.run(["magick", str(png), "-colorspace", "gray", str(pgm)], check=True)


def compare(a, b):
    out = subprocess.run([str(RASTER_BIN), "compare", str(a), str(b)], capture_output=True, text=True, check=True)
    diff = json.loads(out.stdout)
    return {"unmatched_pct": round(diff["unmatched_fraction"] * 100, 4), "ink": [diff["ink_a"], diff["ink_b"]],
            "size": [diff["size_a"], diff["size_b"]]}


def references(pdf, page, dpi, out):
    timings = {}
    started = time.monotonic()
    subprocess.run(["pdftoppm", "-r", str(dpi), "-gray", "-f", str(page), "-l", str(page), "-singlefile",
                    str(pdf), str(out / f"splash-{page}")], check=True)
    timings["pdftoppm_process_ms"] = round((time.monotonic() - started) * 1000, 1)
    subprocess.run(["pdftocairo", "-png", "-gray", "-r", str(dpi), "-f", str(page), "-l", str(page), "-singlefile",
                    str(pdf), str(out / f"cairo-{page}")], check=True)
    to_pgm(out / f"cairo-{page}.png", out / f"cairo-{page}.pgm")
    hayro = subprocess.run([str(RASTER_BIN), "pdf", str(pdf), str(page), str(out / f"hayro-{page}.pgm"),
                            "--dpi", str(dpi)], capture_output=True, text=True, check=True)
    rendered = json.loads(hayro.stdout)
    timings.update(hayro_load_ms=rendered["load_us"] / 1000, hayro_render_ms=rendered["render_us"] / 1000)
    return timings


def evaluate(args, name):
    project = project_for(args, name)
    run = f"e4-{name}"
    build = args.work / run
    warm = warm_font_cache(args.cache, project, args.work, args.clone)
    session = launch(args.clone, project, build, args.evidence, run, True, seconds=600, redact=True,
                     line_limit=256 * MB, total_limit=1024 * MB,
                     extra_env={"SCHOLIUM_RTEX_FONT_CACHE": str(args.cache)})
    try:
        pages, layout, pdf, fallback = converged_pages(session, build)
    finally:
        session.close()
    (build / "layout.json").write_text(json.dumps({"pages_changed": pages}))
    result = {"fixture": name, "pages": len(pages), "pdf_fallback": fallback, "font_cache_warmup": warm,
              "compile": layout["compile"]["state"], "by_dpi": {}}
    for dpi in DPIS:
        out = build / f"dpi{dpi}"
        raster = subprocess.run([str(RASTER_BIN), "pages", str(build / "layout.json"), str(out / "dl"),
                                 "--dpi", str(dpi), "--repeat", "2"], capture_output=True, text=True, check=True)
        lines = [json.loads(line) for line in raster.stdout.splitlines()]
        rows = []
        for line in (l for l in lines if l["round"] == 2):
            cold = next(l for l in lines if l["round"] == 1 and l["page"] == line["page"])
            n = line["page"]
            row = {"page": n, "exact": line["exact"], "degraded": line["stats"]["degraded"],
                   "glyphs": line["stats"]["glyphs_drawn"], "skipped": line["stats"]["glyphs_skipped"],
                   "images_skipped": line["stats"]["images_skipped"], "missing_fonts": line["missing_fonts"],
                   "dl_cold_ms": cold["raster_us"] / 1000, "dl_warm_ms": line["raster_us"] / 1000}
            row.update(references(pdf, n, dpi, out))
            dl = out / "dl" / f"page-{n}.pgm"
            row["dl_vs_splash"] = compare(dl, out / f"splash-{n}.pgm")
            row["dl_vs_cairo"] = compare(dl, out / f"cairo-{n}.pgm")
            row["dl_vs_hayro"] = compare(dl, out / f"hayro-{n}.pgm")
            row["hayro_vs_splash"] = compare(out / f"hayro-{n}.pgm", out / f"splash-{n}.pgm")
            row["splash_vs_cairo"] = compare(out / f"splash-{n}.pgm", out / f"cairo-{n}.pgm")
            if row["exact"] and not row["degraded"]:
                row["path"] = "display-list"
                row["pass"] = row["dl_vs_splash"]["unmatched_pct"] <= LIMIT * 100 and row["skipped"] == 0
            else:
                row["path"] = "pdf-fallback(hayro)"
                row["pass"] = row["hayro_vs_splash"]["unmatched_pct"] <= LIMIT * 100
            rows.append(row)
        result["by_dpi"][dpi] = {
            "pages": rows,
            "pass": all(r["pass"] for r in rows),
            "worst_dl_vs_splash": max((r["dl_vs_splash"]["unmatched_pct"] for r in rows if r["path"] == "display-list"), default=None),
            "worst_dl_vs_cairo": max((r["dl_vs_cairo"]["unmatched_pct"] for r in rows if r["path"] == "display-list"), default=None),
            "worst_splash_vs_cairo": max(r["splash_vs_cairo"]["unmatched_pct"] for r in rows),
            "dl_warm_ms": percentiles([r["dl_warm_ms"] for r in rows]),
            "dl_cold_first_page_ms": rows[0]["dl_cold_ms"] if rows else None,
            "hayro_render_ms": percentiles([r["hayro_render_ms"] for r in rows]),
            "pdftoppm_process_ms": percentiles([r["pdftoppm_process_ms"] for r in rows]),
        }
    keep_pictures(args, name, build, result)
    return result


def keep_pictures(args, name, build, result):
    """Small PNG evidence: degraded pages (hayro | poppler) and the worst display-list page."""
    rows = result["by_dpi"][150]["pages"]
    picks = [r for r in rows if r["path"] != "display-list"]
    exact = [r for r in rows if r["path"] == "display-list"]
    if exact:
        picks.append(max(exact, key=lambda r: r["dl_vs_splash"]["unmatched_pct"]))
    for row in picks:
        n, out = row["page"], build / "dpi150"
        via_dl = row["path"] == "display-list"
        first = out / "dl" / f"page-{n}.pgm" if via_dl else out / f"hayro-{n}.pgm"
        target = args.evidence / f"e4-{name}-page{n}-{'dl' if via_dl else 'hayro'}-vs-poppler.png"
        subprocess.run(["magick", str(first), str(out / f"splash-{n}.pgm"), "+append", "-resize", "50%",
                        "-depth", "4", str(target)], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--clone", type=Path, default=CLONE_DEFAULT)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--run-id", default="e4")
    parser.add_argument("--fixtures", default="book10,mixed,ctex,luatexja,article,tikz")
    args = parser.parse_args()
    args.clone, args.evidence = args.clone.resolve(), args.evidence.resolve()
    args.evidence.mkdir(parents=True, exist_ok=True)
    args.work = HERE / ".work" / args.run_id
    args.work.mkdir(parents=True, exist_ok=False)
    args.cache = args.work / "font-cache"
    results = []
    for name in args.fixtures.split(","):
        try:
            results.append(evaluate(args, name))
        except Exception as error:  # recorded, the other fixtures still run
            results.append({"fixture": name, "error": repr(error), "by_dpi": {}})
        brief = {d: {k: v for k, v in r.items() if k != "pages"} for d, r in results[-1]["by_dpi"].items()}
        print(name, json.dumps(brief), flush=True)
    (args.evidence / "e4-summary.json").write_text(json.dumps(results, indent=1, ensure_ascii=False) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
