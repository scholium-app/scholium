#!/usr/bin/env python3
"""Build the reviewed PIN outside the repo and publish a verified, relocatable rtex bundle.

Run rtex-pin.py before moving PIN, then this script. TeX Live is never included.
The cache is serialized; Cargo.lock, Rust toolchain, host target and input hashes are recorded.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def run(argv, cwd, capture=False):
    env = {k: v for k, v in os.environ.items() if not (k.startswith('CARGO_PROFILE_')
           or k in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_TARGET_DIR', 'CARGO_BUILD_TARGET'))}
    env['RUSTUP_TOOLCHAIN'] = '1.99.0'
    return subprocess.run(list(map(str, argv)), cwd=cwd, check=True, text=True,
                          stdout=subprocess.PIPE if capture else None, env=env).stdout


def sha(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def inventory(root):
    files = {}
    for path in sorted(root.rglob('*')):
        if path.is_symlink():
            raise ValueError(f'symlink in bundle: {path}')
        if path.is_file() and path.relative_to(root).as_posix() != 'manifest.json':
            files[path.relative_to(root).as_posix()] = sha(path)
    return files


def verify(root, manifest):
    for name, digest in manifest['files'].items():
        path = Path(name)
        if path.is_absolute() or '..' in path.parts or name != path.as_posix():
            raise ValueError(f'invalid bundle path: {name}')
        if sha(root / path) != digest:
            raise ValueError(f'bundle checksum mismatch: {name}')
    if inventory(root) != manifest['files']:
        raise ValueError('bundle inventory mismatch')
    if manifest['files'].get('bin/rtex') != manifest['binary_sha256']:
        raise ValueError('binary checksum mismatch')


def verify_review(report, commit, lock_hash):
    if not (report.get('E1') is True and report.get('candidate') == commit
            and report.get('lock_sha256') == lock_hash):
        raise ValueError('E1 review must match the exact PIN commit and Cargo.lock')


def reviewed_e1(commit, lock_hash, requested):
    candidates = [requested] if requested else sorted((ROOT/'docs/log/evidence').glob('*/pin/run.json'))
    for path in candidates:
        report = json.loads(path.read_text())
        try:
            verify_review(report, commit, lock_hash)
        except ValueError:
            continue
        return {'report_sha256': sha(path), 'candidate': commit, 'lock_sha256': lock_hash, 'E1': True}
    raise ValueError('no matching E1 evidence; supply --review <rtex-pin.py evidence/run.json>')


def checkout(pin, cache):
    clone = cache / ('rtex-' + pin['commit'])
    if not clone.exists():
        run(['git', 'clone', pin['repository'], clone], cache)
    if run(['git', 'status', '--porcelain'], clone, True).strip():
        raise ValueError(f'dirty upstream cache: {clone}')
    remote = run(['git', 'remote', 'get-url', 'origin'], clone, True).strip()
    if remote.removesuffix('.git') != pin['repository'].removesuffix('.git'):
        raise ValueError('cache repository differs from PIN')
    if subprocess.run(['git', 'cat-file', '-e', pin['commit']+'^{commit}'], cwd=clone,
                      stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
        run(['git', 'fetch', 'origin', pin['commit']], clone)
    run(['git', 'checkout', '--detach', pin['commit']], clone)
    if run(['git', 'rev-parse', 'HEAD'], clone, True).strip() != pin['commit']:
        raise ValueError('built checkout differs from PIN')
    return clone


def license_files(directory):
    prefixes = ('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'COPYRIGHT', 'AUTHORS')
    return sorted(p for p in directory.rglob('*') if p.is_file()
                  and not any(x in p.relative_to(directory).parts for x in ('target', '.git'))
                  and p.name.upper().startswith(prefixes))


def install_licenses(clone, stage, host):
    metadata = json.loads(run(['cargo', 'metadata', '--locked', '--filter-platform', host, '--format-version=1'], clone, True))
    tree = run(['cargo', 'tree', '--locked', '--offline', '-p', 'rtex-cli', '--target', host,
                '--edges', 'normal,build', '--prefix', 'none', '--format', '{p}'], clone, True)
    built = {tuple(line.split()[:2]) for line in tree.splitlines()}
    notices = []
    for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
        if (package['name'], 'v'+package['version']) not in built:
            continue
        directory = Path(package['manifest_path']).parent
        files = license_files(directory)
        if not files and directory.is_relative_to(clone):
            directory, files = clone, [clone/'LICENSE']
        if not files:
            raise ValueError(f'no license text for {package["name"]} {package["version"]}')
        copied = []
        for source in files:
            target = stage/'licenses'/f'{package["name"]}-{package["version"]}'/source.relative_to(directory)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            copied.append(target.relative_to(stage).as_posix())
        notices.append({key: package[key] for key in ('name', 'version', 'license', 'source')})
        notices[-1]['files'] = copied
    (stage/'licenses/dependencies.json').write_text(json.dumps(notices, indent=2)+'\n')
    shutil.copyfile(clone/'LICENSE', stage/'licenses/RTEX-LICENSE')


def publish(stage, output):
    previous = output.with_name(output.name+'.previous')
    if previous.exists():
        raise ValueError(f'recover or remove previous interrupted publication first: {previous}')
    if output.exists():
        output.rename(previous)
    try:
        stage.rename(output)
    except BaseException:
        if previous.exists():
            previous.rename(output)
        raise
    if previous.exists():
        shutil.rmtree(previous)


def build(args, cache, output):
    pin = json.loads((ROOT/'spikes/rtex-eval/PIN').read_text())
    if not re.fullmatch('[0-9a-f]{40}', pin['commit']):
        raise ValueError('PIN must contain a full commit SHA')
    clone = checkout(pin, cache)
    lock_hash = sha(clone/'Cargo.lock')
    review = reviewed_e1(pin['commit'], lock_hash, args.review)
    rust = run(['rustc', '-vV'], clone, True).strip()
    host = next(line.removeprefix('host: ') for line in rust.splitlines() if line.startswith('host: '))
    # Always ask Cargo to check its fingerprint; do not trust an unverified cached binary.
    run(['cargo', 'build', '--release', '--locked', '--target-dir', clone/'target', '--bin', 'rtex'], clone)
    binary = clone/'target/release/rtex'
    built_hash = sha(binary)
    version = run([binary, '--version'], clone, True).strip()
    if (run(['git', 'rev-parse', 'HEAD'], clone, True).strip() != pin['commit']
            or run(['git', 'status', '--porcelain'], clone, True).strip()
            or sha(clone/'Cargo.lock') != lock_hash):
        raise ValueError('checkout changed during build')
    with tempfile.TemporaryDirectory(prefix='.rtex-stage-', dir=output.parent) as temporary:
        stage = Path(temporary)/'bundle'
        (stage/'bin').mkdir(parents=True)
        shutil.copy2(binary, stage/'bin/rtex')
        shutil.copytree(clone/'tex', stage/'tex')
        install_licenses(clone, stage, host)
        manifest = dict(schema=1, repository=pin['repository'], commit=pin['commit'], version=version,
                        binary_sha256=built_hash, cargo_lock_sha256=lock_hash, rustc=rust,
                        target=host, profile='release', review=review, files=inventory(stage))
        verify(stage, manifest)
        (stage/'manifest.json').write_text(json.dumps(manifest, indent=2, sort_keys=True)+'\n')
        publish(stage, output)
    verify(output, manifest)
    print(json.dumps({'bundle': str(output), 'commit': pin['commit'], 'version': version,
                      'binary_sha256': built_hash, 'files': len(manifest['files'])}, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, default=Path.home()/'.cache/scholium-eval')
    parser.add_argument('--output', type=Path, default=ROOT/'target/rtex-bundle')
    parser.add_argument('--review', type=Path, help='matching rtex-pin.py run.json (E1 evidence)')
    args = parser.parse_args()
    cache, output = args.cache.resolve(), args.output.resolve()
    if cache.is_relative_to(ROOT):
        parser.error('upstream cache/build must be outside the repo')
    if (output == ROOT or ROOT.is_relative_to(output) or output.is_relative_to(cache)
            or cache.is_relative_to(output)
            or (output.is_relative_to(ROOT) and not output.is_relative_to(ROOT/'target'))):
        parser.error('output must not replace the repo/cache; in-repo output must be under target/')
    cache.mkdir(parents=True, exist_ok=True)
    output.parent.mkdir(parents=True, exist_ok=True)
    with (cache/'bundle-rtex.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        # Also serialize different caches targeting the same installation.
        with (output.parent/(output.name+'.lock')).open('a') as install_lock:
            fcntl.flock(install_lock, fcntl.LOCK_EX)
            build(args, cache, output)


if __name__ == '__main__':
    main()
