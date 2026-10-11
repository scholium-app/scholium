#!/usr/bin/env python3
"""Plain launch without SCHOLIUM_SESSION_FILE: legacy head migrates once, original kept."""
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import time

binary = Path(sys.argv[1]).resolve()
# scholium-storage's `hammer` writes genuine legacy (schema 0) heads; the legacy editor is gone.
hammer = Path(sys.argv[2]).resolve()
output = Path(sys.argv[3]).resolve()
output.mkdir(parents=True, exist_ok=True)
if any(output.iterdir()):
    raise SystemExit("output directory must be empty")
data = output / "xdg"
env = dict(os.environ, XDG_DATA_HOME=str(data), SCHOLIUM_EDITOR_AUDIT="1",
           WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR="1")
env.pop("WAYLAND_DISPLAY", None)
env.pop("SCHOLIUM_SESSION_FILE", None)
legacy = data / "scholium" / "session.scholium.sqlite"
target = data / "scholium" / "document.v1.sqlite"


def run(args, name, actions):
    with (output / f"{name}.log").open("w") as log:
        app = subprocess.Popen([str(binary), *args], env=env, stdout=log, stderr=subprocess.STDOUT)
    try:
        time.sleep(4)
        window = subprocess.check_output(["xdotool", "search", "--pid", str(app.pid)], env=env, text=True).split()[0]
        subprocess.run(["xdotool", "windowfocus", window], env=env, check=True)
        actions(window)
        subprocess.run(["import", "-window", window, str(output / f"{name}.png")], env=env, check=True)
    finally:
        app.terminate()
        app.wait(timeout=15)


legacy.parent.mkdir(parents=True)
subprocess.run([str(hammer), str(legacy), "1"], env=env, check=True, stdout=subprocess.DEVNULL)
heads = sqlite3.connect(f"file:{legacy}?mode=ro", uri=True).execute("select count(*) from snapshots").fetchone()[0]
assert heads > 0, "hammer wrote a legacy head"
assert not target.exists(), "writing a legacy head never creates the v1 document"
before = hashlib.sha256(legacy.read_bytes()).hexdigest()
def current(window):
    # Cold debug starts scan fonts before the first Content layout; wait for it.
    end = time.monotonic() + 60
    log = output / "default.log"
    while not any(l.startswith("AUDIT ") and json.loads(l[6:])["current"]
                  for l in log.read_text().split("\n")[:-1]):
        assert time.monotonic() < end, "first Typst scene timeout"
        time.sleep(0.1)
    time.sleep(0.12)


run([], "default", current)
assert target.exists(), "plain launch created the v1 document by migration"
assert hashlib.sha256(legacy.read_bytes()).hexdigest() == before, "legacy file untouched"
audits = [json.loads(l[6:]) for l in (output / "default.log").read_text().split("\n")[:-1] if l.startswith("AUDIT ")]
last = audits[-1]
assert last["backend"] == "Typst Content main app" and last["current"]
assert all(a["page_text_draws"] == 0 for a in audits)
text = json.dumps(last["snapshot"], ensure_ascii=False)
assert "内容 1" in text, "migrated content is the authority"
result = {"binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "legacy_heads": heads,
          "legacy_sha256_unchanged": True, "v1_created": True, "default_backend": last["backend"],
          "audit_frames": len(audits), "committed_egui_text_draws": 0}
(output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
print("PASS default launch migrates legacy head and renders through Typst Content")
