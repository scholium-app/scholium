#!/usr/bin/env python3
"""T22 M1–M7. Real sandbox evidence; acceptance failures are data, not harness errors."""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

from alignment import score
from caret import Caret, audit
from runtime import (BASE, PATCHED, ROOT, converged, exchange, invalid_states, latency,
                     math_offset, sandbox, start, write_json)
from source import Parser, fixtures


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def glyph_rows(pages):
    rows = []
    for page in pages:
        dl = page["dl"]
        for line in dl["lines"]:
            for item in line["items"]:
                if item[0] == "g":
                    font = dl["fonts"][str(item[1])]
                    rows.append((font.get("name"), font.get("size"), *item[2:]))
                elif item[0] == "r":
                    rows.append(tuple(item))
    return rows


def merge_geometry(pages):
    geometry, fonts = {"glyphs": [], "nodes": {}}, {}
    for page in pages:
        dl = page["dl"]
        metadata = dl.get("math_edit") or {}
        geometry["glyphs"].extend(metadata.get("glyphs", []))
        geometry["nodes"].update(metadata.get("nodes", {}))
        fonts.update(dl["fonts"])
    return geometry, fonts


def verify(args, clone, project, name):
    result, work = sandbox(args, clone, project, name,
                           ["/rtex/bin/rtex", "verify", "--project", "/project", "--build", "/work/verify",
                            "--json-out", "/work/verify.json"])
    if (work / "verify.json").exists():
        data = json.loads((work / "verify.json").read_text())
        write_json(args.evidence / f"{name}.json", data)
        result.update({k: data[k] for k in ("paragraphs_total", "paragraphs_eligible", "ineligible_reasons",
                                           "layer1_pass", "layer2_pass", "capture_pdf_equals_clean")})
        result["glyphs_identical"] = sum(p["glyphs_identical"] for p in data["paragraphs"])
        result["glyphs_total"] = sum(p["glyphs_total"] for p in data["paragraphs"])
    return result


def full_compile(args, clone, project, name):
    result, work = sandbox(args, clone, project, name,
                           ["/usr/bin/lualatex", "-interaction=nonstopmode", "-halt-on-error",
                            "-output-directory=/work", "/project/main.tex"])
    if (work / "main.pdf").exists():
        # Exact full-page pixel equality, independent of PDF timestamp/ID metadata.
        subprocess.run(["pdftoppm", "-r", "300", "-gray", "-singlefile", str(work / "main.pdf"),
                        str(work / "page")], check=True, capture_output=True)
        result["page_sha256"] = digest(work / "page.pgm")
        shutil.copyfile(work / "main.pdf", args.evidence / f"{name}.pdf")
    return result


def post_edit(session, source, nodes, case):
    offset = math_offset(source, case)
    record, update = exchange(session, offset, offset + 1, "z")
    result = {"edit": record}
    if update and update.get("dl"):
        geometry = update["dl"].get("math_edit") or {}
        first = next(n for n in nodes if n["parent"] == case["root"] and n.get("char") == "x")
        glyph = next((g for g in geometry.get("glyphs", []) if g["source_id"] == first["id"] and
                      g["char"] == ord("z")), None)
        result["z_owned_by_original_atom"] = bool(glyph)
        if glyph:
            b = glyph["bbox"]
            actual = Caret(nodes, geometry).click((b[0] + (b[2] - b[0]) * 0.75, (b[1] + b[3]) / 2))
            result.update(expected=first["stop"], actual=actual, passed=actual == first["stop"])
    result["restore"], _ = exchange(session, offset, offset + 1, "x")
    return result


def structure_report(nodes, geometry):
    missing = [n for n in nodes if str(n["id"]) not in geometry["nodes"]]
    empty = [n for n in nodes if n["start"] == n["stop"]]
    noads = [n for n in geometry["nodes"].values() if n.get("noads")]
    return {"positions": len(nodes), "with_bbox": len(nodes) - len(missing), "missing": missing,
            "empty_slots": len(empty), "empty_with_bbox": sum(str(n["id"]) in geometry["nodes"] for n in empty),
            "nodes_with_engine_noad_observation": len(noads),
            "kinds": dict(Counter(n["kind"] for n in nodes))}


