#!/usr/bin/env python3
"""Start-page, picker, settings and conflict contracts; bilingual light/dark evidence."""
import json
import os
from pathlib import Path
import sys
import time
import subprocess
from project_ui import ProjectApp
from chrome_ui import Chrome

from check_evidence import evidence_enabled
from log_reader import log_entries, log_latest

binary, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
baseline = os.environ.get('SCHOLIUM_T45_BASELINE') == '1'
results = {}
failures = []


def chrome_states(app):
    return log_entries(app.log, ("PROJECT ", "AUDIT "))


def close_dialog(app):
    """Escape, then wait for a frame reported after it with no dialog open.

    Clicking or typing before the dialog has gone sends the input to the dialog's backdrop.
    """
    before = len(chrome_states(app))
    app.key('Escape')
    app.wait(lambda: any(not s.get('chrome', {}).get('facts', {}).get('file_modal', True)
                         for s in chrome_states(app)[before:]))


def run(theme):
    folder = output / theme
    folder.mkdir()
    app = ProjectApp(binary, folder, folder / 'initial')
    # Persist/UI checks do not require an installed compiler or bundle.
    app.env['SCHOLIUM_RTEX_BIN'] = str(folder / 'unavailable-rtex')
    app.env['SCHOLIUM_RTEX_TEXDIR'] = str(folder / 'unavailable-tex')

    def raw():
        return log_latest(app.log, ("AUDIT ", "PROJECT ")) or {}

    def visible():
        return {'controls': raw().get('controls', {})}

    def modal(dialog_id):
        return app.wait(lambda: next((d for d in app.entries('DIALOG_FRAME ')[-1:] if d['id'] == dialog_id), None))

    def fill(control, value):
        app.control(control)
        app.key('ctrl+a')
        app.type(str(value))

    def capture(name):
        if evidence_enabled():
            time.sleep(0.25)
        app.capture(name)
        if not baseline and name in ('new-project', 'project-settings', 'external-conflict'):
            title = app.entries('DIALOG_FRAME ')[-1]['title']
            assert '…' not in title, title
            if name == 'external-conflict':
                assert 'UTC' not in title, title
        if not baseline and name in ('new-project', 'project-settings', 'external-conflict'):
            frames = len(app.entries('DIALOG_FRAME '))
            app.xdo('windowsize', app.window, 480, 600)
            def settled():
                dialogs = app.entries('DIALOG_FRAME ')
                if len(dialogs) < frames + 2:
                    return False
                x0, y0, x1, y1 = dialogs[-1]['rect']
                return x0 >= 15.5 and y0 >= 15.5 and x1 <= 464.5 and y1 <= 584.5
            app.wait(settled)
            rect = app.entries('DIALOG_FRAME ')[-1]['rect']
            assert rect[0] >= 15.5 and rect[1] >= 15.5 and rect[3] <= 584.5, rect
            app.capture(name + '-480')
            app.xdo('windowsize', app.window, 1280, 900)
            time.sleep(0.25)

    def close():
        Chrome(visible, app.xdo, app.window).menu('ui.menu.file', 'file.close')
        app.source(lambda s: s.get('closed'))

    def open_path(path):
        frames = len(app.entries('DIALOG_FRAME '))
        app.key('ctrl+o')
        app.wait(lambda: len(app.entries('DIALOG_FRAME ')) >= frames + 2)
        modal('file-picker')
        fill('path', path)
        app.key('Return')

    try:
        app.start('dialogs')
        app.state()
        native = Chrome(lambda: app.state(), app.xdo, app.window)
        if theme == 'dark':
            native.menu('ui.menu.view', 'ui.menu.view.theme', 'view.theme.dark')
            app.state(lambda s: s['chrome']['facts']['theme'] == 'Dark')
        native.menu('ui.menu.file', 'file.close')
        app.source(lambda s: s.get('closed'))
        if not baseline:
            app.wait(lambda: 'environment.0' in visible()['controls'])
            labels = app.entries('ENVIRONMENT ')[-1]['items']
            assert labels and all(label.count(' — ') <= 1 for label in labels), labels
            assert not any('This is ' in label or '—:' in label for label in labels), labels
        capture('start-page')
        if not baseline:
            for _ in range(4):
                app.key('Down')
            app.key('Return')
            app.wait(lambda: raw().get('current') and not raw().get('closed'))
            app.state(lambda s: s.get('current'))
            close()
        app.key('ctrl+n')
        app.control('path')
        capture('new-project')
        close_dialog(app)
        app.key('ctrl+o')
        app.control('path')
        capture('open')
        close_dialog(app)
        if baseline:
            # Open before-state source through the old chooser to capture conflict/settings absence.
            root = folder / 'paper'
            root.mkdir()
            entry = root / 'main.tex'
            entry.write_text('original')
            app.key('ctrl+o')
            fill('path', entry)
            app.control('submit')
            app.source(lambda s: s.get('language') == 'Latex')
            capture('project-settings-absent')
        else:
            app.key('ctrl+o')
            modal('file-picker')
            close_dialog(app)
            app.control('recent.0')
            app.state(lambda s: s.get('current'))
            close()
            app.control('remove.0')
            app.source(lambda s: not s.get('recent'))
            app.key('ctrl+n')
            modal('project-chooser')
            app.key('Return')
            app.wait(lambda: app.entries('PROJECT_FORM ')[-1].get('error') == 'project.new.name_error')
            existing = folder / 'existing'
            existing.mkdir()
            fill('name', 'existing')
            fill('path', folder)
            app.key('Return')
            app.wait(lambda: app.entries('PROJECT_FORM ')[-1].get('error') == 'project.new.exists')
            capture('new-existing-error')
            close_dialog(app)
            assert not (existing / '.scholium').exists()
            app.key('ctrl+n')
            modal('project-chooser')
            fill('name', 'paper')
            fill('path', folder)
            assert app.entries('PROJECT_FORM ')[-1]['latex'], 'LaTeX is the default'
            app.key('Return')
            app.source(lambda s: s.get('language') == 'Latex')
            root = folder / 'paper'
            entry = root / 'main.tex'
            original = entry.read_bytes()
            app.key('ctrl+alt+p')
            modal('project-settings')
            capture('project-settings')
            app.control('engine')
            app.control('engine.XeLaTeX')
            app.key('Return')
            app.wait(lambda: 'engine = "XeLatex"' in (root / '.scholium/project.toml').read_text())
            time.sleep(0.25)
            status = app.source()['chrome']['facts']['status']['engine_state']
            engine_applied = app.state(lambda s: s.get('kind') == 'latex')['engine'] == 'XeLatex'
            if not engine_applied or 'XeLaTeX' not in status:
                failures.append(f'{theme}: declared XeLaTeX was not applied to preview: {status}')
            assert entry.read_bytes() == original
            # Status engine segment opens exactly the same settings dialog.
            Chrome(visible, app.xdo, app.window).click('document.project_settings', 'status')
            modal('project-settings')
            capture('project-settings-xelatex')
            close_dialog(app)
        # Durable draft plus external writer; every conflict action must preserve disk bytes.
        s = app.state(lambda s: s.get('kind') == 'latex' and s.get('source_rect'))
        x, y, _, _ = s['source_rect']
        app.xdo('mousemove', '--window', app.window, round(x + 40), round(y + 50))
        app.xdo('click', 1)
        app.key('ctrl+a')
        app.type('private draft')
        app.source(lambda s: s.get('text') == 'private draft' and s.get('dirty'))
        entry.write_text('external writer\n')
        app.key('ctrl+s')
        app.source(lambda s: bool(s.get('error')))
        capture('external-conflict')
        if not baseline:
            app.control('diff')
            capture('external-diff')
            assert entry.read_text() == 'external writer\n'
            app.control('cancel')
            app.source(lambda s: s.get('text') == 'private draft' and s.get('dirty'))
            app.key('ctrl+s')
            app.control('save-as')
            fill('path', folder / 'saved-copy')
            capture('save-as')
            app.key('Return')
            app.source(lambda s: not s.get('dirty') and s.get('text') == 'private draft')
            assert (folder / 'saved-copy/main.tex').read_text() == 'private draft'
            assert entry.read_text() == 'external writer\n'
            s = app.state(lambda s: s.get('kind') == 'latex' and s.get('source_rect'))
            x, y, _, _ = s['source_rect']
            app.xdo('mousemove', '--window', app.window, round(x + 40), round(y + 50))
            app.xdo('click', 1)
            app.key('ctrl+a')
            app.type('discard this draft')
            app.source(lambda s: s.get('text') == 'discard this draft' and s.get('dirty'))
            copy = folder / 'saved-copy/main.tex'
            copy.write_text('reload this external copy\n')
            app.key('ctrl+s')
            app.control('reload')
            app.source(lambda s: s.get('text') == 'reload this external copy\n' and not s.get('dirty'))
            assert copy.read_text() == 'reload this external copy\n'
            # Direct open and the multiple-entry chooser.
            multi = folder / 'multi'
            multi.mkdir()
            (multi / 'a.typ').write_text('a')
            (multi / 'b.typ').write_text('b')
            open_path(multi)
            modal('project-chooser')
            app.control('entry.b.typ')
            capture('entry-choice')
            app.key('Return')
            app.source(lambda s: s.get('entry') == 'b.typ' and s.get('text') == 'b')
            close()
            app.control('start.import')
            modal('project-chooser')
            capture('import')
            close_dialog(app)
            results[theme] = {'recent_click': True, 'recent_remove': True, 'keyboard': True,
                              'latex_default': True, 'inline_errors': True, 'engine_persisted': True, 'engine_applied': engine_applied,
                              'status_engine_updated': 'XeLaTeX' in status, 'diff_cancel_save_as': True, 'entry_choice': True}
        else:
            results[theme] = {'before': True, 'settings': 'absent', 'environment': 'absent'}
    finally:
        app.kill()


