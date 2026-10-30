#!/usr/bin/env python3
"""Compare edited long-lived sandbox and host sessions using PDF text and pages."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

from session import launch


def pdf_facts(pdf):
    info = subprocess.check_output(["pdfinfo", str(pdf)], text=True)
    text = subprocess.check_output(["pdftotext", "-layout", str(pdf), "-"], text=True)
    pages = int(next(line.split(":", 1)[1] for line in info.splitlines() if line.startswith("Pages:")))
    return {"pages": pages, "text_sha256": hashlib.sha256(text.encode()).hexdigest(),
            "has_edit": "EVALCHANGE" in text, "unresolved_reference": "??" in text}


def evaluate(clone, project, work, evidence, fixture, sandbox):
    name = f"positive-{fixture}-{'sandbox' if sandbox else 'host'}"
    build = work / name
    session = launch(clone, project, build, evidence, name, sandbox)
    result = {"name": name}
    try:
        layout = session.wait(lambda e: e.get("event") == "LayoutUpdate")
        result["initial_pages"] = layout["pages_total"]
        assert layout["compile"]["state"] == "Ok", layout["compile"]
        text = (project / "main.tex").read_bytes()
        needle = b"Baseline" if fixture == "book" else b"Scholium"
        offset = text.index(needle) + len(needle)
        session.send({"cmd": "edit", "start": offset, "end": offset, "text": " EVALCHANGE"})
        reply = session.wait(lambda e: e.get("reply") == "edit")
        result["routing"] = reply["result"]["routed"]
        update = session.wait(lambda e: e.get("event") == "ParagraphUpdate")
        result["paragraph_status"] = update["status"]
        assert update["status"] == "ok", update["status"]
        assert update["versions"]["source_revision"] == reply["result"]["source_revision"]
        out = "/work/export.pdf" if sandbox else str(build / "export.pdf")
        session.send({"cmd": "export_pdf", "out": out})
        export = session.wait(lambda e: e.get("event") == "PdfExported")
        result["export"] = export
        assert export["status"]["state"] == "Ok" and export["converged"], export
        result["pdf"] = pdf_facts(build / "export.pdf")
        (evidence / f"{name}.txt").write_text(subprocess.check_output(
            ["pdftotext", "-layout", str(build / "export.pdf"), "-"], text=True))
        assert result["pdf"]["has_edit"]
        assert not result["pdf"]["unresolved_reference"]
        result["passed"] = True
    except Exception as error:
        result.update(passed=False, error=str(error))
    finally:
        result["events"] = session.events
        session.close()
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--clone", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--run-id", default="positive")
    args = parser.parse_args()
    base = Path(__file__).resolve().parent / ".work"
    work = base / args.run_id
    work.mkdir(exist_ok=False)
    evidence = args.evidence.resolve()
    article = work / "article"
    article.mkdir(exist_ok=True)
    shutil.copyfile(Path(__file__).parent / "fixtures/article.tex", article / "main.tex")
    results = []
    for name, project in (("book", base / "smoke/book-10"), ("article", article)):
        pair = [evaluate(args.clone.resolve(), project, work, evidence, name, sandbox)
                for sandbox in (False, True)]
        passed = all(r["passed"] for r in pair) and pair[0].get("pdf") == pair[1].get("pdf")
        results.append({"fixture": name, "passed": passed, "runs": pair})
        print(json.dumps(results[-1], ensure_ascii=False), flush=True)
    (evidence / "positive-summary.json").write_text(json.dumps(results, indent=2) + "\n")
    return 0 if all(r["passed"] for r in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
