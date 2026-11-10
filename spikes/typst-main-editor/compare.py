#!/usr/bin/env python3
"""Require identical dimensions and all RGBA bytes; height equality is insufficient."""
import hashlib
import json
import os
from pathlib import Path
import sys

fork, stock = map(Path, sys.argv[1:3])
results = []
for name in ("delim-input-norm", "delim-input-floor", "delim-input-ceil", "delim-invisible", "delim-open-interval", "delim-half-open", "display-root-delimiter", "display-only", "display-start", "display-middle", "display-end", "square-root", "indexed-root", "root-delimiter", "delim-paren", "delim-bracket", "delim-brace",
             "delim-abs", "delim-norm", "delim-angle", "delim-floor", "delim-ceil", "full", "edge-root", "edge-row", "fraction-at-caret", "scripts", "styles", "heading", "preedit", "empty-start", "empty-middle",
             "empty-three", "empty-end", "empty-heading", "empty-between-headings", "empty-only"):
    candidate = json.loads((fork / f"{name}.json").read_text())
    reference = json.loads((stock / f"{name}.json").read_text())
    a = (fork / f"{name}.rgba").read_bytes()
    b = (stock / f"{name}.rgba").read_bytes()
    assert (candidate["width"], candidate["height"]) == (reference["width"], reference["height"])
    assert a == b, f"stock differs: {name}"
    assert candidate["source_reads"] == 0
    if os.environ.get("SCHOLIUM_TEST_FONTS") == "bundled" and name.startswith("delim-"):
        frozen = json.loads((Path(__file__).parent / "reference/delimiter-pixels.json").read_text())
        if name in frozen:
            assert frozen[name] == {"width": reference["width"], "height": reference["height"],
                                    "sha256": hashlib.sha256(b).hexdigest()}, f"stale window reference: {name}"
    results.append({"case": name, "equal_pixels": True, "fork": candidate, "stock": reference,
                    "sha256": hashlib.sha256(a).hexdigest()})
print(json.dumps(results, indent=2))
