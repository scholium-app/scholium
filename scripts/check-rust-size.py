#!/usr/bin/env python3
"""Check the production workspace's Rust file-size budget; spikes remain independent."""
from pathlib import Path
import sys

root = Path(__file__).resolve().parents[1]
limit = 600
violations = []
for path in sorted((root / "crates").rglob("*.rs")):
    count = len(path.read_text(encoding="utf-8").splitlines())
    if count > limit:
        violations.append(f"{path.relative_to(root)}: {count} lines (maximum {limit})")
if violations:
    print("\n".join(violations), file=sys.stderr)
    sys.exit(1)
print("Rust file sizes: OK")
