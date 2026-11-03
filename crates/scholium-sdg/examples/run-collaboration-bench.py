#!/usr/bin/env python3
"""Run four pinned graphs in separate sequential processes, retaining every receive."""
import argparse
import json
import math
from pathlib import Path
import statistics
import subprocess
import time


def percentile(values, q):
    return sorted(values)[math.ceil(len(values) * q) - 1]


def summarize(rows):
    groups = {}
    for row in rows:
        key = (row['engine'], row['mode'], row['workload'],
               row['actions_per_side'], row['batch_actions'])
        groups.setdefault(key, []).append(row)
    output = []
    for key, samples in sorted(groups.items()):
        group = dict(zip(('engine', 'mode', 'workload', 'actions_per_side', 'batch_actions'), key))
        group['repeats'] = len(samples)
        for metric in ('receive_ms', 'engine_import_ms', 'index_rebuild_ms'):
            values = [v for sample in samples for v in sample[metric]]
            group[metric] = dict(n=len(values), p50=percentile(values, .5),
                                 p99=percentile(values, .99), maximum=max(values),
                                 two_way_median=statistics.median(sum(s[metric]) for s in samples),
                                 two_way_maximum=max(sum(s[metric]) for s in samples))
        group['update_bytes_total_median'] = statistics.median(s['update_bytes_total'] for s in samples)
        output.append(group)
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('legacy', type=Path)
    parser.add_argument('current', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--repeats', type=int, default=3)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--breakdown', action='store_true')
    modes.add_argument('--criteria', action='store_true')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    engines = [(args.legacy.resolve(), 'loro'), (args.current.resolve(), 'loro'),
               (args.legacy.resolve(), 'yrs'), (args.current.resolve(), 'yrs')]
    if args.criteria:
        engines = [(args.current.resolve(), 'loro')]
        scenarios = [('incremental', 'mixed', 50000, 100),
                     ('offline', 'mixed', 10000, 10000)]
    elif args.breakdown:
        scenarios = [('offline', name, n, n)
                     for n in (1000, 10000, 50000)
                     for name in ('text_insert', 'text_delete', 'node_create', 'node_delete',
                                  'wrap', 'unwrap', 'cross_parent_move')]
    else:
        scenarios = [('incremental', 'mixed', 1000 if n == 1 else 10000, n) for n in (100, 10, 1)]
        scenarios += [('offline', 'mixed', n, n) for n in (1000, 10000, 50000)]
    rows = []
    with (args.output / 'raw.jsonl').open('w') as raw:
        for scenario in scenarios:
            for repeat in range(args.repeats):
                order = engines if repeat % 2 == 0 else engines[::-1]
                for binary, engine in order:
                    mono = time.clock_gettime(time.CLOCK_MONOTONIC)
                    boot = time.clock_gettime(time.CLOCK_BOOTTIME)
                    result = subprocess.run([str(binary), engine, *map(str, scenario)],
                                            check=True, capture_output=True, text=True)
                    elapsed_mono = time.clock_gettime(time.CLOCK_MONOTONIC) - mono
                    elapsed_boot = time.clock_gettime(time.CLOCK_BOOTTIME) - boot
                    row = json.loads(result.stdout)
                    row.update(repeat=repeat, elapsed_monotonic_s=elapsed_mono,
                               elapsed_boottime_s=elapsed_boot, suspend_delta_s=elapsed_boot-elapsed_mono)
                    raw.write(json.dumps(row) + '\n')
                    raw.flush()
                    if abs(elapsed_boot-elapsed_mono) > 1:
                        raise RuntimeError('suspension detected; discard the whole run')
                    rows.append(row)
                    print(row['engine'], *scenario, 'repeat', repeat,
                          'merge p99 ms', round(percentile(row['receive_ms'], .99), 3), flush=True)
                    (args.output / 'summary.json').write_text(json.dumps(summarize(rows), indent=2) + '\n')

    if args.criteria:
        if len(rows) != args.repeats * 2 or args.repeats < 1:
            raise RuntimeError('criteria require complete repeated samples')
        p99 = max(percentile(r['receive_ms'], .99) for r in rows if r['mode'] == 'incremental')
        offline = max(sum(r['receive_ms']) for r in rows if r['mode'] == 'offline')
        verdict = dict(engine=rows[0]['engine'], worst_run_incremental_p99_ms=p99,
                       worst_run_offline_two_way_ms=offline,
                       continue_loro=(p99 <= 16 and offline <= 500))
        (args.output / 'criteria.json').write_text(json.dumps(verdict, indent=2) + '\n')
        print(json.dumps(verdict), flush=True)
        if not verdict['continue_loro']:
            raise RuntimeError('Loro acceptance criteria failed; report without switching engines')


if __name__ == '__main__':
    main()
