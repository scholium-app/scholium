#!/usr/bin/env python3
"""E5 location: what source ↔ page mapping the rtex events support, and how precise it is.

rtex exposes spans (unit id ↔ byte range, `spans` reply) and, in page display lists, each row's
unit id and row index with positioned glyphs (`LayoutUpdate`; a `ParagraphUpdate` places a
unit's rows the same way). There is no per-glyph source offset. Three levels are measured:

- unit: page point → row → unit → byte range; byte → unit → union of its row boxes;
- row: as above, plus a proportional guess of the row inside the unit (row k of R ↔ bytes);
- glyph: a host-side alignment of the unit's glyph characters with its source text
  (difflib), the heuristic a host would have to add for character-level caret mapping.

Ground truth: the unique marker words `QZxx` (gen_fixtures; injected into the gen-book book):
their source bytes are known and their glyphs are found on the converged pages.
"""

import argparse
import difflib
import json
from pathlib import Path
import re
import subprocess

from e4_draw import converged_pages
from gen_fixtures import markers, write as generate
from measure import CLONE_DEFAULT, HERE, percentiles, warm_font_cache
from session import MB, launch

SP_PER_PT = 65536.0
SKIP_SOURCE = set(" \t\n~{}$\\&")


def book_with_markers(args, project):
    subprocess.run([str(args.clone / "target/release/rtex"), "gen-book", "--pages", "10", "--out", str(project)],
                   check=True, capture_output=True)
    text = (project / "main.tex").read_text()
    head, body = text.split("\\begin{document}\n", 1)
    names, blocks = markers(), body.split("\n\n")
    for i, block in enumerate(blocks):
        if re.match(r"[A-Z][a-z]+ ", block):
            first = block.index(" ")
            blocks[i] = block[:first] + f" {next(names)}" + block[first:]
    (project / "main.tex").write_text(head + "\\begin{document}\n" + "\n\n".join(blocks))


def glyph_text(dl, item):
    font = dl["fonts"].get(str(item[1]), {}) if isinstance(dl["fonts"], dict) else {}
    if not font.get("filename") or not 0 < item[2] < 0x110000:
        return "\ufffd"  # TFM (math) fonts: char codes are not Unicode
    return chr(item[2])


def page_rows(pages, span_of):
    """span id → rows (page, line dict, glyph list) in reading order.

    Page-list lines carry the layout unit uid; the `units` reply maps uid → span id.
    """
    units = {}
    for page in pages:
        dl = page["dl"]
        for line in dl["lines"]:
            glyphs = [{"page": page["page"], "x": it[4], "y": it[5], "adv": it[6], "ch": glyph_text(dl, it)}
                      for it in line["items"] if it[0] == "g"]
            span = span_of.get(line.get("unit"))
            units.setdefault(span, []).append({"page": page["page"], "line": line, "glyphs": glyphs})
    return units


def placement_agreement(units, placements):
    """Rows whose (page, baseline) also appears in the layout's placements for the same span."""
    known = {(p["par_id"], f["page"], b) for p in placements for f in p["fragments"] for b in f["baselines"]}
    rows = [(span, row["page"], row["line"]["y"]) for span, rs in units.items() if span for row in rs]
    return {"rows": len(rows), "in_placements": sum(1 for r in rows if r in known)}


def find_marker(units, name):
    for unit, rows in units.items():
        for r, row in enumerate(rows):
            text = "".join(g["ch"] for g in row["glyphs"])
            at = text.find(name)
            if at >= 0:
                return unit, r, row["glyphs"][at:at + len(name)]
    return None


def source_chars(source, span):
    data = source.encode()
    start, end = span["range"]["start"], span["range"]["end"]
    chars, offset = [], start
    for ch in data[start:end].decode(errors="ignore"):
        if ch not in SKIP_SOURCE:
            chars.append((ch, offset))
        offset += len(ch.encode())
    return chars


