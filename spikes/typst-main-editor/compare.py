#!/usr/bin/env python3
"""Require identical dimensions and all RGBA bytes; height equality is insufficient."""
import hashlib
import json
from pathlib import Path
import sys

fork, stock = map(Path, sys.argv[1:3])
results = []
for name in ("full", "styles", "heading"):
    candidate = json.loads((fork / f"{name}.json").read_text())
    reference = json.loads((stock / f"{name}.json").read_text())
    a = (fork / f"{name}.rgba").read_bytes()
    b = (stock / f"{name}.rgba").read_bytes()
    assert (candidate["width"], candidate["height"]) == (reference["width"], reference["height"])
    assert a == b, f"stock differs: {name}"
    assert candidate["source_reads"] == 0
    results.append({"case": name, "equal_pixels": True, "fork": candidate, "stock": reference,
                    "sha256": hashlib.sha256(a).hexdigest()})
print(json.dumps(results, indent=2))
