#!/usr/bin/env python3
"""Score the independent unpatched glyph stream and attach recovery/counterexample evidence."""

import argparse
import json

from alignment import score
from caret import Caret
from evaluate import merge_geometry, structure_report
from runtime import write_json
from source import fixtures


def fallback(nodes, cases, geometry, mapping):
    by_id = {n["id"]: n for n in nodes}
    result = {"glyphs": [], "nodes": {}}
    truth_clicks = []
    for case, scored in zip(cases, mapping["details"]):
        glyphs = [g for g in geometry["glyphs"] if g["source_id"] in case["nodes"]]
        for glyph, row in zip(glyphs, scored["rows"]):
            truth = by_id[glyph["source_id"]]
            if truth.get("char"):
                b = glyph["bbox"]
                for ratio, expected in ((0.25, truth["start"]), (0.75, truth["stop"])):
                    truth_clicks.append(((b[0] + ratio * (b[2] - b[0]), (b[1] + b[3]) / 2), expected))
            ident = row["B_id"]
            if ident is None:
                continue
            result["glyphs"].append({**glyph, "source_id": ident})
            while ident:
                node = by_id[ident]
                key = str(ident)
                b = glyph["bbox"]
                if key not in result["nodes"]:
                    result["nodes"][key] = {**node, "bbox": list(b)}
                else:
                    box = result["nodes"][key]["bbox"]
                    box[:] = [min(box[0], b[0]), min(box[1], b[1]), max(box[2], b[2]), max(box[3], b[3])]
                ident = node["parent"]
    caret = Caret(nodes, result)
    clicks = [{"expected": expected, "actual": caret.click(point)} for point, expected in truth_clicks]
    return {"structure": structure_report(nodes, result), "clicks": len(clicks),
            "clicks_exact": sum(c["expected"] == c["actual"] for c in clicks),
            "click_failures": [c for c in clicks if c["expected"] != c["actual"]]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True)
    args = parser.parse_args()
    from pathlib import Path
    evidence = Path(args.evidence)
    summary = json.loads((evidence / "summary.json").read_text())
    recovery = json.loads((evidence / "recovery-summary.json").read_text())
    structural = json.loads((evidence / "structural-edit-summary.json").read_text())
    for font in ("cm", "unicode"):
        _, _, nodes, cases = fixtures(font)
        geometry, fonts = merge_geometry(json.loads((evidence / f"{font}-A-pages.json").read_text()))
        mapping = score(nodes, cases, geometry, fonts,
                        json.loads((evidence / f"{font}-base-pages.json").read_text()))
        summary[font]["A"]["mapping"] = mapping
        summary[font]["B"] = fallback(nodes, cases, geometry, mapping)
        summary[font]["structural_edit"] = structural[font]
        for variant in ("base", "A"):
            record = summary[font][variant]
            # The early probe used an unsupported `export` command and observed a stale
            # background snapshot. The dedicated export_pdf follow-up supersedes it.
            for field in ("export_reply", "snapshot_equals_initial", "invalid_states"):
                record.pop(field, None)
            record["recovery"] = recovery[f"{font}-{variant}"]
    write_json(evidence / "summary.json", summary)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
