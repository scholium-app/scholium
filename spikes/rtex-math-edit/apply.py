#!/usr/bin/env python3
"""Reconstruct the isolated evaluation clone, never change the E1–E8 clone."""

import argparse
import os
from pathlib import Path
import subprocess
import tempfile

BASE = "e0ad11765ec016846b6e07619081f0528621636b"
HERE = Path(__file__).resolve().parent
DEST = Path.home() / ".cache/scholium-eval/rtex-math-edit"


def git(*args, check=True, env=None):
    return subprocess.run(["git", "-C", str(DEST), *args], check=check, capture_output=True, text=True, env=env)


def verify_contents(patches):
    """Rebuild expected blobs in a temporary index; leave the real index untouched."""
    with tempfile.TemporaryDirectory(prefix="t22-index-") as temporary:
        env = dict(os.environ, GIT_INDEX_FILE=str(Path(temporary) / "index"))
        git("read-tree", BASE, env=env)
        for patch in patches:
            git("apply", "--cached", str(patch), env=env)
        paths = git("diff", "--cached", "--name-only", BASE, env=env).stdout.splitlines()
        for path in paths:
            expected = git("ls-files", "--stage", "--", path, env=env).stdout.split()[1]
            actual = git("hash-object", "--", path).stdout.strip()
            if actual != expected:
                raise SystemExit(f"refuse: patched content differs at {path}")
    print("patched file contents match reconstruction")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", action="store_true")
    args = parser.parse_args()
    if not DEST.exists():
        DEST.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(["git", "clone", "https://github.com/HenryXiaoYang/realtime-tex", str(DEST)], check=True)
        git("checkout", "--detach", BASE)
    if git("rev-parse", "HEAD").stdout.strip() != BASE:
        raise SystemExit("refuse: evaluation clone is not at the frozen base")
    patches = sorted((HERE / "patches").glob("*.patch"))
    if not patches:
        raise SystemExit("refuse: no patches")
    # Reconstruct the entire series before changing any working-tree file.
    # git apply treats multiple patch arguments as one diff, so dependent rounds
    # must be applied sequentially to the temporary index.
    with tempfile.TemporaryDirectory(prefix="t22-series-") as temporary:
        env = dict(os.environ, GIT_INDEX_FILE=str(Path(temporary) / "index"))
        git("read-tree", BASE, env=env)
        for patch in patches:
            git("apply", "--cached", str(patch), env=env)
        paths = git("diff", "--cached", "--name-only", BASE, env=env).stdout.splitlines()
        changed = {line[3:] for line in git("status", "--porcelain").stdout.splitlines()}
        if changed:
            if changed != set(paths):
                raise SystemExit("refuse: unexpected evaluation-clone paths")
            for path in paths:
                expected = git("ls-files", "--stage", "--", path, env=env).stdout.split()[1]
                if git("hash-object", "--", path).stdout.strip() != expected:
                    raise SystemExit(f"refuse: patched content differs at {path}")
            print("patches already applied")
        else:
            for patch in patches:
                git("apply", str(patch))
            print("applied", len(patches), "patches")
    verify_contents(patches)
    if args.build:
        subprocess.run(["cargo", "build", "--release", "--locked", "--manifest-path", str(DEST / "Cargo.toml")],
                       check=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
