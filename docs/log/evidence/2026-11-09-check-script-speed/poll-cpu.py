#!/usr/bin/env python3
"""Measure idle-poll CPU against a completed real-window app log, including first read.

Usage: python3 docs/log/evidence/2026-11-09-check-script-speed/poll-cpu.py LOG... > metrics.json
The source logs are generated fixtures from window-checks.py, never user documents.
"""
import hashlib
import json
from pathlib import Path
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / 'spikes/typst-main-editor'))
from log_reader import LogReader  # noqa: E402

POLLS = 100
results = []
for name in sys.argv[1:]:
    path = Path(name)
    size = path.stat().st_size
    start = time.process_time()
    for _ in range(POLLS):
        lines = [line for line in path.read_text().split('\n')[:-1] if line.startswith('AUDIT ')]
        old = json.loads(lines[-1][6:]) if lines else None
    old_cpu = time.process_time() - start
    subscription = LogReader(path, ('AUDIT ',))
    start = time.process_time()
    for _ in range(POLLS):
        new = subscription.latest()
    new_cpu = time.process_time() - start
    assert old == new
    assert subscription.bytes_read == size
    results.append(dict(log=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                        polls=POLLS, log_bytes=size, legacy_bytes=size * POLLS,
                        incremental_bytes=subscription.bytes_read,
                        legacy_cpu_seconds=old_cpu, incremental_cpu_seconds=new_cpu))
print(json.dumps(results, indent=2))
