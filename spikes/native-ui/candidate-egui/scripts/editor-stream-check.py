#!/usr/bin/env python3
"""Compare resident raster/geometry with the independent one-shot PNG export."""
import json
import os
from pathlib import Path
import selectors
import shutil
import statistics
import subprocess
import sys
import tempfile
import time

from PIL import Image

REPO = Path(__file__).resolve().parents[4]


def command(root, mode):
    return ['bash', str(REPO / 'spikes/toolchain-sandbox.sh'),
            str(root / 'input'), str(root / 'output'), '60', '/toolchain/renderer', mode]


def check(root):
    for name in ('input', 'output', 'tools'):
        (root / name).mkdir()
    shutil.copy2(REPO / 'spikes/typst-mapping/target/release/scholium-spike-typst',
                 root / 'tools/renderer')
    source = (REPO / 'docs/spikes/evidence/SPK-0028/theme/generated.typ').read_text()
    env = dict(os.environ, SCHOLIUM_SPIKE_TOOLS=str(root / 'tools'),
               SCHOLIUM_SANDBOX_PROFILE='typst-editor')
    process = subprocess.Popen(command(root, 'editor-stream'), env=env,
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    rows, previous = [], set()
    try:
        for index in range(21):
            current = source + '\n' + 'a' * index
            (root / 'input/main.typ').write_text(current)
            started = time.perf_counter()
            process.stdin.write(b'1\n')
            process.stdin.flush()
            assert selector.select(20), 'request deadline exceeded'
            reply = process.stdout.readline().decode().strip()
            assert reply.startswith('ok '), reply
            scene = json.loads((root / 'output/scene.json').read_text())
            ids = {tile['id'] for page in scene for tile in page['tiles']}
            rows.append(dict(edit=index, roundtrip_ms=(time.perf_counter() - started) * 1000,
                             helper_ms=float(reply.split()[1]), changed_tiles=len(ids - previous),
                             total_tiles=len(ids)))
            assert len(list((root / 'output').glob('tile-*.rgba'))) == len(ids), 'obsolete tiles leaked'
            previous = ids
        process.stdin.close()
        assert process.wait(timeout=5) == 0, process.stderr.read().decode()
        subprocess.run(command(root, 'editor-export'), env=env, check=True, capture_output=True)
        reference = json.loads((root / 'output/scene.json').read_text())
        assert len(reference) == len(scene)
        for index, (page, original) in enumerate(zip(scene, reference)):
            assert page['boxes'] == original['boxes'], 'source geometry differs'
            raster = Image.new('RGBA', tuple(page['raster']))
            for tile in page['tiles']:
                image = Image.frombytes('RGBA', (tile['width'], tile['height']),
                                       (root / f"output/tile-{tile['id']}.rgba").read_bytes())
                raster.paste(image, (tile['x'], tile['y']))
            assert raster.tobytes() == Image.open(root / f'output/page-{index}.png').convert('RGBA').tobytes(), 'pixel mismatch'
        return dict(pixel_and_geometry_parity='pass', bounded_tile_files='pass',
                    warm_median_ms=statistics.median(r['roundtrip_ms'] for r in rows[1:]), runs=rows)
    finally:
        selector.close()
        if process.poll() is None:
            process.kill()
            process.wait(timeout=5)


if __name__ == '__main__':
    with tempfile.TemporaryDirectory(prefix='scholium-stream-check-') as folder:
        result = check(Path(folder))
    destination = Path(sys.argv[1])
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))
