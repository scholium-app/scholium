#!/usr/bin/env python3
"""Report upstream rtex drift without changing the pin or opening issues.

The upstream clone and build remain outside Scholium. External failures are summary rows,
not PR gates. TeX checks are optional and require a successful real bubblewrap probe.
"""
import argparse
import html
import json
import os
from pathlib import Path
import signal
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
EVAL = ROOT / "spikes/rtex-eval"


def run(argv, cwd, log, seconds=900, env=None):
    with log.open("w") as out:
        out.write("$ " + " ".join(map(str, argv)) + "\n")
        out.flush()
        process = subprocess.Popen(argv, cwd=cwd, env=env, stdout=out, stderr=subprocess.STDOUT,
                                   start_new_session=True)
        try:
            return process.wait(timeout=seconds)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
            out.write(f"\nTIMEOUT after {seconds}s\n")
            return 124


def git(clone, *args):
    return subprocess.check_output(["git", "-C", str(clone), *args], text=True).strip()


def resolve(clone, ref):
    for name in (ref, f"origin/{ref}"):
        try:
            return git(clone, 'rev-parse', '--verify', f'{name}^{{commit}}')
        except subprocess.CalledProcessError:
            continue
    raise ValueError(f"unknown upstream ref: {ref}")


def table(pin_branch, pin_sha, upstream_ref, sha, rows, commits=()):
    lines = [f"rtex upstream `{upstream_ref}` (`{sha}`); pin `{pin_branch}` (`{pin_sha}`)", "",
             "| stage | result | detail |", "| --- | --- | --- |"]
    lines += [f"| {name} | {status} | {detail.replace('|', '/').replace(chr(10), ' ')} |"
              for name, status, detail in rows]
    lines += ["", "Commits since pin:"]
    lines += [f"- {html.escape(commit)}" for commit in commits] or ["- None (or unavailable; see setup result)"]
    return "\n".join(lines) + "\n"


def smoke_passed(events):
    edit = next((e.get("result", {}).get("edit_id") for e in events if e.get("reply") == "edit"), None)
    layout = any(e.get("event") == "LayoutUpdate" and e.get("compile", {}).get("state") == "Ok"
                 for e in events)
    update = any(e.get("event") == "ParagraphUpdate" and e.get("edit_id") == edit and e.get("status") == "ok"
                 for e in events)
    return bool(edit is not None and layout and update and not any(e.get("reply") == "error" for e in events))


def load_eval(name):
    sys.path.insert(0, str(EVAL))
    return __import__(name)


