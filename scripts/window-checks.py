#!/usr/bin/env python3
"""Run discovered real-window checks under private, authenticated Xvfb displays.

Usage (repo root):
    python3 scripts/window-checks.py [--binary target/debug/scholium-app] [--out DIR]
        [--only NAME...] [--locale zh-Hans en] [--shard i/n] [--jobs K] [--evidence]
        [--refresh-durations] [--list]

Every selected check runs once in each requested locale (default zh-Hans), without
retries. CI uses four locale x duration-balanced shard jobs, each with --jobs 1.
Local workers share machine-wide /tmp/scholium-window-checks.<n>.lock slots. K is
--jobs, SCHOLIUM_WINDOW_JOBS, or max(1, min(3, cores // 6)). Use the same K across
worktrees for a common capacity. Exclusive EXCEPTIONS (timing/CPU-sensitive) hold
the whole machine gate, including runners with different K; old serial runners
also respect that gate. Never bypass it. Each check gets Xvfb -displayfd, never
the desktop's DISPLAY/Wayland. --evidence sets SCHOLIUM_CHECK_EVIDENCE=1.

Discovery: spikes/typst-main-editor/*-check.py plus a11y/run.sh; per-check exceptions
live below. --shard is one-based, greedy longest-first with stable name tie breaks;
unrecorded checks cost DEFAULT_SECONDS. --refresh-durations updates successful
checks' mean per-locale seconds in durations.json (run unsharded, without --only).
Machine-readable results.json includes execution time separately from slot wait.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
import time

from window_checks_runtime import check_slot, private_display, stop_group

ROOT = Path(__file__).resolve().parent.parent
CHECK_DIR = ROOT / "spikes" / "typst-main-editor"
DURATIONS = CHECK_DIR / "durations.json"
DEFAULT_SECONDS = 15.0
DEFAULT_SCREEN = "1280x900x24"
DEFAULT_TIMEOUT = 300
A11Y = "a11y"

# exclusive protects timings or CPU/raster budgets, not just explicit p50/p95.
EXCEPTIONS = {
    "latex-exit": {"skip": "real rtex + TeX Live acceptance runs in latex-exit.yml",
                   "timeout": 600, "exclusive": True},
    "latex-view": {"exclusive": True},  # E6 key-to-present p50/p95
    "caret-follow": {"exclusive": True},  # sustained typing/reflow at scale 1.5
    "parity": {"exclusive": True},  # input replay and independent raster comparison
    "source-preview-layout": {"exclusive": True},  # expensive evaluation deadline
    "problem-panel": {"screen": "1440x900x24"},
    "ui-polish-final": {"screen": "1440x900x24"},
    "sharp-raster": {"screen": "1920x1400x24", "exclusive": True},
    "default-launch": {"extra": ["target/debug/hammer"]},
    A11Y: {"session": "dbus"},
}


def discover():
    names = sorted(p.name[: -len("-check.py")] for p in CHECK_DIR.glob("*-check.py"))
    if (CHECK_DIR / "a11y" / "run.sh").exists():
        names.append(A11Y)
    return names


def balanced_shards(names, durations, count):
    shards, totals = [[] for _ in range(count)], [0.0] * count
    for name in sorted(names, key=lambda n: (-durations.get(n, DEFAULT_SECONDS), n)):
        index = min(range(count), key=lambda i: (totals[i], i))
        shards[index].append(name)
        totals[index] += durations.get(name, DEFAULT_SECONDS)
    return shards


def parse_shard(value):
    try:
        index, count = map(int, value.split("/"))
        if 1 <= index <= count:
            return index, count
    except ValueError:
        pass
    raise argparse.ArgumentTypeError("shard must be i/n with 1 <= i <= n")


def positive_int(value):
    number = int(value)
    if number < 1:
        raise argparse.ArgumentTypeError("jobs must be positive")
    return number


def command(name, binary, outdir):
    ex = EXCEPTIONS.get(name, {})
    extra = [str(ROOT / e) for e in ex.get("extra", [])]
    inner = (["bash", str(CHECK_DIR / "a11y" / "run.sh")] if name == A11Y else
             ["python3", str(CHECK_DIR / f"{name}-check.py")])
    inner += [str(binary), *extra, str(outdir)]
    return ["dbus-run-session", "--", *inner] if ex.get("session") == "dbus" else inner


def check_environment(locale, outdir, evidence):
    env = {k: v for k, v in os.environ.items()
           if k not in ("DISPLAY", "WAYLAND_DISPLAY", "XAUTHORITY", "DBUS_SESSION_BUS_ADDRESS",
                        "AT_SPI_BUS_ADDRESS")}
    env.update(SCHOLIUM_LOCALE=locale, SCHOLIUM_RECENT_FILE=str(outdir / "recent-projects.json"),
               WINIT_UNIX_BACKEND="x11")
    if evidence:
        env["SCHOLIUM_CHECK_EVIDENCE"] = "1"
    return env


def execute(name, binary, outdir, locale, evidence, log):
    env = check_environment(locale, outdir, evidence)
    ex = EXCEPTIONS.get(name, {})
    with open(log, "w") as fh:
        try:
            with private_display(ex.get("screen", DEFAULT_SCREEN), env) as display_env:
                process = subprocess.Popen(command(name, binary, outdir), cwd=ROOT, env=display_env,
                                           stdout=fh, stderr=subprocess.STDOUT, start_new_session=True)
                try:
                    rc = process.wait(timeout=ex.get("timeout", DEFAULT_TIMEOUT))
                    return "pass" if rc == 0 else f"FAIL({rc})"
                finally:
                    stop_group(process)
        except subprocess.TimeoutExpired:
            return "TIMEOUT"
        except (OSError, RuntimeError) as error:
            print(str(error), file=fh)
            return "ERROR"


def run_one(name, locale, args, binary, out):
    base = out / locale if len(args.locale) > 1 else out
    base.mkdir(parents=True, exist_ok=True)
    outdir, log = base / name, base / f"{name}.log"
    reason = EXCEPTIONS.get(name, {}).get("skip")
    row = dict(name=name, locale=locale, seconds=0.0, wait_seconds=0.0)
    if reason and not (args.only and name == "latex-exit"):
        return dict(row, result=f"skip ({reason})")
    queued = time.monotonic()
    print(f"queued {name} [{locale}] (waiting for machine slot)", flush=True)
    with check_slot(args.jobs, EXCEPTIONS.get(name, {}).get("exclusive", False)):
        start = time.monotonic()
        print(f"running {name} [{locale}] ...", flush=True)
        result = execute(name, binary, outdir, locale, args.evidence, log)
        row.update(result=result, seconds=time.monotonic()-start, wait_seconds=start-queued)
    print(f"{name} [{locale}]: {result} ({row['seconds']:.1f}s)", flush=True)
    if result != "pass":
        print("".join(log.read_text(errors="replace").splitlines(True)[-30:]), flush=True)
    return row


def arguments():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--binary", default="target/debug/scholium-app")
    ap.add_argument("--out", help="fresh output directory (default: temp dir)")
    ap.add_argument("--only", nargs="+", metavar="NAME")
    ap.add_argument("--locale", nargs="+", default=["zh-Hans"], choices=["zh-Hans", "en"])
    ap.add_argument("--shard", type=parse_shard)
    default_jobs = os.environ.get("SCHOLIUM_WINDOW_JOBS", str(max(1, min(3, (os.cpu_count() or 1)//6))))
    ap.add_argument("--jobs", type=positive_int, default=default_jobs)
    ap.add_argument("--evidence", action="store_true")
    ap.add_argument("--refresh-durations", action="store_true")
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()
    if args.refresh_durations and (args.only or args.shard):
        ap.error("--refresh-durations requires a complete, unsharded suite")
    if len(args.locale) != len(set(args.locale)):
        ap.error("locales must be unique")
    return ap, args


def main():
    ap, args = arguments()
    names = discover()
    if args.only:
        unknown = sorted(set(args.only) - set(names))
        if unknown:
            ap.error(f"unknown checks: {', '.join(unknown)}")
        names = [n for n in names if n in args.only]
    durations = json.loads(DURATIONS.read_text()) if DURATIONS.exists() else {}
    if args.shard:
        index, count = args.shard
        names = balanced_shards(names, durations, count)[index-1]
    if args.list:
        print("\n".join(names))
        return 0
    binary = (ROOT / args.binary).resolve()
    if not binary.exists():
        ap.error(f"binary not found: {binary} (build it first)")
    out = Path(args.out).resolve() if args.out else Path(tempfile.mkdtemp(prefix="scholium-window-"))
    out.mkdir(parents=True, exist_ok=True)
    if any(out.iterdir()):
        ap.error(f"output directory must be empty: {out}")
    start = time.monotonic()
    print(f"output: {out}; jobs: {args.jobs}; shard: {args.shard}", flush=True)
    work = [(name, locale) for locale in args.locale for name in names]
    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        rows = list(pool.map(lambda item: run_one(*item, args, binary, out), work))
    report = dict(wall_seconds=time.monotonic()-start, jobs=args.jobs, shard=args.shard, checks=rows)
    (out / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    failed = [r for r in rows if r["result"] != "pass" and not r["result"].startswith("skip")]
    for row in rows:
        print(f"{row['name']} [{row['locale']}]: {row['result']} {row['seconds']:.1f}s")
    print(f"\n{len(rows)-len(failed)}/{len(rows)} ok; wall {report['wall_seconds']:.1f}s; {out}")
    if args.refresh_durations and not failed:
        for name in names:
            samples = [r["seconds"] for r in rows if r["name"] == name and r["result"] == "pass"]
            if samples:
                durations[name] = round(statistics.mean(samples), 2)
        DURATIONS.write_text(json.dumps(durations, indent=2, sort_keys=True) + "\n")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
