#!/usr/bin/env python3
"""Record the local environment, patch blobs and protected baseline state."""

import argparse
import hashlib
from pathlib import Path
import platform
import subprocess

from runtime import BASE, PATCHED, write_json


def command(args):
    result = subprocess.run(args, text=True, capture_output=True)
    return {"exit": result.returncode, "stdout": result.stdout, "stderr": result.stderr}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True, type=Path)
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    patches = [{"path": f"spikes/rtex-math-edit/patches/{p.name}", "sha256": sha(p),
                "numstat": command(["git", "apply", "--numstat", str(p)])}
               for p in sorted((here / "patches").glob("*.patch"))]
    facts = {"base_sha": command(["git", "-C", str(PATCHED), "rev-parse", "HEAD"]),
             "baseline_status": command(["git", "-C", str(BASE), "status", "--porcelain"]),
             "kernel": platform.platform(), "tex": command(["lualatex", "--version"]),
             "bwrap": command(["bwrap", "--version"]), "rust": command(["rustc", "--version"]),
             "patches": patches, "patched_executable_sha256": sha(PATCHED / "target/release/rtex"),
             "baseline_executable_sha256": sha(BASE / "target/release/rtex"),
             "tex_packages": command(["pacman", "-Q", "texlive-basic", "texlive-luatex",
                                      "texlive-langchinese", "texlive-mathscience", "python-fonttools"])}
    write_json(args.evidence / "environment.json", facts)


if __name__ == "__main__":
    main()
