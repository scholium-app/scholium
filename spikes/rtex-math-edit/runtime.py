"""Bounded sandbox runs and E6-compatible math keystroke measurements."""

import json
from pathlib import Path
import shutil
import sys
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / "spikes/rtex-eval"))
from measure import Raster, percentiles, run_sandboxed, summarize_keystrokes  # noqa: E402
from session import launch  # noqa: E402

BASE = Path.home() / ".cache/scholium-eval/rtex-v0.0.5"
PATCHED = Path.home() / ".cache/scholium-eval/rtex-math-edit"


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def sandbox(args, clone, project, name, argv):
    work = args.work / name
    status, wall = run_sandboxed(project, work, 300, argv, clone=clone,
                                 extra_env={"SCHOLIUM_RTEX_FONT_CACHE": str(args.cache)},
                                 log=args.evidence / f"{name}.log")
    return {"exit": status, "wall_s": wall}, work


def start(args, clone, project, name):
    build = args.work / name
    session = launch(clone, project, build, args.evidence, name, True, seconds=600,
                     extra_env={"SCHOLIUM_RTEX_FONT_CACHE": str(args.cache)})
    return session, build


def converged(session, build):
    pages = {}
    for _ in range(8):
        layout = session.wait(lambda e: e.get("event") == "LayoutUpdate", 45)
        if layout["compile"]["state"] != "Ok":
            raise RuntimeError(f"layout failed: {layout}")
        for page in layout["pages_changed"]:
            pages[page["page"]] = page
        if layout["convergence"]["state"] == "Converged":
            version = layout["versions"]["layout_version"]
            pdf = build / "build/bg" / f"layout-{version}.pdf"
            return list(pages.values()), layout, pdf
    raise RuntimeError("layout did not converge")


def exchange(session, start, end, text, timeout=12):
    sent = session.send({"cmd": "edit", "path": "main.tex", "start": start, "end": end, "text": text})
    reply = session.wait(lambda e: e.get("reply") in ("edit", "error"), timeout)
    reply_ms = (session.received_at - sent) * 1000
    if reply["reply"] == "error":
        return {"error": reply, "reply_ms": reply_ms}, None
    result = reply["result"]
    edit_id = result["edit_id"]
    record = {"routed": result["routed"], "reasons": result["reasons"], "edit_id": edit_id,
              "reply_ms": round(reply_ms, 3)}
    try:
        update = session.wait(lambda e: (e.get("event") == "ParagraphUpdate" and
                                        e.get("edit_id") == edit_id) or
                              (e.get("event") == "BackgroundScheduled" and
                               e.get("par_id") in result["outcome"]["touched"]), timeout)
        record.update(update=update["event"], update_ms=round((session.received_at - sent) * 1000, 3),
                      status=update.get("status"), timing=update.get("timing"))
        return record, update
    except TimeoutError:
        return {**record, "update": "timeout"}, None


def math_offset(source, case):
    marker = f"Case {case['name']}: "
    start = source.index(marker) + len(marker)
    # Insert at a nucleus, after the generated attribute assignment, never into its ID.
    value = {"inline": "x", "fraction": "a", "scripts": "x", "radical": "x"}[case["name"]]
    if case["name"] == "radical":
        start = source.index("\\sqrt", start)
    if "\\attribute" in source[start:source.index("\n\n", start)]:
        import re
        match = re.search(r"\\attribute65000=\d+ " + value, source[start:])
        return len(source[:start + match.end() - 1].encode())
    if case["name"] == "fraction":
        start = source.index(r"\frac{", start) + len(r"\frac{")
    return len(source[:source.index(value, start)].encode())


def latency(session, source, cases):
    result, raw = {}, []
    raster = Raster(dpi=300)
    try:
        for case in cases[:4]:
            offset = math_offset(source, case)
            assert source.encode()[offset:offset + 1] in (b"x", b"a"), "edit must target a nucleus"
            for start_at, end_at, text in ((offset, offset, "x"), (offset, offset + 1, "")):
                _, warm = exchange(session, start_at, end_at, text)
                if warm and warm.get("dl"):
                    raster.draw(warm)
            rows = []
            for pattern in ("steady", "burst"):
                part = []
                for i in range(20):
                    record, update = exchange(session, offset, offset + (i % 2), "" if i % 2 else "x")
                    record.update(case=case["name"], mode=case["mode"], pattern=pattern, i=i)
                    if update and update["event"] == "ParagraphUpdate" and record.get("status") == "ok":
                        drawn, done = raster.draw(update)
                        # exchange timestamp is receipt minus update_ms, matching E6's clock.
                        sent = session.received_at - record["update_ms"] / 1000
                        record.update(raster_ms=round((done - sent) * 1000, 3),
                                      raster_us=drawn.get("raster_us"),
                                      raster_glyphs_skipped=drawn.get("stats", {}).get("glyphs_skipped"))
                    part.append(record)
                    time.sleep(0.15 if pattern == "steady" else 0.03)
                    if pattern == "burst" and i % 5 == 4:
                        time.sleep(1.5)
                result[f"{case['mode']}-{pattern}"] = summarize_keystrokes(part)
                rows.extend(part)
            raw.extend(rows)
    finally:
        raster.close()
    return result, raw


def drain(session, seconds):
    messages = []
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            message = session.wait(lambda _: True, deadline - time.monotonic())
            summary = {k: v for k, v in message.items() if k not in ("dl", "placements", "pages_changed")}
            if "pages_changed" in message:
                summary["pages_changed_count"] = len(message["pages_changed"])
            messages.append(summary)
        except TimeoutError:
            break
    return messages


def invalid_states(session, source, case):
    offset = math_offset(source, case)
    results = []
    for name, text in (("unbalanced-brace", "{"), ("undefined-macro", r"\scholiumUndefined")):
        bad, update = exchange(session, offset, offset, text)
        during = drain(session, 3)
        sent = time.monotonic()
        good, recovered = exchange(session, offset, offset + len(text.encode()), "")
        after = drain(session, 2)
        results.append({"state": name, "bad": bad, "bad_dl_present": bool(update and update.get("dl")),
                        "during": during, "repair": good,
                        "recovered_live_ms": good.get("update_ms"), "after": after,
                        "repair_observation_ms": round((time.monotonic() - sent) * 1000, 3)})
    return results
