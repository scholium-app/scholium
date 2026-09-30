#!/usr/bin/env python3
"""Run kernel assertions and compare actual stock/fork raster output."""

from pathlib import Path
import argparse
import os
import subprocess
import sys

from PIL import Image, ImageChops

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--out", type=Path, default=REPO / "docs/spikes/evidence/SPK-0051")
OUTPUT = parser.parse_args().out.resolve()
OUTPUT.mkdir(parents=True, exist_ok=True)
ENVIRONMENT = os.environ.copy()
ENVIRONMENT.setdefault("CARGO_TARGET_DIR", str(REPO / "spikes" / "render-latency" / "target"))


def run(args, log):
    with (OUTPUT / log).open("w") as stream:
        result = subprocess.run(args, cwd=REPO, env=ENVIRONMENT, stdout=stream, stderr=subprocess.STDOUT)
    if result.returncode:
        print((OUTPUT / log).read_text())
        raise SystemExit(result.returncode)
    print(f"PASS {log}")


run([sys.executable, str(ROOT / "bootstrap.py")], "bootstrap.log")
base = ["cargo", "run", "--release", "--locked", "--manifest-path"]
run([*base, str(ROOT / "Cargo.toml"), "--", "--editor", str(OUTPUT)], "kernel.log")
for name in ["full", "nested", "mixed", "ligature", "rtl", "bidi", "long", "empty", "lines"]:
    suffix = [name]
    run(
        [*base, str(ROOT / "reference" / "Cargo.toml"), "--", "--reference", str(OUTPUT / f"stock-{name}.png"), *suffix],
        f"stock-{name}.log",
    )
    stock = Image.open(OUTPUT / f"stock-{name}.png").convert("RGBA")
    fork = Image.open(OUTPUT / f"fork-{name}.png").convert("RGBA")
    assert stock.size == fork.size, (name, stock.size, fork.size)
    difference = ImageChops.difference(stock, fork).tobytes()
    changed = sum(any(difference[index:index + 4]) for index in range(0, len(difference), 4))
    assert changed == 0, (name, "changed pixels", changed)
    print(f"PASS stock_pixels_{name}: {stock.size}, changed={changed}")
print("PASS scoped_kernel_and_stock_comparison")