def align(glyphs, chars):
    """glyph index → source byte, and source byte → glyph index, by sequence alignment."""
    matcher = difflib.SequenceMatcher(None, "".join(g["ch"] for g in glyphs), "".join(c for c, _ in chars),
                                      autojunk=False)
    g2s, s2g = {}, {}
    for block in matcher.get_matching_blocks():
        for k in range(block.size):
            g2s[block.a + k] = chars[block.b + k][1]
            s2g[chars[block.b + k][1]] = block.a + k
    return g2s, s2g


def coverage(units, spans, source):
    """How much of every unit the glyph ↔ source alignment pins down exactly (not only markers):
    text glyphs (Unicode fonts) and TFM math glyphs separately."""
    totals = {"text_glyphs": 0, "text_matched": 0, "tfm_glyphs": 0, "tfm_matched": 0}
    for span_id, rows in units.items():
        if span_id not in spans:
            continue
        flat = [g for row in rows for g in row["glyphs"]]
        g2s, _ = align(flat, source_chars(source, spans[span_id]))
        for i, glyph in enumerate(flat):
            kind = "tfm" if glyph["ch"] == "\ufffd" else "text"
            totals[f"{kind}_glyphs"] += 1
            totals[f"{kind}_matched"] += i in g2s
    for kind in ("text", "tfm"):
        totals[f"{kind}_share"] = round(totals[f"{kind}_matched"] / max(1, totals[f"{kind}_glyphs"]), 4)
    return totals


def nearest(mapping, key):
    if key in mapping:
        return mapping[key]
    before = [k for k in mapping if k <= key]
    after = [k for k in mapping if k >= key]
    pick = max(before) if before else (min(after) if after else None)
    return None if pick is None else mapping[pick]


def box(row):
    line = row["line"]
    return (line["x"], line["y"] - line["h"], line["x"] + line["w"], line["y"] + line["d"])


def distance_pt(point, rect):
    x, y = point
    dx = max(rect[0] - x, 0, x - rect[2])
    dy = max(rect[1] - y, 0, y - rect[3])
    return (dx * dx + dy * dy) ** 0.5 / SP_PER_PT


def measure_fixture(args, name):
    project = args.work / "projects" / name
    if name == "book10":
        book_with_markers(args, project)
    else:
        generate(name, 5, project)
    source = (project / "main.tex").read_text()
    run = f"e5-{name}"
    cache = args.work / "font-cache"
    warm_font_cache(cache, project, args.work, args.clone)
    session = launch(args.clone, project, args.work / run, args.evidence, run, True, seconds=600, redact=True,
                     line_limit=256 * MB, total_limit=1024 * MB, extra_env={"SCHOLIUM_RTEX_FONT_CACHE": str(cache)})
    try:
        pages, layout, _, _ = converged_pages(session, args.work / run)
        session.send({"cmd": "spans", "path": "main.tex"})
        spans = {s["id"]: s for s in session.wait(lambda e: e.get("reply") == "spans")["spans"]}
        session.send({"cmd": "units"})
        span_of = {u["uid"]: u["span"] for u in session.wait(lambda e: e.get("reply") == "units")["units"]}
    finally:
        session.close()
    units = page_rows(pages, span_of)
    agreement = placement_agreement(units, layout["placements"])
    samples = []
    for marker in sorted(set(re.findall(r"QZ[a-z]{2}", source))):
        if source.count(marker) != 1:
            continue
        truth_byte = len(source[:source.index(marker)].encode())
        found = find_marker(units, marker)
        if not found:
            samples.append({"marker": marker, "found_on_page": False})
            continue
        unit, row_index, glyphs = found
        for k, glyph in enumerate(glyphs):
            samples.append(sample(units, spans, source, unit, row_index, glyph, truth_byte + k, marker))
    return {"fixture": name, "pages": layout["pages_total"], "spans": len(spans), "rows_vs_placements": agreement,
            "alignment_coverage": coverage(units, spans, source), **score(samples)}, samples


