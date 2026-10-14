#!/usr/bin/env python3
"""Hardcoded interface text gate (docs/plan/I18N.md, ADR-0040).

Flags string literals that contain CJK characters in non-test Rust under crates/ unless
scripts/ui-strings-allowlist.txt lists them. An entry is `<file>\\t<JSON-quoted literal>`
and matches by file and literal text, never by line. The list may only shrink: a new
literal fails, and an entry whose literal no longer exists also fails, so a migrated
string must be deleted from the list in the same change.

    python3 scripts/check-ui-strings.py          # check (CI)
    python3 scripts/check-ui-strings.py --list   # print every current literal as entries
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ALLOWLIST = ROOT / "scripts" / "ui-strings-allowlist.txt"
CJK = re.compile("[　-〿㐀-䶿一-鿿＀-￯]")
TEST_FILE = re.compile(r"(^|/)(tests?|[a-z_]+_tests)(\.rs|/)")


def is_test_file(path: Path) -> bool:
    rel = path.relative_to(ROOT).as_posix()
    return bool(TEST_FILE.search(rel)) or "/tests/" in rel


def literals(source: str):
    """Yield (offset, raw text) of every string literal outside comments."""
    i, n = 0, len(source)
    while i < n:
        c = source[i]
        if source.startswith("//", i):
            end = source.find("\n", i)
            i = n if end < 0 else end
        elif source.startswith("/*", i):
            depth, i = 1, i + 2
            while i < n and depth:
                if source.startswith("/*", i):
                    depth, i = depth + 1, i + 2
                elif source.startswith("*/", i):
                    depth, i = depth - 1, i + 2
                else:
                    i += 1
        elif c == "r" and re.match(r'r#*"', source[i:i + 8]) and (i == 0 or not (source[i - 1].isalnum() or source[i - 1] == "_")):
            hashes = len(re.match(r"r(#*)", source[i:]).group(1))
            start = i + 2 + hashes
            end = source.find('"' + "#" * hashes, start)
            end = n if end < 0 else end
            yield start, source[start:end]
            i = end + 1 + hashes
        elif c == '"':
            start = i + 1
            i = start
            while i < n and source[i] != '"':
                i += 2 if source[i] == "\\" else 1
            yield start, source[start:i]
            i += 1
        elif c == "'":
            # A char literal ('x', '\n', '中') or a lifetime; skip the literal so its quote
            # can never be mistaken for the start of a string.
            m = re.match(r"'(\\.[^']*|[^\\'])'", source[i:])
            i += m.end() if m else 1
        else:
            i += 1


def test_module_ranges(source: str):
    """Byte ranges of `#[cfg(test)] mod name { ... }` bodies."""
    ranges = []
    for m in re.finditer(r"#\[cfg\(test\)\]\s*(?:pub(?:\([a-z]+\))?\s+)?mod\s+\w+\s*\{", source):
        depth, i = 1, m.end()
        while i < len(source) and depth:
            depth += {"{": 1, "}": -1}.get(source[i], 0)
            i += 1
        ranges.append((m.start(), i))
    return ranges


def collect() -> dict[tuple[str, str], int]:
    found: dict[tuple[str, str], int] = {}
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        if is_test_file(path):
            continue
        source = path.read_text(encoding="utf-8")
        skip = test_module_ranges(source)
        for offset, text in literals(source):
            if CJK.search(text) and not any(a <= offset < b for a, b in skip):
                key = (path.relative_to(ROOT).as_posix(), text)
                found[key] = found.get(key, 0) + 1
    return found


def entry(key: tuple[str, str]) -> str:
    return f"{key[0]}\t{json.dumps(key[1], ensure_ascii=False)}"


def load_allowlist() -> set[tuple[str, str]]:
    allowed = set()
    for line in ALLOWLIST.read_text(encoding="utf-8").splitlines():
        if line and not line.startswith("#"):
            path, quoted = line.split("\t", 1)
            allowed.add((path, json.loads(quoted)))
    return allowed


def main() -> int:
    found = collect()
    if "--list" in sys.argv:
        print("\n".join(entry(k) for k in sorted(found)))
        return 0
    allowed = load_allowlist()
    new = sorted(set(found) - allowed)
    stale = sorted(allowed - set(found))
    if new:
        print("Hardcoded interface text (add a catalog key instead, never grow the allowlist):", file=sys.stderr)
        print("\n".join("  " + entry(k) for k in new), file=sys.stderr)
    if stale:
        print("Allowlist entries whose literal is gone (delete them):", file=sys.stderr)
        print("\n".join("  " + entry(k) for k in stale), file=sys.stderr)
    if new or stale:
        return 1
    print(f"UI strings: OK ({len(found)} allowlisted literals remain)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
