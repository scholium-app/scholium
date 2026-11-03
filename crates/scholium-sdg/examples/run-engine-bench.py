#!/usr/bin/env python3
"""Run separate engine processes sequentially; reject machine suspension during timing."""
import argparse
import json
from pathlib import Path
import subprocess
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("binary", type=Path)
parser.add_argument("output", type=Path)
parser.add_argument("--repeats", type=int, default=5)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=False)
for repeat in range(args.repeats):
    order = ("loro", "yrs") if repeat % 2 == 0 else ("yrs", "loro")
    for engine in order:
        mono = time.clock_gettime(time.CLOCK_MONOTONIC)
        boot = time.clock_gettime(time.CLOCK_BOOTTIME)
        result = subprocess.run([str(args.binary.resolve()), engine], check=True,
                                capture_output=True, text=True)
        elapsed_mono = time.clock_gettime(time.CLOCK_MONOTONIC) - mono
        elapsed_boot = time.clock_gettime(time.CLOCK_BOOTTIME) - boot
        row = json.loads(result.stdout)
        row.update(repeat=repeat, elapsed_monotonic_s=elapsed_mono,
                   elapsed_boottime_s=elapsed_boot, suspend_delta_s=elapsed_boot-elapsed_mono)
        (args.output / f"{repeat}-{engine}.json").write_text(json.dumps(row, indent=2) + "\n")
        if abs(elapsed_boot-elapsed_mono) > 1:
            raise RuntimeError("suspension detected; discard the whole run and rerun identically")
        print(json.dumps(row), flush=True)
