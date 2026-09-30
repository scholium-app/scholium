#!/usr/bin/env python3
"""Prepare only this spike's pinned, ignored upstream checkout and patch series."""

from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent
CHECKOUT = ROOT / ".vendor" / "typst"
UPSTREAM = "https://github.com/typst/typst.git"
REVISION = "9dfd3a08500b7896045f907433cf7b4b02434fad"


def git(*args, check=True):
    return subprocess.run(
        ["git", "-C", str(CHECKOUT), *args],
        check=check,
        text=True,
        capture_output=True,
    )


if not CHECKOUT.exists():
    CHECKOUT.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["git", "clone", "--depth", "1", "--branch", "v0.15.1", UPSTREAM, str(CHECKOUT)],
        check=True,
    )

assert git("rev-parse", "HEAD").stdout.strip() == REVISION, "unexpected upstream HEAD"
for patch in sorted((ROOT / "patches").glob("*.patch")):
    if git("apply", "--reverse", "--check", str(patch), check=False).returncode == 0:
        print(f"Already applied: {patch.name}")
        continue
    result = git("apply", "--check", str(patch), check=False)
    if result.returncode:
        raise RuntimeError(f"Cannot apply {patch.name}; preserving checkout edits:\n{result.stderr}")
    git("apply", str(patch))
    print(f"Applied: {patch.name}")
print(f"Pinned Typst ready: {REVISION}")
