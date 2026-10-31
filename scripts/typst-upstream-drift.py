#!/usr/bin/env python3
"""Weekly drift check of the vendored Typst fork against an upstream ref (default: upstream main).

Usage: python3 scripts/typst-upstream-drift.py <upstream-clone> [--ref main] [--work DIR] [--summary FILE]

Three stages, each run only if the previous one passed; the exit code names the first failing stage:
  1  the tracked patches do not apply (clean = as is, merged = git 3-way merge, conflict = needs a human)
  2  scholium-typst --features editor does not compile against the patched upstream tree
  3  the patched fork's pixels differ from unpatched upstream at the same commit (fork-check vs stock reference)
Exit 4 is a setup error (unknown ref, unusable --work directory), not a drift result.
The clone needs the pinned base commit and the ref; full history adds the upstream commits behind each conflict.
The checkout is left unchanged: lockfiles are restored, builds go to target/typst-upstream, trees and logs to --work.
"""
import argparse
import importlib.util
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile

REPO = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location("verify_typst_fork", REPO / "scripts/verify-typst-fork.py")
fork = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(fork)
IDENTITY = ["-c", "user.name=typst-upstream-drift", "-c", "user.email=drift@localhost", "-c", "commit.gpgsign=false"]
STOCK_MANIFEST = REPO / "spikes/typst-main-editor/reference/Cargo.toml"
TARGET = REPO / "target/typst-upstream"
MARKER = ".typst-upstream-drift"
# Source trees stay outside the repository: path crates under it would join the Scholium workspace.
WORK = Path(tempfile.gettempdir()) / "scholium-typst-upstream"


def setup_error(message):
    print(message, file=sys.stderr)
    raise SystemExit(4)


def run(cmd, log=None):
    """Run a command from the repository root, appending its output to `log`; return (exit code, output)."""
    done = subprocess.run(cmd, cwd=REPO, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, check=False)
    if log:
        with log.open("a") as out:
            out.write(f"$ {' '.join(map(str, cmd))}\n{done.stdout}\n")
    return done.returncode, done.stdout


def git(tree, *args, log=None):
    return run(["git", "-C", str(tree), *IDENTITY, *args], log)


def resolve(clone, ref):
    for name in (ref, f"origin/{ref}"):
        code, out = git(clone, "rev-parse", "--verify", "--quiet", f"{name}^{{commit}}")
        if code == 0:
            return out.strip()
    setup_error(f"unknown upstream ref: {ref}")


def apply_series(tree, target, patches, log):
    """Commit the series on the pinned base (3-way merges need its blobs), then replay it onto `target`."""
    steps = [("checkout", "-q", "--detach", fork.commit)]
    for patch in patches:
        steps += [("apply", "--index", str(patch)), ("commit", "-q", "-m", patch.stem)]
    for step in [*steps, ("checkout", "-q", "--detach", target)]:
        if git(tree, *step, log=log)[0] != 0:
            setup_error(f"cannot rebuild the fork at {fork.tag} in the clone (git {step[0]}); see {log}")
    rows, blocked = [], False
    for patch in patches:
        if blocked:
            rows.append((f"patch {patch.stem}", "not tried", "blocked by the conflict above"))
            continue
        clean = git(tree, "apply", "--check", str(patch), log=log)[0] == 0
        code, _ = git(tree, "apply", "--3way", "--index", str(patch), log=log)
        unmerged = git(tree, "diff", "--name-only", "--diff-filter=U")[1].split()
        if code == 0 and not unmerged:
            git(tree, "commit", "-q", "-m", patch.stem, log=log)
            rows.append((f"patch {patch.stem}", "clean" if clean else "merged", ""))
        else:
            blocked = True
            rows.append((f"patch {patch.stem}", "conflict", conflict_detail(tree, target, unmerged)))
    return rows


def conflict_detail(tree, target, files):
    """Name the conflicting files and the upstream commits (not cherry-picked into the base) that touched them."""
    _, commits = git(tree, "log", "--format=%h %as %s", "--cherry-pick", "--right-only",
                     f"{fork.commit}...{target}", "--", *files)
    shown = "; ".join(commits.splitlines()[:5]) or "history not available in this clone"
    return f"{', '.join(files) or 'patch does not apply'}; upstream: {shown}"


def export(tree, rev, destination):
    """Extract the 13 vendored crates at `rev`, workspace shrunk exactly like vendor/typst-edit."""
    archive = subprocess.check_output(["git", "-C", str(tree), "archive", rev, *fork.paths])
    with tarfile.open(fileobj=io.BytesIO(archive)) as source:
        source.extractall(destination, filter="data")
    fork.library_workspace(destination)
    config = destination.with_suffix(".toml")
    lines = ["[patch.crates-io]"] + [f'{n} = {{ path = "{destination / "crates" / n}" }}' for n in fork.names]
    config.write_text("\n".join(lines) + "\n")
    return config


