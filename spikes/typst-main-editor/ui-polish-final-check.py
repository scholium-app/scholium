#!/usr/bin/env python3
"""Breadcrumb jumps, caret-safe command popups, and the final bilingual surface matrix."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from project_ui import ProjectApp
from chrome_ui import Chrome

binary, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
baseline = os.environ.get('SCHOLIUM_T48_BASELINE') == '1'
results = {}


def latest(app, prefix, predicate=lambda s: True):
    return app.wait(lambda: next((s for s in app.entries(prefix)[-1:] if predicate(s)), None))


def raw(app):
    for line in reversed(app.log.read_text().split('\n')[:-1]):
        if line.startswith(('AUDIT ', 'PROJECT ')):
            return json.loads(line.split(' ', 1)[1])
    return {}


def paste(app, text):
    p = subprocess.Popen(['xclip', '-selection', 'clipboard', '-in'], stdin=subprocess.PIPE, env=app.env)
    p.communicate(text.encode())
    app.key('ctrl+v')
    time.sleep(0.2)


def record(name, condition, **facts):
    results[name] = {'pass': bool(condition), **facts}
    assert condition, (name, facts)
    print('PASS ' + name, flush=True)


def matrix(app, name):
    c = Chrome(lambda: raw(app), app.xdo, app.window)
    for theme in ('light', 'dark'):
        # Menus remain accessible at every supported width.
        c.menu('ui.menu.view', 'ui.menu.view.theme', 'view.theme.' + theme)
        app.wait(lambda: raw(app).get('chrome', {}).get('facts', {}).get('theme') == theme.title())
        for width, height in ((1440, 900), (1024, 700), (480, 700)):
            app.xdo('windowsize', app.window, width, height)
            time.sleep(0.35)
            app.capture(f'{name}-{theme}-{width}')
        app.xdo('windowsize', app.window, 1280, 900)
        time.sleep(0.25)


def dialog_matrix(app, name, open_dialog):
    c = Chrome(lambda: raw(app), app.xdo, app.window)
    for theme in ('light', 'dark'):
        c.menu('ui.menu.view', 'ui.menu.view.theme', 'view.theme.' + theme)
        for width, height in ((1440, 900), (1024, 700), (480, 700)):
            app.xdo('windowsize', app.window, width, height)
            time.sleep(0.25)
            open_dialog()
            time.sleep(0.35)
            app.capture(f'{name}-{theme}-{width}')
            if not baseline:
                d = app.entries('DIALOG_FRAME ')[-1]
                x0, y0, x1, y1 = d['rect']
                record(f'{name}-{theme}-{width}-fits', x0 >= 15 and y0 >= 15 and x1 <= width - 15 and y1 <= height - 15, rect=d['rect'])
            app.key('Escape')
        app.xdo('windowsize', app.window, 1280, 900)
        time.sleep(0.25)


def command_matrix(app, source):
    c = Chrome(lambda: raw(app), app.xdo, app.window)
    for theme in ('light', 'dark'):
        c.menu('ui.menu.view', 'ui.menu.view.theme', 'view.theme.' + theme)
        for width, height in ((1440, 900), (1024, 700), (480, 700)):
            app.xdo('windowsize', app.window, width, height)
            time.sleep(0.2)
            if source:
                app.control('source')
            else:
                c.click('view.mode.source', 'focus')
                c.click('view.mode.page', 'focus')
            app.key('ctrl+Home')
            app.type('\\al')
            if not baseline:
                latest(app, 'COMMAND_BOX ', lambda p: p['query'] == 'al')
            app.capture(f'command-{theme}-{width}')
            app.key('Escape')
        app.xdo('windowsize', app.window, 1280, 900)
        time.sleep(0.25)


def cursor_context(app, kind):
    app.key('ctrl+Home')
    for _ in range(4 if kind == 'tex' else 2):
        app.key('Down')
    app.key('End')
    if kind == 'typ':
        app.key('Left')


def source_case(kind, text):
    folder = output / kind
    folder.mkdir()
    project = folder / ('main.' + kind)
    project.write_text(text)
    app = ProjectApp(binary, folder, project)
    app.env['SCHOLIUM_RTEX_BIN'] = str(folder / 'missing-rtex')
    app.env['SCHOLIUM_RTEX_TEXDIR'] = str(folder / 'missing-tex')
    app.start(kind)
    try:
        app.source(lambda s: s.get('text') == text)
        c = Chrome(lambda: raw(app), app.xdo, app.window)
        app.control('source')
        cursor_context(app, kind)
        if not baseline:
            s = latest(app, 'BREADCRUMB ', lambda s: s['items'][-1]['label'] == ('equation' if kind == 'tex' else 'math'))
            record(kind + '-cursor-context', len(s['items']) == 4, items=s['items'])
            for index in (2, 1, 0):
                c.click(f'ui.breadcrumb.{index}', 'focus')
                target = s['items'][index]['byte']
                expected = [text[:target].count('\n') + 1, len(text[:target].rsplit('\n', 1)[-1]) + 1]
                app.wait(lambda: raw(app).get('chrome', {}).get('facts', {}).get('status', {}).get('source_position') == expected)
                record(f'{kind}-jump-{index}', True, position=expected)
            app.control('source')
            cursor_context(app, kind)
        matrix(app, kind + '-source')
        if kind == 'tex':
            dialog_matrix(app, 'settings', lambda: app.key('ctrl+alt+p'))
            app.control('source')
            app.key('ctrl+Home')
            app.type('\\al')
            if not baseline:
                p = latest(app, 'COMMAND_BOX ', lambda p: not p['above'])
                record('source-popup-below-caret', p['rect'][1] >= p['caret'][3], popup=p)
            app.capture('command-top')
            app.key('Escape')
            app.key('ctrl+End')
            paste(app, '\n' * 55)
            app.type('\\')
            if not baseline:
                p = latest(app, 'COMMAND_BOX ', lambda p: p['above'])
                record('source-popup-flips-above', p['rect'][3] <= p['caret'][1], popup=p)
            app.capture('command-bottom')
            app.key('Escape')
            # A real durable draft and external edit exercise the conflict dialog.
            project.write_text('external edit\n')
            dialog_matrix(app, 'conflict', lambda: app.key('ctrl+s'))
        command_matrix(app, True)
        c.menu('ui.menu.view', 'view.diagnostics')
        matrix(app, kind + '-problems')
        c.menu('ui.menu.view', 'view.diagnostics')
        c.click('view.mode.page', 'focus')
        matrix(app, kind + '-preview')
    finally:
        app.kill()


def native_case():
    folder = output / 'native'
    folder.mkdir()
    app = ProjectApp(binary, folder, folder / 'paper')
    app.start('native')
    try:
        app.state()
        paste(app, 'Scientific typesetting 中文\nAn editable native page.')
        app.state(lambda s: s['current'] and s['snapshot']['blocks'][0]['content'][0]['body']['text'] == 'Scientific typesetting 中文')
        matrix(app, 'native-page')
        c = Chrome(lambda: raw(app), app.xdo, app.window)
        c.menu('ui.menu.view', 'view.navigation')
        matrix(app, 'navigation')
        c.menu('ui.menu.view', 'view.navigation')
        c.click('insert.formula', 'toolbar')
        app.type('x+y')
        app.state(lambda s: s['current'] and s['chrome']['facts']['status']['context'] == 'chrome.status.context.math')
        matrix(app, 'native-math')
        app.key('Escape')
        c.click('view.mode.source', 'focus')
        matrix(app, 'generated-source')
        c.click('view.mode.page', 'focus')
        app.key('ctrl+End')
        app.type('\\al')
        if not baseline:
            p = latest(app, 'COMMAND_BOX ')
            record('page-popup-clears-caret', p['rect'][1] >= p['caret'][3] or p['rect'][3] <= p['caret'][1], popup=p)
        app.capture('page-command')
        app.key('Escape')
        app.xdo('windowsize', app.window, 1280, 700)
        paste(app, '\n' * 30)
        app.state(lambda s: s['current'] and len(s['snapshot']['blocks']) >= 30)
        app.type('\\')
        if not baseline:
            p = latest(app, 'COMMAND_BOX ', lambda p: p['above'])
            record('page-popup-flips-above', p['rect'][3] <= p['caret'][1], popup=p)
        app.capture('page-command-bottom')
        app.key('Escape')
        app.xdo('windowsize', app.window, 1280, 900)
        command_matrix(app, False)
        app.key('ctrl+s')
        c.menu('ui.menu.file', 'file.close')
        app.source(lambda s: s.get('closed'))
        matrix(app, 'start')
        dialog_matrix(app, 'new-project', lambda: app.key('ctrl+n'))
        dialog_matrix(app, 'open', lambda: app.key('ctrl+o'))
    finally:
        app.kill()


try:
    source_case('tex', '\\documentclass{article}\n\\section{Introduction}\n\\subsection{Method}\n\\begin{equation}\nx\n\\end{equation}\n')
    source_case('typ', '= Introduction\n== Method\n$x$\n')
    native_case()
finally:
    (output / 'summary.json').write_text(json.dumps(results, ensure_ascii=False, indent=2) + '\n')
