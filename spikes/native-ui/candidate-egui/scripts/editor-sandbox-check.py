#!/usr/bin/env python3
"""Compare identical editor artifacts and check the reduced sandbox boundary."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

REPO = Path(__file__).resolve().parents[4]
PROBE = r'''
use std::{fs, path::Path, time::Duration};
fn main() {
    if std::env::args().any(|a| a == "wait") {
        std::thread::sleep(Duration::from_secs(4));
        return;
    }
    assert_eq!(fs::read_to_string("/project/sentinel").unwrap(), "read-only");
    assert!(fs::write("/project/sentinel", "changed").is_err());
    assert!(!Path::new("/etc/hostname").exists());
    assert!(!Path::new("/usr/bin/bash").exists());
    assert!(!Path::new("/usr/bin/xelatex").exists());
    assert!(!Path::new("/usr/share/texmf-dist").exists());
    assert!(Path::new("/usr/share/fonts").exists());
    assert!(std::env::var("SCHOLIUM_HOST_SENTINEL").is_err());
    fs::write("/work/probe-ok", "ok").unwrap();
}
'''


def execute(root, profile, args, seconds=20):
    env = dict(os.environ, SCHOLIUM_SPIKE_TOOLS=str(root / 'tools'),
               SCHOLIUM_SANDBOX_PROFILE=profile, SCHOLIUM_HOST_SENTINEL='private')
    started = time.perf_counter()
    result = subprocess.run(
        ['bash', str(REPO / 'spikes/toolchain-sandbox.sh'), str(root / 'input'),
         str(root / 'output'), str(seconds), *args], env=env,
        capture_output=True, timeout=seconds + 10)
    return result, round((time.perf_counter() - started) * 1000, 2)


def check(root):
    for directory in ('input', 'output', 'tools'):
        (root / directory).mkdir()
    (root / 'input/sentinel').write_text('read-only')
    (root / 'probe.rs').write_text(PROBE)
    subprocess.run(['rustc', str(root / 'probe.rs'), '-o', str(root / 'tools/probe')],
                   check=True, capture_output=True)
    started = time.perf_counter()
    shutil.copy2(REPO / 'spikes/typst-mapping/target/release/scholium-spike-typst',
                 root / 'tools/renderer')
    copy_ms = round((time.perf_counter() - started) * 1000, 2)
    shutil.copy2(REPO / 'docs/spikes/evidence/SPK-0028/theme/generated.typ',
                 root / 'input/main.typ')
    result, _ = execute(root, 'typst-editor', ['/toolchain/probe'])
    assert result.returncode == 0, result.stderr.decode()
    result, _ = execute(root, 'typst-editor', ['/toolchain/probe', 'wait'], seconds=1)
    assert result.returncode == 124, result.stderr.decode()
    result, _ = execute(root, 'invalid-profile', ['/toolchain/probe'])
    assert result.returncode == 2, 'unknown profile must be rejected'
    # Remove the probe so timed runs mount only the actual renderer and its libraries.
    (root / 'tools/probe').unlink()
    rows, reference = [], None
    for run in range(5):
        for profile in ('full', 'typst-editor'):
            result, startup_ms = execute(root, profile, ['/usr/bin/prlimit', '--version'])
            assert result.returncode == 0, result.stderr.decode()
            result, render_ms = execute(root, profile, ['/toolchain/renderer', 'editor-export'])
            assert result.returncode == 0, result.stderr.decode()
            artifacts = {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                         for p in (root / 'output').iterdir()
                         if p.suffix in ('.png', '.json')}
            if reference is None:
                reference = artifacts
            assert artifacts == reference, 'profile changed pixels or source geometry'
            rows.append(dict(run=run + 1, profile=profile, startup_ms=startup_ms,
                             render_ms=render_ms))
    return dict(isolation='pass', timeout='pass', invalid_profile='pass',
                identical_artifacts=reference, helper_copy_ms=copy_ms, runs=rows)


if __name__ == '__main__':
    destination = Path(sys.argv[1])
    with tempfile.TemporaryDirectory(prefix='scholium-editor-sandbox-') as temp:
        result = check(Path(temp))
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))
