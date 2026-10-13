#!/usr/bin/env python3
"""Check that every relative Markdown link and image under docs/, AGENT.md and README.md resolves.

Usage: python3 scripts/check-doc-links.py   (run from anywhere; exits 1 on a broken link)
Anchors (#...) are stripped and not verified; absolute URLs and mailto: are skipped.
"""
import re
import sys
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parent.parent
LINK = re.compile(r"!?\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
FENCE = re.compile(r"^\s*(```|~~~)")
SKIP_PREFIXES = ("http://", "https://", "mailto:", "#", "tel:")


def markdown_files():
    yield from sorted((ROOT / "docs").rglob("*.md"))
    for name in ("AGENT.md", "README.md"):
        path = ROOT / name
        if path.exists():
            yield path


def broken_links(path):
    in_fence = False
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if FENCE.match(line):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        line = re.sub(r"`[^`]*`", "", line)
        for target in LINK.findall(line):
            if target.startswith(SKIP_PREFIXES):
                continue
            rel = unquote(target.split("#", 1)[0])
            if rel and not (path.parent / rel).resolve().exists():
                yield number, target


def main():
    failures = 0
    for path in markdown_files():
        for number, target in broken_links(path):
            print(f"{path.relative_to(ROOT)}:{number}: broken link {target}")
            failures += 1
    if failures:
        print(f"{failures} broken link(s)")
        return 1
    print("all relative links resolve")
    return 0


if __name__ == "__main__":
    sys.exit(main())
