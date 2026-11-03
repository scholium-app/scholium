#!/usr/bin/env python3
"""Test-only JSON-lines fault injector; never reachable from the public launcher."""
import argparse
import json
import os
from pathlib import Path
import signal
import sys
import time

parser = argparse.ArgumentParser()
parser.add_argument('--directory', type=Path, required=True)
parser.add_argument('--mode', required=True)
args, rest = parser.parse_known_args()
args.directory.mkdir(exist_ok=True)
count = args.directory / 'count'
generation = int(count.read_text()) + 1 if count.exists() else 1
count.write_text(str(generation))
with (args.directory / 'launch.jsonl').open('a') as file:
    file.write(json.dumps({'pid': os.getpid(), 'argv': rest}) + '\n')
if args.mode == 'startup-exit':
    print('fake startup failed', file=sys.stderr, flush=True)
    sys.exit(2)
if args.mode == 'stderr-closed':
    os.close(2)
revision = 0
text = ''
hung = False


def emit(value):
    print(json.dumps(value), flush=True)


def layout(failed=False):
    emit({'event': 'LayoutUpdate', 'versions': {'source_revision': revision},
          'pages_total': 1, 'compile': {'state': 'Failed' if failed else 'Ok'},
          'unknown_future_field': {'ignored': True}})


for raw in sys.stdin:
    command = json.loads(raw)
    with (args.directory / 'commands.jsonl').open('a') as file:
        file.write(json.dumps({'generation': generation, **command}) + '\n')
    cmd = command.get('cmd')
    if cmd == 'set_document':
        text = command['text']
        revision += 1
        emit({'reply': cmd, 'result': {'source_revision': 'bad' if args.mode == 'bad-ack' else revision}})
        if args.mode == 'malformed':
            print('{bad json', flush=True)
            emit({'event': 'ParagraphUpdate', 'par_id': 'wrong'})
            emit({'event': 'FutureEvent', 'anything': True})
        if args.mode == 'long-line':
            print('x' * 5000, flush=True)
        if args.mode == 'total-flood':
            for _ in range(1000):
                emit({'event': 'FutureEvent', 'padding': 'x' * 100})
        if args.mode == 'missing-package':
            emit({'event': 'EngineState', 'state': 'Failed', 'reason': "File `missing.sty' not found"})
        else:
            emit({'event': 'EngineState', 'state': 'Ready', 'reason': None})
    elif cmd == 'edit':
        start, end = command['start'], command['end']
        data = text.encode()
        text = (data[:start] + command['text'].encode() + data[end:]).decode()
        revision += 1
        if generation == 1 and args.mode == 'crash':
            os._exit(9)
        if generation == 1 and args.mode == 'hang':
            signal.pause()
        emit({'reply': cmd, 'result': {'source_revision': revision}})
        if args.mode in ('restarting', 'failed', 'pass-timeout'):
            emit({'event': 'EngineState', 'state': 'Restarting' if args.mode == 'restarting' else 'Failed',
                  'reason': 'injected upstream pass timeout' if args.mode == 'pass-timeout' else 'injected live failure'})
            # Deliberately lose the in-flight paragraph update, as observed in E8.
        else:
            emit({'event': 'ParagraphUpdate', 'par_id': 1, 'versions': {'source_revision': revision}})
    elif cmd == 'status':
        emit({'reply': 'status', 'versions': {'source_revision': revision}})
    elif cmd == 'request_layout':
        emit({'reply': cmd})
        layout(args.mode in ('pass-timeout', 'missing-package'))
    elif cmd == 'spans':
        emit({'reply': 'spans', 'spans': [{'range': {'start': 0, 'end': len(text.encode())}}]})
