#!/usr/bin/env python3
"""M6 follow-up: type while invalid, repair, export, and compare authoritative bytes."""

import argparse
import json
from pathlib import Path
import unicodedata

from evaluate import verify
from runtime import (BASE, PATCHED, ROOT, converged, drain, exchange, math_offset,
                     start, write_json)
from source import fixtures


def export(session, build, expected):
    session.send({"cmd": "export_pdf", "out": "/work/restored.pdf"})
    event = session.wait(lambda e: e.get("event") == "PdfExported", 30)
    snapshot = build / "build/export-src/main.tex"
    return {"event": event, "source_equals_expected": snapshot.read_bytes() == expected.encode()}


def measure(args, font, variant):
    source, tagged, _, cases = fixtures(font)
    text = tagged if variant == "A" else source
    clone = PATCHED if variant == "A" else BASE
    project = args.work / "projects" / f"{font}-{variant}"
    project.mkdir(parents=True)
    (project / "main.tex").write_text(text)
    name = f"{font}-{variant}-recovery"
    result = {"final_patch_verify": verify(args, clone, project, f"{name}-verify")}
    session, build = start(args, clone, project, name)
    try:
        converged(session, build)
        offset = math_offset(text, cases[0])
        states = []
        for kind, prefix in (("unbalanced-brace", "{"), ("undefined-macro", "\\scholiumUndefined ")):
            bad, update = exchange(session, offset, offset, prefix)
            during = drain(session, 3)
            typed, _ = exchange(session, offset + len(prefix), offset + len(prefix), "q")
            during += drain(session, 2)
            repair, updated = exchange(session, offset, offset + len(prefix), "")
            glyphs = []
            if updated and updated.get("dl"):
                glyphs = [g for line in updated["dl"]["lines"] for g in line["items"] if g[0] == "g"]
            seen = any(unicodedata.normalize("NFKC", chr(g[2])) == "q" for g in glyphs)
            expected = text.encode()[:offset] + b"q" + text.encode()[offset:]
            repaired_export = export(session, build, expected.decode())
            restore, _ = exchange(session, offset, offset + 1, "")
            restored = export(session, build, text)
            states.append({"state": kind, "bad": bad, "typed_while_invalid": typed,
                           "error_events": during, "repair": repair, "q_in_live_update": seen,
                           "repaired_export": repaired_export, "restore": restore, "restored_export": restored})
        result["states"] = states
    finally:
        session.close()
    return result


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
        for variant in ("base", "A"):
            results[f"{font}-{variant}"] = measure(args, font, variant)
            write_json(args.evidence / "recovery-summary.json", results)
            print("done", font, variant, flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
