#!/usr/bin/env python3
"""Run every real-window check exactly as CI does, under private Xvfb displays.

Usage (repo root):
    python3 scripts/window-checks.py [--binary target/debug/scholium-app] [--out DIR]
                                     [--only NAME...] [--locale zh-Hans en] [--list]

Checks are discovered: spikes/typst-main-editor/*-check.py (signature
`<binary> <outdir>`) plus the AT-SPI check a11y/run.sh. Per-check exceptions live
in EXCEPTIONS below. Checks run sequentially and are never retried. A machine-wide
lock serialises concurrent runs so parallel tasks never contend for the CPU.

Every check runs once per `--locale` (default zh-Hans; CI runs zh-Hans and en in one job after one build)
with SCHOLIUM_LOCALE set, so chrome is found by command id in either language. With more than
one locale the evidence goes to `<out>/<locale>/<name>`.
"""
import argparse
import fcntl
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHECK_DIR = ROOT / "spikes" / "typst-main-editor"
LOCK_PATH = "/tmp/scholium-window-checks.lock"
DEFAULT_SCREEN = "1280x900x24"
DEFAULT_TIMEOUT = 300
A11Y = "a11y"

# name -> overrides: screen (Xvfb geometry), timeout (s), extra (inserted after the
# binary), session ("dbus" wraps the check in dbus-run-session), skip (reason).
EXCEPTIONS = {
    "sharp-raster": {"screen": "1920x1400x24"},
    "default-launch": {"extra": ["target/debug/hammer"]},
    A11Y: {"session": "dbus"},
}


def discover():
    names = sorted(p.name[: -len("-check.py")] for p in CHECK_DIR.glob("*-check.py"))
    if (CHECK_DIR / "a11y" / "run.sh").exists():
        names.append(A11Y)
    return names


def command(name, binary, outdir):
    ex = EXCEPTIONS.get(name, {})
    extra = [str(ROOT / e) for e in ex.get("extra", [])]
    if name == A11Y:
        inner = ["bash", str(CHECK_DIR / "a11y" / "run.sh")]
    else:
        inner = ["python3", str(CHECK_DIR / f"{name}-check.py")]
    inner += [str(binary), *extra, str(outdir)]
    if ex.get("session") == "dbus":
        inner = ["dbus-run-session", "--", *inner]
    screen = ex.get("screen", DEFAULT_SCREEN)
    return ["xvfb-run", "-a", "-s", f"-screen 0 {screen}", *inner]


def run_one(name, binary, out, locale):
    outdir = out / name
    outdir.mkdir(parents=True, exist_ok=True)
    log = out / f"{name}.log"
    env = {k: v for k, v in os.environ.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY")}
    env.pop("XAUTHORITY", None)
    env["SCHOLIUM_LOCALE"] = locale
    timeout = EXCEPTIONS.get(name, {}).get("timeout", DEFAULT_TIMEOUT)
    start = time.monotonic()
    with open(log, "w") as fh:
        try:
            rc = subprocess.run(command(name, binary, outdir), cwd=ROOT, env=env,
                                stdout=fh, stderr=subprocess.STDOUT, timeout=timeout).returncode
            result = "pass" if rc == 0 else f"FAIL({rc})"
        except subprocess.TimeoutExpired:
            result = "TIMEOUT"
    return result, time.monotonic() - start, log


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--binary", default="target/debug/scholium-app")
    ap.add_argument("--out", help="output directory (default: fresh temp dir)")
    ap.add_argument("--only", nargs="+", metavar="NAME")
    ap.add_argument("--locale", nargs="+", default=["zh-Hans"], metavar="TAG",
                    help="interface language(s) to run every check in (zh-Hans, en)")
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()

    names = discover()
    if args.list:
        print("\n".join(names))
        return 0
    if args.only:
        unknown = sorted(set(args.only) - set(names))
        if unknown:
            print(f"unknown checks: {', '.join(unknown)}", file=sys.stderr)
            return 2
        names = [n for n in names if n in args.only]

    binary = Path(args.binary)
    binary = binary if binary.is_absolute() else ROOT / binary
    if not binary.exists():
        print(f"binary not found: {binary} (build it first)", file=sys.stderr)
        return 2
    out = Path(args.out).resolve() if args.out else Path(tempfile.mkdtemp(prefix="scholium-window-"))
    out.mkdir(parents=True, exist_ok=True)

    with open(LOCK_PATH, "w") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            print("waiting for another window-check run", flush=True)
            fcntl.flock(lock, fcntl.LOCK_EX)
        print(f"output: {out}", flush=True)
        rows = []
        for locale in args.locale:
            base = out / locale if len(args.locale) > 1 else out
            for name in names:
                label = f"{name} [{locale}]"
                reason = EXCEPTIONS.get(name, {}).get("skip")
                if reason:
                    rows.append((label, f"skip ({reason})", 0.0))
                    continue
                print(f"running {label} ...", flush=True)
                result, secs, log = run_one(name, binary, base, locale)
                rows.append((label, result, secs))
                if result != "pass":
                    print(f"--- {label} {result}; tail of {log}", flush=True)
                    print("".join(log.read_text(errors="replace").splitlines(True)[-30:]), flush=True)

    width = max(len(n) for n, _, _ in rows)
    print(f"\n{'check':<{width}}  {'result':<10}  seconds")
    for name, result, secs in rows:
        print(f"{name:<{width}}  {result:<10}  {secs:7.1f}")
    failed = [n for n, r, _ in rows if r not in ("pass",) and not r.startswith("skip")]
    print(f"\n{len(rows) - len(failed)}/{len(rows)} ok; logs and evidence in {out}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
