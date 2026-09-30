#!/usr/bin/env python3
"""Prepare only this spike's pinned, ignored upstream checkout and patch series."""

from pathlib import Path
import subprocess
import tempfile

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
patches = sorted((ROOT / "patches").glob("*.patch"))
paths = {
    line[6:]
    for patch in patches
    for line in patch.read_text().splitlines()
    if line.startswith("+++ b/")
}


def contents(directory, relative):
    path = directory / relative
    return path.read_bytes() if path.exists() else None


# Later patches may change earlier context, so a reverse-check of patch 1 alone
# cannot detect a fully applied series. Compare exact prefix states instead.
matched = None
with tempfile.TemporaryDirectory(prefix="typst-edit-bootstrap-") as temporary:
    expected = Path(temporary)
    subprocess.run(
        ["git", "-c", "advice.detachedHead=false", "clone", "--quiet", "--no-hardlinks", str(CHECKOUT), temporary],
        check=True,
    )
    for index in range(len(patches) + 1):
        if all(contents(expected, path) == contents(CHECKOUT, path) for path in paths):
            matched = index
        if index < len(patches):
            subprocess.run(["git", "-C", temporary, "apply", str(patches[index])], check=True)
if matched is None:
    raise RuntimeError("Checkout differs from every patch prefix; preserving existing edits")
for patch in patches[matched:]:
    git("apply", "--check", str(patch))
    git("apply", str(patch))
    print(f"Applied: {patch.name}")
if matched == len(patches):
    print("Already applied: complete patch series")
print(f"Pinned Typst ready: {REVISION}")