def tex_checks(clone, work, logs):
    """Small Linux bwrap profile: empty HOME, RO runtime/project, private writable build, no network."""
    session_api = load_eval("session")
    fixture = "\\documentclass{article}\n\\begin{document}\nFirst paragraph marker.\n\nSecond paragraph.\n\\end{document}\n"
    project, build = work / "project", work / "build"
    project.mkdir()
    build.mkdir()
    (project / "main.tex").write_text(fixture)
    command = ["bwrap", "--unshare-all", "--die-with-parent", "--new-session", "--cap-drop", "ALL",
               "--ro-bind", "/usr", "/usr", "--ro-bind", "/etc", "/etc", "--ro-bind", "/var/lib/texmf", "/var/lib/texmf",
               "--symlink", "usr/bin", "/bin", "--symlink", "usr/lib", "/lib", "--symlink", "usr/lib64", "/lib64",
               "--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp", "--clearenv",
               "--setenv", "PATH", "/usr/bin", "--setenv", "HOME", "/tmp", "--setenv", "LANG", "C.UTF-8",
               "--setenv", "TEXMFVAR", "/tmp/texmf-var", "--setenv", "TEXMFCONFIG", "/tmp/texmf-config",
               "--setenv", "TEXINPUTS", ".:/project:", "--setenv", "RTEX_TEXLIVE_BIN", "/usr/bin",
               "--setenv", "RTEX_TEXDIR", "/rtex/tex", "--ro-bind", str(clone), "/rtex",
               "--ro-bind", str(project), "/project", "--bind", str(build), "/work", "--chdir", "/work",
               "/rtex/target/release/rtex", "serve", "--project", "/project", "--build", "/work/build"]
    s = session_api.Session(command, dict(os.environ), ROOT, logs, "protocol", redact=True)
    observed, rows = [], []
    try:
        layout = s.wait(lambda e: e.get("event") == "LayoutUpdate", 90)
        while layout.get("convergence", {}).get("state") != "Converged":
            layout = s.wait(lambda e: e.get("event") == "LayoutUpdate", 30)
        observed.append(layout)
        at = fixture.index("marker")
        s.send({"cmd": "edit", "path": "main.tex", "start": at, "end": at, "text": "x"})
        reply = s.wait(lambda e: e.get("reply") in ("edit", "error"), 20)
        observed.append(reply)
        edit_id = reply.get("result", {}).get("edit_id")
        observed.append(s.wait(lambda e: e.get("event") == "ParagraphUpdate" and e.get("edit_id") == edit_id, 20))
        rows.append(("protocol smoke", "ok" if smoke_passed(observed) else "FAIL", "serve + one edit + own update + layout"))
        for label, cmd in [("range rejection", {"cmd":"edit", "start":999999, "end":999999,"text":"x"}),
                           ("path rejection", {"cmd":"set_document", "path":"../../escape.tex", "text":"x"})]:
            s.send(cmd)
            answer=s.wait(lambda e: "reply" in e, 10)
            s.send({"cmd":"status"})
            alive=s.wait(lambda e:e.get("reply")=="status", 10)
            rows.append((label, "ok" if answer.get("reply")=="error" and alive else "FAIL", "E8 subset; server stays alive"))
    except Exception as error:
        rows.append(("TeX/protocol", "FAIL", repr(error)))
    finally:
        s.close()
    # E2 boundary: the same absolute TeX/Lua read works outside; private /tmp hides it inside.
    canary = Path('/tmp') / f'rtex-upstream-canary-{os.getpid()}'
    canary.write_text('CANARY')
    try:
        probe_source = r'\documentclass{article}\begin{document}\directlua{local f=io.open("' + str(canary) + r'","r"); if f then f:close(); texio.write_nl("SCHOLIUM_CANARY_READ") else texio.write_nl("SCHOLIUM_CANARY_DENIED") end}Probe.\end{document}'
        probe_tex = project/'probe.tex'
        probe_tex.write_text(probe_source)
        host_code=run(['lualatex','-interaction=nonstopmode',f'-output-directory={build}',str(probe_tex)],ROOT,logs/'e2-host.log',60)
        probe=command[:command.index('/rtex/target/release/rtex')] + ['/usr/bin/lualatex','-interaction=nonstopmode','-output-directory=/work','/project/probe.tex']
        code=run(probe,ROOT,logs/'e2-sandbox.log',60)
        host='SCHOLIUM_CANARY_READ' in (logs/'e2-host.log').read_text()
        denied='SCHOLIUM_CANARY_DENIED' in (logs/'e2-sandbox.log').read_text()
        rows.append(("E2 negative read", "ok" if host_code==0 and code==0 and host and denied else "FAIL", f"host effective={host}; sandbox denied={denied}"))
    finally:
        canary.unlink(missing_ok=True)
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("upstream", type=Path)
    parser.add_argument("--ref", default="main")
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--summary", type=Path)
    parser.add_argument("--tex", action="store_true")
    args=parser.parse_args()
    clone, work=args.upstream.resolve(), args.work.resolve()
    if clone.is_relative_to(ROOT) or work.is_relative_to(ROOT):
        parser.error("upstream and work must stay outside repository")
    work.mkdir(parents=True, exist_ok=False)
    logs=work/'logs'
    logs.mkdir()
    pin=json.loads((EVAL/'PIN').read_text())
    rows=[]
    sha='unresolved'
    commits=[]
    try:
        sha=resolve(clone, args.ref)
        if run(['git','-C',str(clone),'checkout','--detach',sha], clone, logs/'checkout.log', 30):
            raise RuntimeError('upstream checkout failed')
        commits=git(clone,'log','--oneline',f"{pin['commit']}..{sha}").splitlines()
        (logs/'commits.log').write_text('\n'.join(commits)+'\n')
        rows += [('upstream commit vs pin', 'same' if sha == pin['commit'] else 'different', sha),
                 ('commits since pin', 'info', f"{len(commits)}; see below and commits.log")]
        built=run(['cargo','build','--release','--locked'],clone,logs/'build.log',1200)
        rows.append(('build', 'ok' if built==0 else 'FAIL', f'exit {built}'))
        for name, checks in [('licenses-sources',['licenses','sources']),('bans',['bans']),('advisories',['advisories'])]:
            code=run(['cargo','deny','--locked','--manifest-path',str(clone/'Cargo.toml'),'--config',str(ROOT/'deny.toml'),
                      'check','--show-stats',*checks], ROOT, logs/f'deny-{name}.log',300)
            rows.append((f'cargo deny {name}', 'ok' if code==0 else 'findings', f'exit {code}; unfiltered log'))
        probe=run(['bwrap','--unshare-all','--ro-bind','/','/','/usr/bin/true'],ROOT,logs/'userns.log',20)
        rows.append(('unprivileged user namespaces', 'ok' if probe==0 else 'unavailable', f'bwrap exit {probe}; no sysctl override'))
        if args.tex and built==0 and probe==0:
            rows+=tex_checks(clone,work,logs)
        else:
            rows.append(('TeX/sandbox checks','not run','requires --tex, successful build and real bubblewrap namespace probe; full E2/E8/E3/E5/E6 remain local'))
    except Exception as error:
        rows.append(('setup', 'FAIL', repr(error)))
    report=table(pin.get('branch', 'main'),pin['commit'],args.ref,sha,rows,commits)
    (logs/'summary.md').write_text(report)
    if args.summary:
        with args.summary.open('a') as out: out.write(report)
    print(report,end='')
    return 0  # Monitoring only: the table retains failures; this is never a PR gate.


if __name__=='__main__':
    raise SystemExit(main())
