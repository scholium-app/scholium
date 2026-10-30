#!/usr/bin/env python3
"""Time Text checkpoints with full history; reject suspend-spanning measurements."""
import argparse
import ast
import json
import platform
import re
import subprocess
import time
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--label", required=True)
    args = parser.parse_args()
    command = ["cargo", "test", "--locked", "-p", "scholium-session",
               "full_history_text_checkpoint", "--", "--ignored", "--nocapture"]
    mono = time.clock_gettime(time.CLOCK_MONOTONIC)
    boot = time.clock_gettime(time.CLOCK_BOOTTIME)
    result = subprocess.run(command, text=True, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, check=False)
    mono = time.clock_gettime(time.CLOCK_MONOTONIC) - mono
    boot = time.clock_gettime(time.CLOCK_BOOTTIME) - boot
    print(result.stdout, end="")
    result.check_returncode()
    if abs(boot - mono) > 1:
        raise SystemExit("DISCARD: suspend detected; rerun identical preparation")
    samples = ast.literal_eval(re.search(r"checkpoint_ns=(\[.*\])", result.stdout)[1])
    ordered = sorted(samples)
    evidence = dict(label=args.label, command=command, platform=platform.platform(),
                    revision=subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
                    profile="test (workspace opt-level 0, dependencies opt-level 2)",
                    blocks=286, pages=26, undo_depth=512, warmup=8,
                    samples_ns=samples, p50_ns=ordered[49], p95_ns=ordered[94],
                    monotonic_seconds=mono, boottime_seconds=boot,
                    suspend_delta_seconds=boot - mono)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(evidence, indent=2) + "\n")


if __name__ == "__main__":
    main()
