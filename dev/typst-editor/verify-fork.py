#!/usr/bin/env python3
"""Rebuild from pinned Git objects and tracked patches; ignore checkout dirt entirely."""
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile

repo = Path(__file__).resolve().parents[2]
upstream = Path(sys.argv[1]).resolve()
commit = "9dfd3a08500b7896045f907433cf7b4b02434fad"
names = ["typst", "typst-eval", "typst-html", "typst-kit", "typst-layout", "typst-library", "typst-macros",
         "typst-realize", "typst-render", "typst-svg", "typst-syntax", "typst-timing", "typst-utils"]
paths = ["Cargo.toml", "LICENSE", "README.md"] + [f"crates/{name}" for name in names]
archive = subprocess.check_output(["git", "-C", str(upstream), "archive", commit, *paths])
patches = sorted((repo / "spikes/typst-edit-session/patches").glob("*.patch"))
assert len(patches) == 4
with tempfile.TemporaryDirectory(prefix="scholium-fork-check-") as directory:
    rebuilt = Path(directory)
    with tarfile.open(fileobj=io.BytesIO(archive)) as source:
        source.extractall(rebuilt, filter="data")
    for patch in patches:
        subprocess.run(["git", "apply", str(patch)], cwd=rebuilt, check=True)
    manifest = rebuilt / "Cargo.toml"
    manifest.write_text(manifest.read_text().replace(
        'members = ["crates/*", "docs", "tests", "tests/fuzz", "tests/wrapper"]', 'members = ["crates/*"]'
    ).replace('default-members = ["crates/typst-cli"]', 'default-members = ["crates/typst"]'))
    vendored = repo / "vendor/typst-edit"
    files = sorted(p.relative_to(rebuilt) for p in rebuilt.rglob("*") if p.is_file())
    actual = sorted(p.relative_to(vendored) for p in vendored.rglob("*") if p.is_file())
    assert files == actual, "vendored file inventory differs"
    hashes = {}
    for path in files:
        data = (rebuilt / path).read_bytes()
        assert data == (vendored / path).read_bytes(), f"unaccounted source change: {path}"
        hashes[str(path)] = hashlib.sha256(data).hexdigest()
    result = {"upstream_commit": commit, "upstream_tag": "v0.15.1", "crate_group": names,
              "source_sha256": hashlib.sha256(json.dumps(hashes, sort_keys=True).encode()).hexdigest(),
              "file_count": len(files), "files": hashes,
              "patches": {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in patches},
              "workspace_scope": "13 libraries only; no upstream CLI/docs/tests/fuzz"}
    print(json.dumps(result, indent=2))
