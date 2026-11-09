#!/usr/bin/env python3
"""L1 exit against the bundled real rtex and TeX Live in a private Xvfb window."""
import json
import math
import os
from pathlib import Path
import signal
import sys
import time

from latex_exit_window import Window

from log_reader import log_entries

binary, out = map(lambda p: Path(p).resolve(), sys.argv[1:3])
out.mkdir(parents=True, exist_ok=True)
assert not any(out.iterdir()), "output must be empty"
assert os.environ.get("DISPLAY") and os.environ["DISPLAY"] != ":0", "private Xvfb required"
fixture = (Path(__file__).parent / "latex-exit/paper.tex").read_text()
results = {}


def record(name, **details):
    results[name] = {"pass": True, **details}
    (out / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
    print("PASS " + name, flush=True)


def current_event(window, revision, paragraph):
    return any(s["shown_revision"] == revision and s["paragraph"] == paragraph
               for s in window.events)


def paper(engine):
    source = fixture.replace("program = lualatex", "program = " + engine)
    window = Window(binary, out / engine, source)
    try:
        initial = window.ready(full=True)
        assert initial["page_count"] >= 3 and not initial["toolchain_unavailable"]
        assert "Full build" in initial["status"] and initial["elapsed_ms"] > 0
        if engine == "xelatex":
            assert initial["origin"] == ["FullCompilation"] * initial["pages"]
        window.settled_pixels()
        ui_labels = [s["chrome"]["facts"]["status"]["engine_state"]
                     for s in log_entries(window.log, "PROJECT ")]
        ui_label = ui_labels[-1]
        assert ("Full build" if os.environ.get("SCHOLIUM_LOCALE") == "en" else "整编") in ui_label
        record(engine + "-open", page_count=initial["page_count"], status=initial["status"],
               localized_ui_label=ui_label,
               origin=initial["origin"], elapsed_ms=initial["elapsed_ms"])
        initial_pixels = window.settled_pixels()
        window.body()
        old = window.state()["revision"]
        window.key("x")
        changed = window.ready(old + 1)
        body_revision = changed["revision"]
        window.ready(body_revision, full=True)
        if engine == "lualatex":
            assert current_event(window, body_revision, True), "body paragraph update absent"
            assert current_event(window, body_revision, False), "body full build absent"
        body_pixels = window.settled_pixels(unlike=initial_pixels, timeout=15)
        assert body_pixels != initial_pixels, "body edit did not alter preview pixels"
        # Replace a formula through the same native clipboard event path.
        text = window.state()["text"].replace("x^2+1", "x^3+2")
        revision = window.replace(text)
        window.ready(revision)
        window.ready(revision, full=True)
        if engine == "lualatex":
            assert current_event(window, revision, True), "formula paragraph update absent"
        formula_pixels = window.settled_pixels(unlike=body_pixels, timeout=15)
        assert formula_pixels != body_pixels, "formula edit did not alter preview pixels"
        record(engine + "-edit", body_revision=body_revision, formula_revision=revision,
               body_pixels=body_pixels, formula_pixels=formula_pixels,
               paragraph=engine == "lualatex", full_revision=window.state()["shown_revision"])
        window.key("ctrl+s")
        saved = window.wait(lambda s: not s["dirty"], "save")
        assert window.path.read_text() == saved["text"]
        pixels = window.settled_pixels()
        window.snapshot("saved.png")
        window.kill()
        window.start()
        recovered = window.ready(full=True)
        assert recovered["text"] == saved["text"] and not recovered["dirty"]
        recovered_pixels = window.settled_pixels()
        window.snapshot("recovered.png")
        assert recovered_pixels == pixels, ("saved/recovered preview differs", pixels, recovered_pixels)
        record(engine + "-recovery", source_equal=True, preview_sha256=pixels,
               page_count=recovered["page_count"])
        if engine == "lualatex":
            fault(window)
    finally:
        window.close()


def fault(window):
    before = window.state()
    rtex = [pid for pid, name in window.descendants() if name == "rtex"]
    assert len(rtex) == 1, ("real rtex descendant", rtex)
    os.kill(rtex[0], signal.SIGKILL)
    window.body()
    window.key("y")
    window.wait(lambda s: any(e["engine_state"] == "Recovering" for e in window.events), "Restarting UI")
    state = window.ready(before["revision"] + 1, full=True)
    assert state["generation"] > before["generation"]
    assert any("Restarting" in s["status"] for s in window.events)
    record("rtex-kill-recovery", generation=state["generation"], revision=state["revision"],
           restarting_shown=True, layout_recovered=True)
    original = state["text"]
    revision = window.replace(original.replace("Editable body:",
                              "\\directlua{while true do end} Editable body:"))
    window.wait(lambda s: "pass_timeout" in json.dumps(s.get("diagnostics"))
                or "pass_timeout" in (s.get("error") or ""), "pass_timeout diagnostic", 80)
    assert window.app.poll() is None
    fixed = window.replace(original)
    assert fixed > revision
    recovered = window.ready(fixed, full=True)
    record("hung-background-pass", failed_revision=revision, repaired_revision=fixed,
           diagnostic="pass_timeout", app_responsive=True,
           repaired_preview_recovered=True, generation=recovered["generation"])


def negatives():
    secret = out / "outside-secret.tex"
    secret.write_text("OUTSIDE SECRET")
    target = out / "outside-write.txt"
    attacks = {
        "lua-read": f'\\directlua{{assert(io.open("{secret}"), "outside read denied")}}',
        "absolute-input": f'\\input{{{secret}}}',
        "lua-shell": '\\directlua{assert(os.execute("true"), "shell denied")}',
        "write18": (f'\\immediate\\write18{{touch {target}}}\n'
                    f'\\directlua{{assert(io.open("{target}"), "write18 denied")}}'),
        "lua-write": f'\\directlua{{assert(io.open("{target}","w"), "outside write denied")}}',
        "project-write": '\\directlua{assert(io.open("/project/forbidden.txt","w"), "project write denied")}',
        "tex-write": f'\\newwrite\\attack\\immediate\\openout\\attack={target}\n',
    }
    expected = {"lua-read": "outside read denied", "absolute-input": secret.name,
                "lua-shell": "shell denied", "write18": "write18 denied",
                "lua-write": "outside write denied", "project-write": "project write denied",
                "tex-write": target.name}
    for name, attack in attacks.items():
        source = "\\documentclass{article}\n\\begin{document}\n" + attack + "\nText.\\end{document}\n"
        window = Window(binary, out / name, source)
        try:
            state = window.wait(lambda s: expected[name] in json.dumps(s["diagnostics"])
                                and any(i["severity"] == "error" for i in s["diagnostics"]["items"]),
                                "sandbox diagnostic " + name)
            assert not state["toolchain_unavailable"], state
            assert not target.exists() and secret.read_text() == "OUTSIDE SECRET"
            assert not (window.project / "forbidden.txt").exists()
            assert window.app.poll() is None
            record("sandbox-" + name, diagnostic=state["error"] or state["diagnostics"]["items"],
                   crashed=False, outside_unchanged=True)
        finally:
            window.close()


def long_source(pages):
    preamble = "\\documentclass{article}\n\\begin{document}\n"
    paragraph = ("The benchmark paragraph contains ordinary English words and inline math $x^2+1$. " * 12)
    return preamble + "\n\\newpage\n".join(
        f"\\section{{Page {n+1}}}\n{paragraph}\n\n{paragraph}\n" for n in range(pages)
    ) + "\n\\end{document}\n"


def benchmark(pages):
    window = Window(binary, out / f"e6-{pages}", long_source(pages))
    try:
        s = window.ready(full=True)
        assert s["page_count"] == pages, s["page_count"]
        assert s["raster_pages"] == [1], "only the visible page is uploaded"
        window.click("preview.next")
        window.wait(lambda s: s["page_index"] == 1 and s["raster_pages"] == [2], "second page raster")
        window.click("preview.previous")
        window.wait(lambda s: s["page_index"] == 0 and s["raster_pages"] == [1], "return to first page")
        # Restore source focus after navigating the preview.
        rect = window.state()["source_rect"]
        window.xdo("mousemove", round(rect[0]+100), round(rect[1]+80), "click", "1")
        window.key("ctrl+Home", "Down", "Down", "Down", "End")
        samples, inputs, engine_samples = [], [], []
        delta = time.clock_gettime(time.CLOCK_BOOTTIME) - time.monotonic()
        for _ in range(12):
            # Complete the prior edit before sampling; its late full repaint is not this key.
            window.ready(full=True)
            revision = window.state()["revision"]
            pixels = window.settled_pixels()
            start = time.monotonic()
            window.key("x")
            accepted = window.wait(lambda s: s["revision"] > revision, "unblocked input")
            inputs.append((time.monotonic()-start)*1000)
            deadline = start + 30
            while window.pixels() == pixels and time.monotonic() < deadline:
                time.sleep(.002)
            assert time.monotonic() < deadline, "preview never repainted"
            repaint_ms = (time.monotonic()-start)*1000
            window.ready(accepted["revision"])
            assert current_event(window, accepted["revision"], True), "paragraph repaint required"
            timing = [e["elapsed_ms"] for e in window.engine_timings
                      if e["revision"] == accepted["revision"] and e["event"] == "paragraph"]
            assert timing and timing[-1] is not None, "rtex self-reported paragraph timing absent"
            engine_samples.append(timing[-1])
            samples.append(repaint_ms)
        # A real Lua infinite loop keeps the engine occupied while native input and frames run.
        payload = "\\directlua{while true do end} "
        text = window.state()["text"]
        def tex_ticks():
            ticks = 0
            for pid, name in window.descendants():
                if name in ("lualatex", "luatex", "luahbtex"):
                    try:
                        fields = Path(f"/proc/{pid}/stat").read_text().split(") ", 1)[1].split()
                        ticks += int(fields[11]) + int(fields[12])
                    except FileNotFoundError:
                        pass
            return ticks
        window.replace(text.replace("The benchmark paragraph", payload + "The benchmark paragraph", 1))
        time.sleep(.25)
        ticks_before = tex_ticks()
        busy_inputs = []
        frame_start = len(window.frames)
        for _ in range(15):
            revision = window.state()["revision"]
            start = time.monotonic()
            window.key("z")
            window.wait(lambda s: s["revision"] > revision, "input while rtex busy", 2)
            busy_inputs.append((time.monotonic()-start)*1000)
            time.sleep(.05)
        ticks_after = tex_ticks()
        assert ticks_after > ticks_before, "real TeX process did not consume CPU during input"
        busy_frames = window.frames[frame_start:]
        assert busy_frames and all(f["work_ms"] < 2000 for f in busy_frames), "UI blocked by engine"
        window.wait(lambda s: "pass_timeout" in json.dumps(s.get("diagnostics")),
                    "busy pass timeout before repair", 80)
        repaired = window.replace(window.state()["text"].replace(payload, "", 1))
        window.ready(repaired, full=True)
        suspended = abs(time.clock_gettime(time.CLOCK_BOOTTIME)-time.monotonic()-delta)
        assert suspended <= 1, "discard timing: host suspended"
        percentile = lambda data, q: sorted(data)[math.ceil(len(data)*q)-1]
        record(f"e6-{pages}-pages", pages=pages, n=len(samples),
               p50_ms=percentile(samples, .5), p95_ms=percentile(samples, .95),
               key_to_repaint_ms=samples, input_accept_ms=inputs,
               rtex_paragraph_ms=engine_samples,
               busy_input_ms=busy_inputs, busy_frame_work_ms=[f["work_ms"] for f in busy_frames],
               busy_frame_interval_ms=[f["interval_ms"] for f in busy_frames], suspend_seconds=suspended,
               host_preview_pipeline_ms=window.state()["elapsed_ms"],
               busy_tex_cpu_ticks=[ticks_before, ticks_after],
               method="native key dispatch to XGetImage preview pixels; audit revision + paragraph required")
    finally:
        window.close()


paper("lualatex")
paper("xelatex")
negatives()
benchmark(10)
benchmark(100)
