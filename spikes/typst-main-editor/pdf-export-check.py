#!/usr/bin/env python3
"""Real command/keymap PDF exports: unsaved native math, source success and syntax failure."""
import json
import re
import subprocess
import sys
import time
import tomllib
from pathlib import Path
from project_ui import ProjectApp
from chrome_ui import Chrome

binary, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
results = {}
root = Path(__file__).resolve().parents[2]
keymap = tomllib.loads((root / 'crates/scholium-app/src/command/default-keymap.toml').read_text())


def chord(command):
    binding = next(b for b in keymap['binding'] if b['command'] == command)
    return binding['keys'][0].lower()


def record(name, condition, **facts):
    results[name] = {'pass': bool(condition), **facts}
    assert condition, (name, facts)
    print('PASS ' + name, flush=True)


def paste(app, text):
    subprocess.run(['xclip', '-selection', 'clipboard'], input=text.encode(), env=app.env, check=True)
    app.key('ctrl+v')


def report(app, count):
    return app.wait(lambda: app.entries('PDF_EXPORT ')[-1] if len(app.entries('PDF_EXPORT ')) > count else None)


def export(app, target):
    count = len(app.entries('PDF_EXPORT '))
    app.key(chord('file.export'))
    app.control('path')
    app.key('ctrl+a')
    app.type(str(target))
    app.key('Return')
    return report(app, count)


def pdf_pages(path):
    assert path.read_bytes().startswith(b'%PDF-')
    info = subprocess.check_output(['pdfinfo', str(path)], text=True)
    return int(re.search(r'^Pages:\s+(\d+)', info, re.M).group(1))


def pdf_text(path):
    return ' '.join(subprocess.check_output(['pdftotext', str(path), '-'], text=True).split())


def native():
    folder = output / 'native'
    folder.mkdir()
    app = ProjectApp(binary, folder, folder / 'paper')
    app.env['SCHOLIUM_EXPORT_SETTINGS_DIR'] = str(folder / 'config')
    app.start('native')
    try:
        app.state()
        paste(app, 'Native export line\nFormula ')
        app.state(lambda s: s['current'] and len(s['snapshot']['blocks']) == 2)
        app.key(chord('insert.formula'))
        app.type('a')
        app.key('Right')
        app.state(lambda s: s['current'] and any(i['body']['kind'] == 'math' for b in s['snapshot']['blocks'] for i in b['content']))
        target = folder / 'current.pdf'
        done = export(app, target)
        record('native-unsaved-text-and-formula-pdf', done['success'] and done['unsaved'] and done['revision'] == app.state()['revision'] and done['page_count'] == pdf_pages(target) == 1 and not done['diagnostics']['items'], report=done)
        record('native-pdf-contains-current-text', 'Native export line' in pdf_text(target))
        app.key('ctrl+j')
        current = app.state()
        record('native-empty-problem-panel', not current['controls'].get('ui.diagnostics.row'))
        app.capture('native-export')
        # Last successful target is restored when the dialog reopens, including after a restart.
        app.key('Escape')
        app.kill()
        app.start('reopened')
        app.state()
        app.key(chord('file.export'))
        settings = json.loads(next((folder / 'config/scholium/project-settings').glob('*.json')).read_text())
        record('project-export-destination-survives-reopen', Path(settings) == target)
        app.key('Return')
        again = report(app, 0)
        record('reopened-export-uses-remembered-destination', again['success'] and Path(again['output_path']) == target)
    finally:
        app.kill()


def source():
    folder = output / 'source'
    folder.mkdir()
    project = folder / 'project'
    project.mkdir()
    entry = project / 'main.typ'
    entry.write_text('Initial source\n')
    app = ProjectApp(binary, folder, entry)
    app.env['SCHOLIUM_EXPORT_SETTINGS_DIR'] = str(folder / 'config')
    app.start('source')
    try:
        app.source()
        app.control('source')
        app.key('ctrl+a')
        paste(app, 'Unsaved source $frac(1,2)$')
        app.source(lambda s: s['dirty'] and s['text'].startswith('Unsaved'))
        target = folder / 'source.pdf'
        done = export(app, target)
        record('sandboxed-source-exports-unsaved-revision', done['success'] and done['unsaved'] and pdf_pages(target) == 1, report=done)
        record('source-pdf-contains-unsaved-editor-text', 'Unsaved source' in pdf_text(target))
        record('source-disk-remains-unchanged', entry.read_text() == 'Initial source\n')
        app.control('source')
        app.key('ctrl+a')
        paste(app, '#let broken = (')
        app.source(lambda s: s['text'] == '#let broken = (')
        failed_target = folder / 'must-not-exist.pdf'
        failed = export(app, failed_target)
        record('syntax-error-never-publishes-pdf', not failed['success'] and not failed_target.exists() and bool(failed['diagnostics']['items']), report=failed)
        app.source(lambda s: bool(s['controls'].get('ui.diagnostics.row')))
        record('export-error-reaches-problem-panel', True)
        app.capture('source-export-error')
    finally:
        app.kill()


try:
    native()
    source()
finally:
    (output / 'summary.json').write_text(json.dumps(results, ensure_ascii=False, indent=2))
