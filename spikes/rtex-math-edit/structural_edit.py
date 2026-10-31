#!/usr/bin/env python3
"""M4 counterexample: inserting a nonempty fraction needs fresh ownership/slot IDs."""

import argparse
from pathlib import Path
import unicodedata

from caret import Caret
from recovery import export
from runtime import PATCHED, converged, exchange, math_offset, start, write_json
from source import fixtures


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--evidence", required=True, type=Path)
    args = parser.parse_args()
    args.evidence = args.evidence.resolve()
    args.work = Path(__file__).resolve().parent / ".work" / args.run_id
    args.work.mkdir(parents=True, exist_ok=False)
    args.cache = args.work.parent / "font-cache"
    results = {}
    for font in ("cm", "unicode"):
        _, tagged, nodes, cases = fixtures(font)
        project = args.work / "projects" / font
        project.mkdir(parents=True)
        (project / "main.tex").write_text(tagged)
        session, build = start(args, PATCHED, project, f"{font}-structural-edit")
        try:
            converged(session, build)
            offset = math_offset(tagged, cases[0])
            replacement = r"\frac{a}{b}"
            record, update = exchange(session, offset, offset + 1, replacement)
            geometry = (update or {}).get("dl", {}).get("math_edit") or {}
            glyph = next((g for g in geometry.get("glyphs", [])
                          if unicodedata.normalize("NFKC", chr(g["char"])) == "b"), None)
            actual = None
            if glyph:
                b = glyph["bbox"]
                actual = Caret(nodes, geometry).click((b[0] + (b[2] - b[0]) * 0.75, (b[1] + b[3]) / 2))
            # Expected offset in the uninstrumented authoritative source after replacement.
            expected = cases[0]["start"] + replacement.index("b") + 1
            result = {"edit": record, "expected_caret": expected, "actual_caret": actual,
                      "passed": actual == expected, "denominator_glyph": glyph,
                      "new_num_den_slots": [n for n in geometry.get("nodes", {}).values()
                                            if n["kind"] in ("num", "den")]}
            result["restore"], _ = exchange(session, offset, offset + len(replacement), "x")
            result["restored_export"] = export(session, build, tagged)
            results[font] = result
            write_json(args.evidence / "structural-edit-summary.json", results)
        finally:
            session.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