def notices(theme):
    folder = output / ('notices-' + theme)
    folder.mkdir()
    app = ProjectApp(binary, folder, folder / 'initial')
    held = folder / 'initial'
    holder = None

    def visible():
        return {'controls': (log_latest(app.log, ('PROJECT ', 'AUDIT ')) or {}).get('controls', {})}

    def fill(name, value):
        app.control(name)
        app.key('ctrl+a')
        app.type(str(value))

    def open_path(path):
        frames = len(app.entries('DIALOG_FRAME '))
        app.key('ctrl+o')
        app.wait(lambda: len(app.entries('DIALOG_FRAME ')) >= frames + 2)
        fill('path', path)
        app.control('submit')

    try:
        app.start('notices')
        app.state()
        chrome = Chrome(visible, app.xdo, app.window)
        if theme == 'dark':
            chrome.menu('ui.menu.view', 'ui.menu.view.theme', 'view.theme.dark')
        # The legacy head is genuine storage output; the original file is retained.
        legacy = folder / 'legacy.sqlite'
        hammer = Path(__file__).resolve().parents[2] / 'target/debug/hammer'
        subprocess.run([str(hammer), str(legacy), '1'], check=True, stdout=subprocess.DEVNULL)
        original = legacy.read_bytes()
        if baseline:
            app.key('ctrl+o')
            app.control('path')
            app.key('shift+Tab')
            app.key('space')
        else:
            chrome.menu('ui.menu.file', 'file.import')
        fill('path', legacy)
        fill('target', folder / 'imported')
        app.capture('import')
        app.control('submit')
        app.wait(lambda: any(d['id'] == 'project-migrated' for d in app.entries('DIALOG_FRAME ')))
        app.capture('migration')
        assert legacy.read_bytes() == original
        frames = len(app.entries('AUDIT '))
        app.key('Escape')
        app.wait(lambda: len(app.entries('AUDIT ')) > frames
                 and not app.entries('AUDIT ')[-1]['chrome']['facts']['file_modal'])
        app.state(lambda s: s.get('current') and not s['chrome']['facts']['file_modal'])
        lock_probe = Path(__file__).resolve().parents[2] / 'target/debug/project_hammer'
        holder = subprocess.Popen([str(lock_probe), str(held), 'lock'], stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, text=True)
        assert holder.stdout.readline().strip() == 'LOCKED'
        # Opening an already-held project names it and keeps the current project.
        open_path(held)
        if not baseline:
            app.wait(lambda: any(d['id'] == 'project-locked' for d in app.entries('DIALOG_FRAME ')))
        else:
            app.wait(lambda: any(d['id'] == 'project-error' for d in app.entries('DIALOG_FRAME ')))
        app.capture('project-locked')
        close_dialog(app)
        chrome.menu('ui.menu.file', 'file.close')
        app.source(lambda s: s.get('closed'))
        open_path(held)
        if baseline:
            close_dialog(app)
        else:
            app.source(lambda s: s.get('closed') and s.get('locked'))
        app.capture('start-locked')
        holder.terminate()
        holder.wait(timeout=15)
        if not baseline:
            app.control('start.retry')
            app.state(lambda s: s.get('current'))
    finally:
        if holder and holder.poll() is None:
            holder.terminate()
            holder.wait(timeout=15)
        app.kill()


for theme in ['light', 'dark']:
    notices(theme)
for theme in ['light', 'dark']:
    run(theme)
(output / 'summary.json').write_text(json.dumps(results, indent=2) + '\n')
assert not failures, '\n'.join(failures)
print('PASS start page/dialogs ' + ('baseline evidence' if baseline else 'behavior and light/dark evidence'))