def cargo(config, args, log, target):
    """Cargo with a config-level [patch], which overrides Cargo.toml's. Lockfiles drift, so restore them."""
    locks = [REPO / "Cargo.lock", STOCK_MANIFEST.with_name("Cargo.lock")]
    saved = {lock: lock.read_bytes() for lock in locks}
    try:
        return run(["cargo", "--config", str(config), *args, "--target-dir", str(target)], log)
    finally:
        for lock, data in saved.items():
            lock.write_bytes(data)


def compile_stage(work, logs):
    code, out = cargo(work / "patched.toml", ["check", "-p", "scholium-typst", "--features", "editor", "--examples",
                                              "--message-format", "short"], logs / "compile.log", TARGET)
    if code == 0:
        return "ok", ""
    errors = [line for line in out.splitlines() if ": error" in line]
    ours = sum(1 for line in errors if not line.startswith(str(work)))
    first = errors[0] if errors else out.strip().splitlines()[-1]
    where = f"{ours} in Scholium, {len(errors) - ours} in the patched tree"
    return "FAIL", f"{len(errors)} errors ({where}); first: {first}"


def pixel_stage(work, logs):
    fork_out, stock_out = work / "pixels-fork", work / "pixels-stock"
    builds = [
        (work / "patched.toml", ["run", "-p", "scholium-typst", "--features", "editor", "--example",
                                 "typst-main-fork-check", "--", str(fork_out)], TARGET),
        (work / "stock.toml", ["run", "--manifest-path", str(STOCK_MANIFEST), "--", str(stock_out)], TARGET),
    ]
    for config, args, target in builds:
        if cargo(config, args, logs / "pixels.log", target)[0] != 0:
            return "FAIL", f"{config.stem} render failed; see pixels.log"
    code, out = run(["python3", str(REPO / "spikes/typst-main-editor/compare.py"), str(fork_out), str(stock_out)])
    (logs / "compare.json").write_text(out)
    if code != 0:
        return "FAIL", out.strip().splitlines()[-1]
    return "equal", ", ".join(case["case"] for case in json.loads(out)) + " (all RGBA bytes)"


def fresh(work):
    """Reset the work directory, refusing to delete one this script did not create."""
    if work.exists() and any(work.iterdir()):
        if not (work / MARKER).exists():
            setup_error(f"--work {work} is not empty and was not created by this script")
        shutil.rmtree(work)
    (work / "logs").mkdir(parents=True)
    (work / MARKER).touch()
    return work / "logs"


def table(sha, date, rows):
    lines = [f"Typst upstream drift: `{sha[:12]}` ({date}) vs fork base {fork.tag} `{fork.commit[:12]}`", "",
             "| stage | result | detail |", "| --- | --- | --- |"]
    lines += [f"| {stage} | {result} | {detail.replace('|', '/')} |" for stage, result, detail in rows]
    return "\n".join(lines) + "\n"


def stages(clone, sha, work, logs):
    """Run the three stages in order; return the table rows and the first failing stage (0 if none)."""
    tree = work / "git"
    git(clone, "worktree", "prune")
    if git(clone, "worktree", "add", "-q", "--detach", str(tree), sha, log=logs / "patches.log")[0] != 0:
        setup_error(f"cannot add a worktree to {clone}; see {logs / 'patches.log'}")
    try:
        rows = apply_series(tree, sha, sorted(fork.patch_dir.glob("*.patch")), logs / "patches.log")
        if any(status in ("conflict", "not tried") for _, status, _ in rows):
            return rows, 1
        export(tree, "HEAD", work / "patched")
        export(tree, sha, work / "stock")
    finally:
        git(clone, "worktree", "remove", "--force", str(tree))
    rows.append(("compile scholium-typst (editor)", *compile_stage(work, logs)))
    if rows[-1][1] != "ok":
        return rows, 2
    rows.append(("pixels: patched vs unpatched", *pixel_stage(work, logs)))
    return rows, 0 if rows[-1][1] == "equal" else 3


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("upstream", type=Path)
    parser.add_argument("--ref", default="main")
    parser.add_argument("--work", type=Path, default=WORK, help="scratch trees and logs, outside the repo")
    parser.add_argument("--summary", type=Path, help="also append the table to this file")
    args = parser.parse_args()
    clone, work = args.upstream.resolve(), args.work.resolve()
    if work.is_relative_to(REPO):
        setup_error("--work must be outside the repository")
    sha = resolve(clone, args.ref)
    date = git(clone, "show", "-s", "--format=%cs", sha)[1].strip()
    logs = fresh(work)
    rows, failed = stages(clone, sha, work, logs)
    report = table(sha, date, rows)
    (logs / "summary.md").write_text(report)
    if args.summary:
        with args.summary.open("a") as out:
            out.write(report)
    print(report, end="")
    raise SystemExit(failed)


if __name__ == "__main__":
    main()
