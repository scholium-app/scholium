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

repo = Path(__file__).resolve().parents[1]
tag = "v0.15.1"
commit = "9dfd3a08500b7896045f907433cf7b4b02434fad"
names = ["typst", "typst-eval", "typst-html", "typst-kit", "typst-layout", "typst-library", "typst-macros",
         "typst-realize", "typst-render", "typst-svg", "typst-syntax", "typst-timing", "typst-utils"]
paths = ["Cargo.toml", "LICENSE", "README.md"] + [f"crates/{name}" for name in names]
patch_dir = repo / "spikes/typst-edit-session/patches"


def library_workspace(root):
    """Shrink the upstream workspace to the 13 vendored libraries, as in vendor/typst-edit."""
    manifest = root / "Cargo.toml"
    text = manifest.read_text()
    members = 'members = ["crates/*", "docs", "tests", "tests/fuzz", "tests/wrapper"]'
    default = 'default-members = ["crates/typst-cli"]'
    assert members in text and default in text, "upstream workspace members changed"
    text = text.replace(members, 'members = ["crates/*"]')
    manifest.write_text(text.replace(default, 'default-members = ["crates/typst"]'))


def main():
    upstream = Path(sys.argv[1]).resolve()
    archive = subprocess.check_output(["git", "-C", str(upstream), "archive", commit, *paths])
    patches = sorted(patch_dir.glob("*.patch"))
    assert len(patches) == 4
    with tempfile.TemporaryDirectory(prefix="scholium-fork-check-") as directory:
        rebuilt = Path(directory)
        with tarfile.open(fileobj=io.BytesIO(archive)) as source:
            source.extractall(rebuilt, filter="data")
        for patch in patches:
            subprocess.run(["git", "apply", str(patch)], cwd=rebuilt, check=True)
        library_workspace(rebuilt)
        vendored = repo / "vendor/typst-edit"
        files = sorted(p.relative_to(rebuilt) for p in rebuilt.rglob("*") if p.is_file())
        actual = sorted(p.relative_to(vendored) for p in vendored.rglob("*") if p.is_file())
        assert files == actual, "vendored file inventory differs"
        hashes = {}
        for path in files:
            data = (rebuilt / path).read_bytes()
            assert data == (vendored / path).read_bytes(), f"unaccounted source change: {path}"
            hashes[str(path)] = hashlib.sha256(data).hexdigest()
        result = {"upstream_commit": commit, "upstream_tag": tag, "crate_group": names,
                  "source_sha256": hashlib.sha256(json.dumps(hashes, sort_keys=True).encode()).hexdigest(),
                  "file_count": len(files), "files": hashes,
                  "patches": {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in patches},
                  "workspace_scope": "13 libraries only; no upstream CLI/docs/tests/fuzz"}
        print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