def run_variant(args, font, tagged):
    source, instrumented, nodes, cases = fixtures(font)
    name = f"{font}-{'A' if tagged else 'base'}"
    clone = PATCHED if tagged else BASE
    project = args.work / "projects" / name
    project.mkdir(parents=True)
    text = instrumented if tagged else source
    (project / "main.tex").write_text(text)
    write_json(args.evidence / f"{font}-source-map.json", {"nodes": nodes, "cases": cases})
    result = {"verify": verify(args, clone, project, f"{name}-verify"),
              "full_compile": full_compile(args, clone, project, f"{name}-full")}
    session, build = start(args, clone, project, f"{name}-live")
    try:
        pages, layout, pdf = converged(session, build)
        write_json(args.evidence / f"{name}-pages.json", pages)
        result["layout"] = {"pages": layout["pages_total"], "compile": layout["compile"]}
        if tagged:
            geometry, fonts = merge_geometry(pages)
            baseline_pages = json.loads((args.evidence / f"{font}-base-pages.json").read_text())
            result["mapping"] = score(nodes, cases, geometry, fonts, baseline_pages)
            result["structure"] = structure_report(nodes, geometry)
            result["caret"], checks = audit(nodes, geometry)
            write_json(args.evidence / f"{font}-caret-checks.json", checks)
            result["post_edit"] = post_edit(session, text, nodes, cases[0])
            result["cjk"] = [g for g in geometry["glyphs"] if g["char"] in map(ord, "中文")]
        if args.live:
            result["latency"], records = latency(session, text, cases)
            write_json(args.evidence / f"{name}-keystrokes.json", records)
            result["invalid_states"] = invalid_states(session, text, cases[0])
        # Explicitly verify the authoritative rtex buffer after all edits were repaired.
        # export flushes the current source snapshot to a background compilation.
        session.send({"cmd": "export_pdf", "out": "/work/restored.pdf"})
        result["export_reply"] = session.wait(lambda e: e.get("event") == "PdfExported", 30)
        snapshot = build / "build/export-src/main.tex"
        result["snapshot_equals_initial"] = snapshot.exists() and snapshot.read_text() == text
    finally:
        session.close()
        result["session_exit"] = session.process.returncode
        result["peak_bytes"] = session.peak_bytes
    return result, pages


def intermediate(args, font):
    groups = {"fraction": ["x", r"\frac{}{}", r"\frac{x}{}", r"\frac{x}{y}"],
              "script": ["x", "x^{}", "x^{2}", "x^{2}_{}", "x^{2}_{i}"],
              "sqrt": ["x", r"\sqrt{}", r"\sqrt{x}", r"\sqrt[3]{x}"],
              "matrix": [r"\begin{pmatrix}a\end{pmatrix}",
                         r"\begin{pmatrix}a\\\end{pmatrix}",
                         r"\begin{pmatrix}a\\b\end{pmatrix}"]}
    source, _, _, _ = fixtures(font)
    preamble = source.split(r"\begin{document}")[0]
    states = [v for values in groups.values() for v in values]
    states.append(r"\alpha")
    # All visible states are independent complete formulas in one compile. A malformed
    # state fails the compile; the host issues each structural transaction atomically.
    for state in states:
        Parser(state).parse()
    project = args.work / "projects" / f"{font}-intermediate"
    project.mkdir(parents=True)
    (project / "main.tex").write_text(preamble + r"\begin{document}" + "\n" +
                                     "\n\n".join(f"${s}$" for s in states) + "\n" + r"\end{document}")
    buffered, sent = "", []
    for token in ("\\", "a", "l", "p", "h", "a"):
        buffered += token
        if buffered == r"\alpha":
            sent.append(buffered)
    assert sent == [r"\alpha"]
    result = full_compile(args, PATCHED, project, f"{font}-intermediate")
    return {**result, "states": groups, "state_count": len(states),
            "command_typing": {"buffered_prefix": r"\al", "engine_inputs": sent}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--live", action="store_true", help="also run M5 latency and M6 fault injections")
    args = parser.parse_args()
    args.evidence = args.evidence.resolve()
    args.evidence.mkdir(parents=True, exist_ok=True)
    args.work = Path(__file__).resolve().parent / ".work" / args.run_id
    args.work.mkdir(parents=True, exist_ok=False)
    args.cache = args.work.parent / "font-cache"
    args.cache.mkdir(parents=True, exist_ok=True)
    summary = {}
    for font in ("cm", "unicode"):
        source, _, _, _ = fixtures(font)
        warm = args.work / "projects" / f"{font}-warm"
        warm.mkdir(parents=True)
        (warm / "main.tex").write_text(source)
        warm_result, _ = sandbox(args, PATCHED, warm, f"{font}-warm",
                                 ["/usr/bin/lualatex", "-interaction=nonstopmode", "-halt-on-error",
                                  "-output-directory=/work", "/project/main.tex"])
        assert warm_result["exit"] == 0, warm_result
        a, b = {}, {}
        for tagged in (False, True):
            name = "A" if tagged else "base"
            print(f"start {font} {name}", flush=True)
            result, pages = run_variant(args, font, tagged)
            summary.setdefault(font, {})[name] = result
            if tagged:
                a = pages
            else:
                b = pages
            write_json(args.evidence / "summary.json", summary)
            print(f"done {font} {name}", flush=True)
        summary[font]["identity"] = {
            "full_compile_pixels_equal": summary[font]["A"]["full_compile"].get("page_sha256") ==
                                         summary[font]["base"]["full_compile"].get("page_sha256"),
            "all_glyphs_and_rules_equal": glyph_rows(a) == glyph_rows(b),
            "glyphs_and_rules": len(glyph_rows(a))}
        summary[font]["intermediate"] = intermediate(args, font)
        write_json(args.evidence / "summary.json", summary)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