def sample(units, spans, source, unit, row_index, glyph, truth, marker):
    rows, span = units[unit], spans.get(unit)
    point = (glyph["x"] + glyph["adv"] // 2, glyph["y"] - 2 * 65536)
    out = {"marker": marker, "truth_byte": truth, "unit": unit, "row": row_index + 1, "rows": len(rows)}
    if span is None:
        return {**out, "span": None}
    start, end = span["range"]["start"], span["range"]["end"]
    # page → source
    out["unit_hit"] = start <= truth < end
    out["unit_span_bytes"] = end - start
    guess = start + int((row_index + 0.5) / len(rows) * (end - start))
    out["row_guess_err_bytes"] = abs(guess - truth)
    flat = [g for row in rows for g in row["glyphs"]]
    g2s, s2g = align(flat, source_chars(source, span))
    index = next(i for i, g in enumerate(flat) if g is glyph)
    estimate = nearest(g2s, index)
    out["glyph_err_bytes"] = None if estimate is None else abs(estimate - truth)
    # source → page
    rects = [box(row) for row in rows if row["page"] == glyph["page"]]
    out["unit_rect_err_pt"] = round(min(distance_pt(point, r) for r in rects), 3) if rects else None
    row_guess = min(len(rows) - 1, int((truth - start) / max(1, end - start) * len(rows)))
    out["row_guess_same_row"] = row_guess == row_index
    out["row_guess_err_pt"] = round(abs(rows[row_guess]["line"]["y"] - glyph["y"]) / SP_PER_PT, 3) \
        if rows[row_guess]["page"] == glyph["page"] else None
    target = nearest(s2g, truth)
    if target is None:
        out["glyph_caret_err_pt"] = None
    else:
        hit = flat[target]
        out["glyph_caret_err_pt"] = round(((hit["x"] - glyph["x"]) ** 2 + (hit["y"] - glyph["y"]) ** 2) ** 0.5
                                          / SP_PER_PT, 3) if hit["page"] == glyph["page"] else None
    return out


def score(samples):
    found = [s for s in samples if "unit_hit" in s]
    rate = lambda xs: round(sum(1 for x in xs if x) / max(1, len(xs)), 3)
    nums = lambda key: [s[key] for s in found if s.get(key) is not None]
    return {
        "markers": len({s["marker"] for s in samples}),
        "markers_not_found": sorted({s["marker"] for s in samples if s.get("found_on_page") is False}),
        "samples": len(found),
        "click_unit_hit_rate": rate([s["unit_hit"] for s in found]),
        "unit_span_bytes": percentiles(nums("unit_span_bytes")),
        "click_row_guess_err_bytes": percentiles(nums("row_guess_err_bytes")),
        "click_glyph_exact_rate": rate([s.get("glyph_err_bytes") == 0 for s in found]),
        "click_glyph_err_bytes": percentiles(nums("glyph_err_bytes")),
        "caret_unit_rect_hit_rate": rate([s.get("unit_rect_err_pt") == 0 for s in found]),
        "caret_row_guess_same_row_rate": rate([s["row_guess_same_row"] for s in found]),
        "caret_row_guess_err_pt": percentiles(nums("row_guess_err_pt")),
        "caret_glyph_exact_rate": rate([s.get("glyph_caret_err_pt") == 0 for s in found]),
        "caret_glyph_err_pt": percentiles(nums("glyph_caret_err_pt")),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--clone", type=Path, default=CLONE_DEFAULT)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--run-id", default="e5")
    args = parser.parse_args()
    args.clone, args.evidence = args.clone.resolve(), args.evidence.resolve()
    args.evidence.mkdir(parents=True, exist_ok=True)
    args.work = HERE / ".work" / args.run_id
    args.work.mkdir(parents=True, exist_ok=False)
    summary = []
    for name in ("book10", "mixed"):
        result, samples = measure_fixture(args, name)
        summary.append(result)
        (args.evidence / f"e5-{name}-samples.json").write_text(json.dumps(samples, indent=1) + "\n")
        print(json.dumps(result), flush=True)
    (args.evidence / "e5-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
