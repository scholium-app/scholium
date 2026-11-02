#!/usr/bin/env python3
"""Review an upstream main commit with E1/E2/full E8, then atomically update PIN on success.

Default candidate: the main head resolved at the start of this review. Upstream builds stay outside this repository.
E1 gates licenses/sources; bans and unfiltered advisories are report-only per ADR 0043.
E3/E5/E6 and repository gates remain required manual review before committing a bump.
"""
import argparse
import fcntl
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import time

ROOT = Path(__file__).resolve().parents[1]
EVAL = ROOT / 'spikes/rtex-eval'
PIN = EVAL / 'PIN'
spec = importlib.util.spec_from_file_location('rtex_drift', Path(__file__).with_name('rtex-upstream-drift.py'))
drift = importlib.util.module_from_spec(spec)
spec.loader.exec_module(drift)


def external(path):
    path = path.resolve()
    if path.is_relative_to(ROOT):
        raise ValueError(f'upstream build paths must stay outside repository: {path}')
    return path


def e1_passed(codes):
    return codes.get('licenses-sources') == 0


def e2_passed(rows):
    required = {('a','default'), ('b','default'), ('c','default'), ('c','lifted'),
                ('d-lua','default'), ('d-openout','default'), ('e','lifted'), ('f','default')}
    passed = set()
    for row in rows:
        v = row.get('verdict', {})
        key = (v.get('control'), v.get('config'))
        if key == ('e', 'default') and v.get('result', '').startswith('Inconclusive'):
            continue  # LuaSocket disabled by TeX itself; lifted network probe is mandatory.
        runs = row.get('runs', [])
        if (v.get('result') != 'Pass' or not v.get('host_effective') or v.get('sandbox_effective')
                or len(runs) != 2 or any(r.get('stray_processes') for r in runs)):
            return False
        passed.add(key)
    return required <= passed


def e8_passed(rows):
    names = ['live engine SIGKILL', 'live and background hang', 'preamble error: missing-package',
             'preamble error: undefined-command', 'malformed host commands', 'rtex serve SIGKILL']
    if len(rows) != len(names) or {r.get('scenario') for r in rows} != set(names):
        return False
    crash, hang, missing, undefined, malformed, killed = [next(r for r in rows if r['scenario'] == n) for n in names]
    crash_ok = (bool(crash.get('killed')) and crash.get('sandbox_alive') and crash.get('recovered_live_s')
                and any(e.get('state') == 'Ready' for e in crash.get('engine_states', []))
                and crash.get('layout_after', {}).get('compile', {}).get('state') == 'Ok')
    failed_timeout = any(e.get('compile', {}).get('state') == 'Failed'
                         and 8000 <= e.get('wall_ms', 0) < 12000 for e in hang.get('layouts', []))
    hang_ok = (hang.get('watchdog_s') is None and hang.get('upstream_pass_timeout_ms') == 8000
               and failed_timeout and hang.get('layout_recovered_after_fix_s')
               and 'time limit' in json.dumps(hang.get('diagnostics', [])))
    before_fix = [e for e in missing.get('engine_states', []) if 0 <= e.get('t', -1) < missing.get('fixed_at_s', 0)]
    missing_ok = ([e.get('state') for e in before_fix] == ['Starting', 'Failed']
                  and missing.get('recovered_live_s') and missing.get('layout_ok_after_fix_s'))
    undefined_ok = undefined.get('recovered_live_s') and undefined.get('layout_ok_after_fix_s')
    cases = malformed.get('cases', [])
    malformed_ok = (len(cases) == 11 and all(c.get('alive') and c.get('reply', {}).get('reply') == 'error' for c in cases)
                    and not malformed.get('written_outside_snapshot'))
    restarts = [v for k, v in killed.items() if k.startswith('restart_') and k.endswith('_first_layout_s')]
    killed_ok = (bool(killed.get('killed')) and killed.get('sandbox_exit') == 137
                 and killed.get('eof_after_s') is not None and not killed.get('stray_processes')
                 and len(restarts) == 2 and all(v > 0 for v in restarts))
    return bool(crash_ok and hang_ok and missing_ok and undefined_ok and malformed_ok and killed_ok)


def promote(path, repository, commit, gates):
    if set(gates) != {'E1', 'E2', 'E8'} or not all(gates.values()):
        return False
    candidate = {'repository': repository, 'branch': 'main', 'commit': commit}
    temporary = path.with_name(path.name + '.tmp')
    temporary.write_text(json.dumps(candidate, indent=2) + '\n')
    temporary.replace(path)
    return True


def load(path):
    return json.loads(path.read_text())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def prepare_candidate(args, report):
    old = load(PIN)
    cache = external(args.cache)
    cache.mkdir(parents=True, exist_ok=True)
    requested = args.commit or subprocess.check_output(
        ['git', 'ls-remote', old['repository'], 'refs/heads/main'], text=True).split()[0]
    if not re.fullmatch(r'[0-9a-f]{40}', requested):
        raise ValueError('candidate must be a full 40-character upstream commit SHA')
    clone = cache / f'rtex-{requested}'
    if not clone.exists():
        subprocess.run(['git', 'clone', old['repository'], str(clone)], check=True)
    if drift.git(clone, 'status', '--porcelain'):
        raise RuntimeError(f'upstream clone is dirty: {clone}')
    if drift.git(clone, 'remote', 'get-url', 'origin').removesuffix('.git') != old['repository'].removesuffix('.git'):
        raise RuntimeError('external clone repository differs from PIN')
    subprocess.run(['git', '-C', str(clone), 'fetch', 'origin', 'main', '--tags'], check=True)
    candidate = drift.resolve(clone, requested)
    subprocess.run(['git', '-C', str(clone), 'merge-base', '--is-ancestor', candidate, 'origin/main'], check=True)
    subprocess.run(['git', '-C', str(clone), 'checkout', '--detach', candidate], check=True)
    report.update(candidate=candidate, previous_pin=old, clone=str(clone), lock_sha256=sha(clone/'Cargo.lock'))
    return old, cache, clone, candidate


