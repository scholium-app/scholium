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


def fragments():
    return [{'page':1,'first_line':1,'last_line':1,'xs':[655360],'baselines':[1310720]}]

def layout(failed=False):
    if args.mode == 'diagnostics':
        items = [{'severity': 'error', 'file': 'main.tex', 'line': 2, 'message': 'Undefined control sequence.'}] if '\\foo' in text else []
        emit({'event': 'Diagnostics', 'source': 'background', 'items': items})
    if args.mode == 'timeout-repair' and generation == 1 and 'loop ' in text:
        failed = True
        emit({'event': 'Diagnostics', 'source': 'background', 'items': [{
            'severity': 'error', 'message': 'background pass: lualatex was stopped after 30 s, the time limit of a pass (does the document loop forever?)'}]})
    value = {'event': 'LayoutUpdate', 'versions': {'source_revision': revision},
          'pages_total': 1, 'compile': {'state': 'Failed' if failed else 'Ok'},
          'unknown_future_field': {'ignored': True}}
    if args.mode == 'drawing':
        value.update(pages_changed=[{'page':1,'exact':True,'dl':{'page_width':4736287,'page_height':4736287,'lines':[{'x':655360,'y':1310720,'items':[['r',655360,655360,655360,655360]]}]}}], placements=[{'par_id':1,'fragments':fragments()}])
    emit(value)


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
        if args.mode == 'timeout-repair':
            layout()
        if args.mode == 'diagnostics':
            emit({'event': 'Diagnostics', 'source': 'background', 'versions': {'source_revision': revision - 1}, 'items': [{'severity': 'error', 'file': 'stale.tex', 'line': 1, 'message': 'stale'}]})
            layout()
        if args.mode in ('restarting', 'failed', 'pass-timeout'):
            emit({'event': 'EngineState', 'state': 'Restarting' if args.mode == 'restarting' else 'Failed',
                  'reason': 'injected upstream pass timeout' if args.mode == 'pass-timeout' else 'injected live failure'})
            # Deliberately lose the in-flight paragraph update, as observed in E8.
        else:
            value = {'event': 'ParagraphUpdate', 'par_id': revision if args.mode == 'distinct-paragraphs' else 1, 'versions': {'source_revision': revision}}
            if args.mode == 'drawing':
                value.update(dl={'lines':[{'x':0,'y':0,'items':[['r',0,-655360,1310720,655360]]}]}, fragments=fragments())
            emit(value)
    elif cmd == 'status':
        emit({'reply': 'status', 'versions': {'source_revision': revision}})
    elif cmd == 'request_layout':
        emit({'reply': cmd})
        layout(args.mode in ('pass-timeout', 'missing-package'))
    elif cmd == 'spans':
        emit({'reply': 'spans', 'spans': [{'range': {'start': 0, 'end': len(text.encode())}}]})
