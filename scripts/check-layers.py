#!/usr/bin/env python3
"""Reject undeclared workspace layers, upward edges and UI below scholium-app."""
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
# Same-height typst/storage are independent services above document, below session.
LAYERS = {
    "scholium-model": 0,
    "scholium-document": 1,
    "scholium-typst": 2,
    "scholium-storage": 2,
    "scholium-session": 3,
    "scholium-app": 4,
}
UI = {"egui", "eframe", "winit", "accesskit"}


def check(metadata):
    packages = {p["id"]: p for p in metadata["packages"]}
    members = set(metadata["workspace_members"])
    graph = {n["id"]: n["dependencies"] for n in metadata["resolve"]["nodes"]}
    failures = []
    for member in sorted(members):
        name = packages[member]["name"]
        if name not in LAYERS:
            failures.append(f"unregistered workspace crate: {name}")
            continue
        # Follow all dependency kinds, including build and test dependencies. Use paths
        # for actionable transitive failures, including edges via non-workspace crates.
        pending = [(member, [name])]
        seen = {member}
        while pending:
            parent, path = pending.pop()
            for dep in graph.get(parent, []):
                dep_name = packages[dep]["name"]
                dep_path = path + [dep_name]
                if name != "scholium-app" and dep_name in UI:
                    failures.append("UI below app: " + " -> ".join(dep_path))
                if dep in members:
                    if dep_name not in LAYERS:
                        failures.append(f"unregistered workspace crate: {dep_name}")
                    elif dep != member and LAYERS[dep_name] >= LAYERS[name]:
                        failures.append("upward/lateral dependency: " + " -> ".join(dep_path))
                if dep not in seen:
                    seen.add(dep)
                    pending.append((dep, dep_path))
    return sorted(set(failures))


def main():
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked"],
        cwd=ROOT, capture_output=True, text=True, check=False,
    )
    if result.returncode:
        print(result.stderr, file=sys.stderr, end="")
        return result.returncode
    failures = check(json.loads(result.stdout))
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("Layers: OK (all workspace crates registered; no upward edges or UI below app)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
