#!/usr/bin/env python3
"""Measure a runner process tree without GNU time.

Usage: python3 measure-run.py METRICS.json python3 scripts/window-checks.py ARGS...
Elapsed wall time includes runner lock waits; sum per-check seconds to exclude them.
"""
import json
from pathlib import Path
import resource
import subprocess
import sys
import time

start = time.monotonic()
boot = time.clock_gettime(time.CLOCK_BOOTTIME)
result = subprocess.run(sys.argv[2:])
usage = resource.getrusage(resource.RUSAGE_CHILDREN)
metrics = dict(
    wall_seconds=time.monotonic() - start,
    boot_seconds=time.clock_gettime(time.CLOCK_BOOTTIME) - boot,
    user_seconds=usage.ru_utime,
    system_seconds=usage.ru_stime,
    max_rss_kib=usage.ru_maxrss,
    exit_code=result.returncode,
)
Path(sys.argv[1]).write_text(json.dumps(metrics, indent=2) + '\n')
sys.exit(result.returncode)