def build_config(cache, clone, candidate, stamp, run, report):
    source = cache/(stamp+'-config-src')
    source.mkdir()
    archive = subprocess.check_output(['git','-C',str(clone),'archive',candidate])
    with tarfile.open(fileobj=io.BytesIO(archive)) as tree:
        tree.extractall(source, filter='data')
    with (source/'crates/rtex-cli/Cargo.toml').open('a') as manifest:
        manifest.write('\n[[bin]]\nname = "rtex-pass-config-eval"\npath = '+json.dumps(str(EVAL/'pass-config.rs'))+'\n')
    # Build the evaluation target within the same upstream workspace and locked dependency graph.
    if run('config-build', ['cargo','build','--release','--locked','--offline','--manifest-path',str(source/'Cargo.toml'),
                            '--target-dir',str(clone/'target'),'--bin','rtex-pass-config-eval']):
        raise RuntimeError("evaluation config build failed")
    report['config_lock_sha256'] = sha(source/'Cargo.lock')
    binary = clone/'target/release/rtex-pass-config-eval'
    report['helper_source_sha256'] = sha(EVAL/'pass-config.rs')
    report['helper_binary_sha256'] = sha(binary)
    return binary


def evaluate(args, evidence, report):
    old, cache, clone, candidate = prepare_candidate(args, report)
    env = dict(os.environ, RUSTUP_TOOLCHAIN='1.99.0')
    def run(name, argv, seconds=1200):
        print(f'RUN {name}', flush=True)
        code = drift.run(argv, ROOT, evidence/(name+'.log'), seconds, env)
        report.setdefault('commands', []).append({'name':name, 'argv':list(map(str,argv)), 'exit':code})
        (evidence/'run.json').write_text(json.dumps(report, indent=2)+'\n')
        return code
    if run('build', ['cargo', 'build', '--release', '--locked', '--manifest-path', str(clone/'Cargo.toml')]):
        return
    codes = {}
    for name, checks in [('licenses-sources',['licenses','sources']), ('bans',['bans']), ('advisories',['advisories'])]:
        codes[name] = run('e1-'+name, ['cargo','deny','--locked','--manifest-path',str(clone/'Cargo.toml'),
                                     '--config',str(ROOT/'deny.toml'),'check','--show-stats',*checks], 300)
    report['E1'] = e1_passed(codes)
    if not report['E1']:
        return
    stamp = args.run_id or f'rtex-pin-{candidate[:12]}-{time.time_ns()}'
    def driver(name, script, extra):
        folder = evidence/name
        folder.mkdir()
        argv = [sys.executable, str(EVAL/script), '--clone', str(clone), '--evidence', str(folder), *extra]
        return run(name, argv), folder
    code, folder = driver('e2', 'negative.py', ['--run-id', stamp+'-e2'])
    report['E2'] = code == 0 and e2_passed(load(folder/'negative-summary.json'))
    if not report['E2']:
        return
    binary = build_config(cache, clone, candidate, stamp, run, report)
    code, full = driver('e8', 'e8_faults.py', ['--run-id',stamp+'-e8', '--binary',str(binary),'--pass-timeout-ms','8000'])
    matrix = load(full/f'e8-summary-{stamp}-e8.json')
    if code or not e8_passed(matrix):
        report['E8'] = False
        return
    code, passes = driver('e8-passes', 'v006_pass_faults.py', ['--binary',str(binary),'--work',str(cache/(stamp+'-passes'))])
    pass_ok = code == 0 and len(load(passes/'pass-faults.json')) == 2 and all(r['verdict']['pass'] for r in load(passes/'pass-faults.json'))
    code, cli = driver('e8-cli', 'e8_faults.py', ['--run-id',stamp+'-cli','--only','malformed'])
    cli_rows = load(cli/f'e8-summary-{stamp}-cli.json')
    sys.path.insert(0, str(EVAL))
    from v006_contracts import e8_verdict
    cli_ok = code == 0 and len(cli_rows) == 1 and len(cli_rows[0].get('cases', [])) == 11 and e8_verdict(cli_rows[0])['pass']
    report['E8'] = bool(pass_ok and cli_ok and report['config_lock_sha256'] == report['lock_sha256'])
    gates = {k:report[k] for k in ('E1','E2','E8')}
    report['pin_updated'] = promote(PIN, old['repository'], candidate, gates)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('commit', nargs='?', help='upstream main commit (default: freshly fetched origin/main)')
    parser.add_argument('--cache', type=Path, default=Path.home()/'.cache/scholium-eval')
    parser.add_argument('--evidence', type=Path, help='new evidence directory (default: external cache)')
    parser.add_argument('--run-id')
    args = parser.parse_args()
    report = {'E1':False,'E2':False,'E8':False,'pin_updated':False}
    evidence = (args.evidence or args.cache/f'rtex-pin-evidence-{time.time_ns()}').resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    try:
        # E2's historical global stray-process detector must never overlap another pin review.
        with Path('/tmp/scholium-rtex-pin.lock').open('a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            evaluate(args, evidence, report)
    except Exception as error:
        report['error'] = repr(error)
    (evidence/'run.json').write_text(json.dumps(report, indent=2)+'\n')
    for name in ('E1','E2','E8'):
        print(name, 'PASS' if report[name] else 'FAIL / incomplete', flush=True)
    print('PIN updated' if report['pin_updated'] else 'PIN unchanged', f'; evidence: {evidence}', flush=True)
    return 0 if report['pin_updated'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
