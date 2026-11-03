"""Reproduce the pin's E3 verification, E5 location and sequential E6 regression.

Run from the repository root. Run latency only after window checks and other native tests finish.
"""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import time
from types import SimpleNamespace

root = Path.cwd()
sys.path.insert(0, str(root / 'spikes/rtex-eval'))
from e3_cjk import prepare, verify

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--clone', type=Path, required=True)
parser.add_argument('--phase', choices=['quick', 'latency'], required=True)
options = parser.parse_args()
out = Path(__file__).resolve().parent


def command(script, name):
    argv = [sys.executable, str(root / 'spikes/rtex-eval' / script),
            '--clone', str(options.clone), '--evidence', str(out / name),
            '--run-id', f't38-531e2c6-{name}']
    with (out / f'{name}-driver.log').open('w') as log:
        subprocess.run(argv, check=True, stdout=log, stderr=subprocess.STDOUT)


if options.phase == 'quick':
    args = SimpleNamespace(clone=options.clone, evidence=out / 'e3',
                           work=root / 'spikes/rtex-eval/.work/t38-531e2c6-e3')
    args.work.mkdir(parents=True, exist_ok=False)
    args.evidence.mkdir(parents=True, exist_ok=True)
    summary = {name: verify(args, name, prepare(args.work, name))
               for name in ('ctex', 'luatexja')}
    (args.evidence / 'e3-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    for item in summary.values():
        result = item['sandbox']
        assert result['exit'] == 0 and result['layer1_pass'] and result['layer2_pass']
        assert result['capture_pdf_equals_clean'] and result['pages'] == result['pages_exact']
    command('e5_locate.py', 'e5')
    for item in json.loads((out / 'e5/e5-summary.json').read_text()):
        assert item['samples'] and not item['markers_not_found']
        assert item['click_glyph_exact_rate'] == item['caret_glyph_exact_rate'] == 1
else:
    before = (time.monotonic(), time.clock_gettime(time.CLOCK_BOOTTIME))
    command('e6_latency.py', 'e6')
    after = (time.monotonic(), time.clock_gettime(time.CLOCK_BOOTTIME))
    drift = abs((after[1] - before[1]) - (after[0] - before[0]))
    (out / 'e6/clock.json').write_text(json.dumps({'before': before, 'after': after,
                                                'suspend_seconds': drift}, indent=2) + '\n')
    assert drift < 1, 'Suspend invalidated latency observations'
    for item in json.loads((out / 'e6/e6-summary.json').read_text()):
        assert 'error' not in item and item['sandbox_exit'] == 0
        for pattern in ('steady', 'bursts'):
            assert item[pattern]['edits'] == item[pattern]['routed']['fast'] == item[pattern]['paragraph_ok'] == 120
print(options.phase, 'PASS', flush=True)
