#!/usr/bin/env python3
"""Real-window sharpness at a fractional display scale (docs/log/2026-10-12-sharp-raster.md).

Runs the app twice at WINIT_X11_SCALE_FACTOR=1.5: once with the display-resolution raster and
once with the former fixed 2 px/pt forced through SCHOLIUM_RASTER_PX_PER_PT. It types a sentence,
asserts the raster is exactly as wide as the page is shown (physical px) with the page origin on
the pixel grid, and that the text crop has a clearly higher mean gradient magnitude than the baseline.

Run under Xvfb from the repository root:
  xvfb-run -a -s '-screen 0 1280x900x24' python3 spikes/typst-main-editor/sharp-raster-check.py \\
    target/debug/scholium-app /tmp/out
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

SCALE = "1.5"
SENTENCE = "Sharp text rendering at fractional scale keeps stems and serifs crisp"
GAIN_REQUIRED = 1.2

binary = Path(sys.argv[1]).resolve()
output = Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
if any(output.iterdir()):
    raise SystemExit("output directory must be empty")
results = {}


def record(name, ok, **detail):
    results[name] = {"pass": bool(ok), **detail}
    print(("PASS " if ok else "FAIL ") + name, flush=True)
    if not ok:
        raise AssertionError(f"{name}: {detail}")


def gradient(png, box):
    """Mean |dx|+|dy| of the gray crop `box` = (x, y, w, h), in 0..255 units per pixel."""
    x, y, w, h = box
    raw = subprocess.run(
        ["convert", str(png), "-crop", f"{w}x{h}+{x}+{y}", "+repage", "-colorspace", "Gray",
         "-depth", "8", "gray:-"],
        check=True, capture_output=True).stdout
    assert len(raw) == w * h, (len(raw), w, h)
    total = 0
    for row in range(h):
        base = row * w
        for col in range(w):
            here = raw[base + col]
            if col + 1 < w:
                total += abs(here - raw[base + col + 1])
            if row + 1 < h:
                total += abs(here - raw[base + col + w])
    return total / (w * h)


def run(label, extra_env, scroll=False):
    folder = output / label
    folder.mkdir()
    env = dict(os.environ, SCHOLIUM_SESSION_FILE=str(folder / "doc.sqlite"), SCHOLIUM_EDITOR_AUDIT="1",
               WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR=SCALE, **extra_env)
    env.pop("WAYLAND_DISPLAY", None)
    log_path = folder / "app.log"

    def state():
        lines = [l for l in log_path.read_text().split("\n")[:-1] if l.startswith("AUDIT ")]
        return json.loads(lines[-1][6:]) if lines else None

    def wait(predicate, what, timeout=60):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            current = state()
            if current and predicate(current):
                return current
            time.sleep(0.05)
        raise RuntimeError(f"timeout: {what}: {state()}")

    def sharp(s):
        r = s.get("raster")
        return bool(r) and abs(r["px_per_pt"] - r["wanted_px_per_pt"]) < 1e-6 and r["visible_sharp"]

    with log_path.open("w") as log:
        app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
    try:
        wait(lambda s: s["current"], "first scene")
        window = subprocess.check_output(["xdotool", "search", "--pid", str(app.pid)], env=env,
                                         text=True).split()[0]
        subprocess.run(["xdotool", "windowfocus", window], check=True, env=env)
        subprocess.run(["xdotool", "type", "--delay", "30", SENTENCE], check=True, env=env)
        s = wait(lambda s: s["current"] and sharp(s) and len(s["carets"]) > len(SENTENCE) // 2,
                 "sentence laid out and rasterized at the wanted scale")
        time.sleep(0.4)
        s = state()
        png = folder / "page.png"
        subprocess.run(["import", "-window", window, str(png)], check=True, env=env)
        scrolled = scroll_case(window, env, wait, sharp) if scroll else None
        return s, png, scrolled
    finally:
        if app.poll() is None:
            app.terminate()
            try:
                app.wait(timeout=15)
            except subprocess.TimeoutExpired:
                app.kill()


def scroll_case(window, env, wait, sharp):
    """Lengthen the page past the viewport, then scroll to both ends: sharp strips follow."""
    xdo = lambda *args: subprocess.run(["xdotool", *map(str, args)], check=True, env=env)
    xdo("key", "Return")
    for number in range(150):
        xdo("type", "--delay", "2", f"Line {number} of a page longer than the viewport")
        xdo("key", "Return")
    wait(lambda s: s["current"] and s["raster"] and s["raster"]["height_px"] > 4000, "long page laid out")
    xdo("mousemove", "--window", window, "600", "400")
    seen = {}
    for name, button in (("top", 4), ("bottom", 5)):
        for _ in range(120):
            xdo("click", button)
        time.sleep(0.2)
        s = wait(lambda s: sharp(s) and s["raster"]["strips"] > 0, f"sharp strips at the {name}")
        seen[name] = {"page_top_px": s["raster"]["page_top_px"], "strips": s["raster"]["strips"],
                      "total_strips": -(-s["raster"]["height_px"] // 512)}
    return seen


def text_box(s):
    """Physical-pixel crop around the first text line, from the audited caret positions."""
    ppp = s["raster"]["pixels_per_point"]
    xs = [c["x"] for c in s["carets"]]
    y = s["carets"][0]["y"]
    left, right = int(min(xs) * ppp) - 2, int(max(xs) * ppp) + 2
    return left, int(y * ppp) - 20, right - left, 40


sharp_state, sharp_png, scrolled = run("display-resolution", {}, scroll=True)
raster = sharp_state["raster"]
record("01-fractional-scale-in-effect", abs(raster["pixels_per_point"] - float(SCALE)) < 1e-6,
       pixels_per_point=raster["pixels_per_point"])
record("02-raster-width-equals-displayed-width", abs(raster["width_px"] - raster["displayed_width_px"]) <= 1.0,
       width_px=raster["width_px"], displayed_width_px=raster["displayed_width_px"])
record("03-raster-is-denser-than-the-former-2px-per-pt", raster["px_per_pt"] > 2.5,
       px_per_pt=raster["px_per_pt"])
for axis in ("page_left_px", "page_top_px"):
    record(f"04-{axis}-on-pixel-grid", abs(raster[axis] - round(raster[axis])) < 1e-2, value=raster[axis])

old_state, old_png, _ = run("fixed-2px-per-pt", {"SCHOLIUM_RASTER_PX_PER_PT": "2"})
record("04b-scrolling-moves-the-page", scrolled["top"]["page_top_px"] > scrolled["bottom"]["page_top_px"],
       scrolled=scrolled)
record("04c-only-a-window-of-strips-is-resident",
       all(v["strips"] < v["total_strips"] for v in scrolled.values()), scrolled=scrolled)
record("05-baseline-uses-former-scale", abs(old_state["raster"]["px_per_pt"] - 2.0) < 1e-9,
       px_per_pt=old_state["raster"]["px_per_pt"])

box = text_box(sharp_state)
new_gradient, old_gradient = gradient(sharp_png, box), gradient(old_png, text_box(old_state))
for label, png in (("display-resolution", sharp_png), ("fixed-2px-per-pt", old_png)):
    x, y, w, h = box
    subprocess.run(["convert", str(png), "-crop", f"{w}x{h}+{x}+{y}", "+repage", "-scale", "300%",
                    str(output / f"crop-{label}.png")], check=True)
record("06-text-edges-are-sharper", new_gradient >= GAIN_REQUIRED * old_gradient,
       display_resolution=new_gradient, fixed_2px_per_pt=old_gradient, gain=new_gradient / old_gradient,
       required=GAIN_REQUIRED, crop=box)
(output / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
