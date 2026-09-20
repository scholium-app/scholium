#!/usr/bin/env python3
"""Real Linux editor acceptance. Record failures, restore session settings, never imply Pass."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import traceback

import pyatspi
from evdev import UInput, AbsInfo, ecodes

ROOT = Path(__file__).resolve().parents[4]
os.chdir(ROOT)
OUT = Path(sys.argv[1] if len(sys.argv) > 1 else '/tmp/scholium-native-edit').resolve()
OUT.mkdir(parents=True, exist_ok=True)
BINARY = Path(os.environ.get('SCHOLIUM_SPIKE_BINARY',
              'spikes/native-ui/candidate-egui/target/debug/scholium-spike-egui')).resolve()


def call(*args):
    return subprocess.run(args, check=True, capture_output=True, text=True).stdout.strip()


def wait_for(predicate, message, timeout=8):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(0.15)
    raise AssertionError(message)


class Window:
    def __init__(self, name):
        self.name = name
        self.replicas = 'SCHOLIUM_SPIKE_REPLICAS' in os.environ
        self.path = OUT / f'{name}.log'
        self.log = self.path.open('w')
        self.process = subprocess.Popen(
            [str(BINARY)],
            stdout=self.log, stderr=self.log)
        self.app = None
        self.crash = False
        self.graceful = False

    def __enter__(self):
        try:
            self.window = wait_for(lambda: next((w for w in json.loads(call('niri', 'msg', '--json', 'windows'))
                                   if w.get('pid') == self.process.pid), None), 'window missing')
            call('niri', 'msg', 'action', 'focus-window', '--id', str(self.window['id']))
            self.app = wait_for(lambda: next((a for a in pyatspi.Registry.getDesktop(0)
                                            if a.get_process_id() == self.process.pid), None), 'AT-SPI missing')
            expected = '源码权威 · Loro 双副本实验' if self.replicas else '正文结构编辑器'
            wait_for(lambda: any(n.name == expected for n in self.nodes()), 'editor missing')
            time.sleep(0.5)
            self.wait_current()
            return self
        except Exception:
            self.process.terminate()
            self.process.wait(timeout=5)
            self.log.close()
            raise

    def __exit__(self, *args):
        try:
            call('niri', 'msg', 'action', 'screenshot-window', '--id', str(self.window['id']),
                 '--path', str(OUT / f'{self.name}.png'), '-p', 'false')
            records = []
            for node in self.nodes():
                record = {'role': node.getRoleName(), 'name': node.name,
                          'description': node.description, 'interfaces': list(node.get_interfaces())}
                try:
                    text = node.queryText()
                    record.update(text=text.getText(0, -1), caret=text.caretOffset,
                                  selections=[list(text.getSelection(i)) for i in range(text.getNSelections())])
                except NotImplementedError:
                    pass
                records.append(record)
            (OUT / f'{self.name}.json').write_text(json.dumps(records, ensure_ascii=False, indent=2))
        finally:
            if self.graceful:
                call('niri', 'msg', 'action', 'close-window', '--id', str(self.window['id']))
            elif self.crash:
                self.process.kill()
            else:
                self.process.terminate()
            self.process.wait(timeout=5)
            self.log.close()

    def nodes(self):
        def walk(node):
            yield node
            for child in node:
                yield from walk(child)
        return list(walk(self.app))

    def named(self, name):
        return next(n for n in self.nodes() if n.name == name)

    def body(self):
        return self.named('正文结构编辑器').queryText()

    def source(self):
        return next(n for n in self.nodes() if n.getRoleName() == 'entry' and n.name != '正文结构编辑器')

    def text(self):
        self.wait_current()
        return self.body().getText(0, -1)

    def wait_current(self):
        if self.replicas:
            return
        def ready():
            nodes = self.nodes()
            revisions = [re.search(r'^revision (\d+) /', n.name or '') for n in nodes]
            revision = next((m.group(1) for m in revisions if m), None)
            return revision and any((n.name or '').startswith(f'Typst · revision {revision} ·') for n in nodes)
        wait_for(ready, 'current Typst editor scene unavailable', timeout=30)

    def focus_guard(self):
        current = next((w for w in json.loads(call('niri', 'msg', '--json', 'windows')) if w.get('is_focused')), None)
        assert current is not None and current['pid'] == self.process.pid, 'focus changed; injection stopped'

    def key(self, *keys):
        self.focus_guard()
        call('ydotool', 'key', *[f'{k}:1' for k in keys], *[f'{k}:0' for k in reversed(keys)])
        time.sleep(0.3)
        self.wait_current()

    def type(self, value):
        self.focus_guard()
        call('ydotool', 'type', value)
        time.sleep(0.4)
        self.wait_current()

    def select(self, start, end):
        assert self.body().setSelection(0, start, end)
        time.sleep(0.4)

    def action(self, name):
        assert self.named(name).queryAction().doAction(0)
        time.sleep(0.5)
        self.wait_current()

    def history(self):
        matches = re.findall(r'核心动作 (\d+)', self.path.read_text())
        return int(matches[-1])


def ime():
    with Window('ime') as w:
        w.select(0, 0)
        before = w.text()
        actions = w.history()
        call('fcitx5-remote', '-s', 'rime')
        call('fcitx5-remote', '-o')
        time.sleep(0.5)
        w.type('nihao')
        wait_for(lambda: '预编辑「' in w.path.read_text(), 'no real preedit observed')
        assert w.text() == before and w.history() == actions, 'preedit mutated document'
        w.key(1)  # Escape cancels composition.
        assert w.text() == before and w.history() == actions
        w.type('nihao')
        w.key(57)  # Space chooses candidate.
        wait_for(lambda: w.text() != before, 'IME did not commit')
        assert any('\u4e00' <= c <= '\u9fff' for c in w.text()[:2]), 'no Chinese candidate'
        assert w.history() == actions + 1, 'IME commit must be one action'
        source_before = w.source().queryText().getText(0, -1)
        w.type('ni')
        assert w.source().queryComponent().grabFocus()
        time.sleep(0.5)
        call('fcitx5-remote', '-c')
        time.sleep(0.4)
        assert w.source().queryText().getText(0, -1) == source_before, 'body preedit committed into source after focus transfer'
        w.type('SOURCE')
        assert 'SOURCE' in w.source().queryText().getText(0, -1)
        assert 'SOURCE' not in w.text(), 'focus transfer writes to body'


def source_dialects():
    with Window('source') as w:
        call('fcitx5-remote', '-c')
        for dialect in ['LaTeX', 'Typst']:
            w.action(dialect)
            before = w.text()
            assert w.source().queryComponent().grabFocus()
            time.sleep(0.3)
            w.key(29, 102)  # Ctrl+Home.
            w.type('CHECK')
            w.action('应用源码')
            assert w.text() == 'CHECK' + before, f'{dialect} source commit failed'
        assert w.source().queryComponent().grabFocus()
        time.sleep(0.3)
        w.key(29, 102)
        w.type('$broken(')
        draft = w.source().queryText().getText(0, -1)
        before = w.text()
        w.action('应用源码')
        assert w.text() == before, 'invalid draft changed authority'
        assert w.source().queryText().getText(0, -1) == draft, 'invalid draft lost'
        advertised_enabled = w.named('LaTeX').getState().contains(pyatspi.STATE_ENABLED)
        assert not advertised_enabled, 'disabled language button advertised as enabled'
        w.action('LaTeX')
        assert w.source().queryText().getText(0, -1) == draft, 'dirty dialect switch discarded draft'
        assert any((n.name or '').startswith('Typst ·') for n in w.nodes()), 'dirty dialect switch allowed'
        (OUT / 'disabled-state.json').write_text(json.dumps({'atspi_enabled': advertised_enabled,
            'switch_rejected': True}, indent=2))


def math_edit():
    with Window('math') as w:
        call('fcitx5-remote', '-c')
        original = w.text()
        numerator = original.index('a')
        w.select(numerator, numerator)
        w.key(108)  # Down: numerator -> denominator.
        assert w.body().caretOffset == original.index('b')
        w.key(103)
        assert w.body().caretOffset == numerator
        w.action('包裹为根式')
        assert '\\sqrt{a}' in w.source().queryText().getText(0, -1)
        w.action('解除结构')
        w.type('Q')
        assert 'Qa' in w.text(), 'unwrap lost editing focus or surviving leaf'
        w.key(29, 44)
        assert w.text() == original


def slot_selection():
    with Window('slot-selection') as w:
        call('fcitx5-remote', '-c')
        original = w.text()
        w.select(0, 0)
        w.action('包裹为根式')
        wrapped = w.source().queryText().getText(0, -1)
        w.action('选择整节点')
        assert w.body().getNSelections() == 1, 'slot selection missing from AT-SPI'
        start, end = w.body().getSelection(0)
        assert start == 0 and end > 0, (start, end)
        call('niri', 'msg', 'action', 'screenshot-window', '--id', str(w.window['id']),
             '--path', str(OUT / 'slot-selection-highlight.png'), '-p', 'false')
        w.type('REPLACED')
        assert w.text().startswith('REPLACED')
        w.key(29, 44)
        # ydotool submits separate keystrokes; undo once per resulting action.
        for _ in range(len('REPLACED')):
            if w.text() == original: break
            previous = w.text()
            w.key(29, 44)
            assert w.text() != previous, 'undo stopped restoring selected structure'
        assert w.text() == original, 'bounded undo sequence did not restore original'
        assert w.source().queryText().getText(0, -1) == wrapped
        w.action('选择整节点')
        w.key(14)
        w.type('AFTERCUT')
        assert 'AFTERCUT' in w.text(), 'typing after slot deletion failed'


def multipage_preview():
    saved_count = os.environ.get('SCHOLIUM_SPIKE_PARAGRAPHS')
    saved_metrics = os.environ.get('SCHOLIUM_SPIKE_FRAME_METRICS')
    os.environ['SCHOLIUM_SPIKE_PARAGRAPHS'] = '256'
    os.environ['SCHOLIUM_SPIKE_FRAME_METRICS'] = '1'
    try:
        with Window('multipage-preview') as w:
            call('fcitx5-remote', '-c')
            w.action('启用后台预览')
            def current():
                return any(re.search(r'预览 revision Some\((\d+)\) / 正文 \1$', n.name or '') for n in w.nodes())
            wait_for(current, 'large preview did not compile', timeout=60)
            label = next(n.name for n in w.nodes() if re.match(r'第 1 / \d+ 页$', n.name or ''))
            count = int(label.split('/')[1].split()[0])
            assert count >= 20, label
            w.action('下一页')
            assert any(n.name == f'第 2 / {count} 页' for n in w.nodes())
            marker = next(n.name for n in w.nodes() if (n.name or '').startswith('定位段落 '))
            before_caret = w.body().caretOffset
            w.action(marker)
            assert w.body().caretOffset != before_caret, 'preview marker did not move body caret'
            slider = next(n for n in w.nodes() if n.getRoleName() == 'slider' and '缩放' in (n.name or ''))
            slider.queryValue().currentValue = 1.5
            time.sleep(0.5)
            assert abs(slider.queryValue().currentValue - 1.5) < 0.01
            w.action('上一页')
            assert any(n.name == f'第 1 / {count} 页' for n in w.nodes())
            time.sleep(4)
            (OUT / 'preview-pages.txt').write_text(label + '\n' + w.path.read_text())
    finally:
        for key, value in [('SCHOLIUM_SPIKE_PARAGRAPHS', saved_count), ('SCHOLIUM_SPIKE_FRAME_METRICS', saved_metrics)]:
            if value is None: os.environ.pop(key, None)
            else: os.environ[key] = value


def frame_cpu_measurement():
    records = re.findall(
        r'\[frame-cpu\] samples=(\d+) p95_ms=([\d.]+) max_ms=([\d.]+) excludes_gpu_present=true',
        (OUT / 'multipage-preview.log').read_text())
    assert records, 'no real-window CPU frame samples recorded'
    windows = [dict(samples=int(count), p95_ms=float(p95), max_ms=float(peak))
               for count, p95, peak in records]
    (OUT / 'frame-cpu.json').write_text(json.dumps({
        'scope': '256 paragraphs; AT-SPI enabled; eframe CPU timing; excludes GPU presentation',
        'binary': str(BINARY),
        'result': 'Measured', 'windows': windows,
    }, indent=2))


def large_document_edit():
    saved_count = os.environ.get('SCHOLIUM_SPIKE_PARAGRAPHS')
    os.environ['SCHOLIUM_SPIKE_PARAGRAPHS'] = '256'
    try:
        with Window('large-document-edit') as w:
            call('fcitx5-remote', '-c')
            before = w.text()
            status = next(n.name for n in w.nodes() if (n.name or '').startswith('Typst · revision'))
            pages = int(re.search(r'· (\d+) 页$', status).group(1))
            assert pages >= 20, 'large editing fixture must actually span at least 20 pages'
            w.select(0, 0)
            w.type('LARGE')
            assert w.text() == 'LARGE' + before, 'large-document typing corrupted content'
            w.select(0, 5)
            w.key(14)
            assert w.text() == before, 'large-document selected deletion failed'
            w.key(29, 44)
            assert w.text() == 'LARGE' + before, 'large-document undo failed'
            length = len(w.text())
            w.select(length, length)
            w.type('END')
            assert w.text() == 'LARGE' + before + 'END', 'last-page edit failed'
            (OUT / 'large-document-edit-summary.json').write_text(json.dumps(
                {'paragraphs': 256, 'pages': pages, 'original_characters': len(before),
                 'checks': ['first-page typing', 'selected deletion', 'undo', 'last-page typing']}, indent=2))
    finally:
        if saved_count is None:
            os.environ.pop('SCHOLIUM_SPIKE_PARAGRAPHS', None)
        else:
            os.environ['SCHOLIUM_SPIKE_PARAGRAPHS'] = saved_count


def unicode_clipboard():
    with Window('unicode') as w:
        call('fcitx5-remote', '-c')
        w.select(0, 0)
        original = w.text()
        # Preserve the existing text clipboard in memory; never include it in evidence.
        saved_clipboard = subprocess.run(['wl-paste', '--no-newline'], capture_output=True)
        assert saved_clipboard.returncode == 0, 'text clipboard unavailable for reversible test'
        try:
            subprocess.run(['wl-copy'], input='中👩‍💻e\u0301'.encode(), check=True)
            w.key(29, 47)
            assert w.text().startswith('中👩‍💻e\u0301')
            for prefix in ['中👩‍💻', '中', '']:
                w.key(14)
                assert w.text() == prefix + original, 'native backspace split a grapheme'
        finally:
            subprocess.run(['wl-copy'], input=saved_clipboard.stdout, check=True)


def accessibility():
    with Window('accessibility') as w:
        w.select(0, 1)
        assert tuple(w.body().getSelection(0)) == (0, 1)
        assert w.named('正文结构编辑器').getState().contains(pyatspi.STATE_FOCUSED)
        w.key(15)  # Tab leaves body for next native control.
        assert any(n.getState().contains(pyatspi.STATE_FOCUSED) and n.name != '正文结构编辑器' for n in w.nodes())
        assert any(n.getRoleName() == 'math' and '分子' in (n.name or '') for n in w.nodes()), 'math semantics missing'


def screen_reader():
    # Start only after focusing our own window; do not capture other applications.
    with Window('screen-reader') as w:
        path = OUT / 'orca.log'
        with (OUT / 'orca-process.log').open('w') as log:
            reader = subprocess.Popen(['orca', '--debug', '--debug-file', str(path)],
                                      stdout=log, stderr=log, env=dict(os.environ, PYTHONUNBUFFERED='1'))
            try:
                time.sleep(3)
                assert reader.poll() is None, 'Orca exited during startup'
                w.action('LaTeX')
                assert w.named('正文结构编辑器').queryComponent().grabFocus()
                time.sleep(1)
                w.select(0, 1)
                w.key(106)  # Right: real caret notification.
                assert w.source().queryComponent().grabFocus()
                time.sleep(1)
                w.key(29, 102)
                call('fcitx5-remote', '-c')
                w.type('READER')
                w.key(42, 105)  # Shift+Left: real selection notification.
                w.key(15)  # Tab to commit action.
                time.sleep(2)
            finally:
                reader.terminate()
                reader.wait(timeout=10)
            speech = [line for line in path.read_text().splitlines() if 'SPEECH OUTPUT:' in line]
            (OUT / 'orca-speech.txt').write_text('\n'.join(speech))
            assert any('正文结构编辑器' in line for line in speech), 'body focus not spoken'
            assert any('应用源码' in line for line in speech), 'button focus not spoken'
            assert 'text-selection-changed' in path.read_text(), 'selection notification missing'


def pointer_selection():
    with Window('pointer') as w:
        call('fcitx5-remote', '-c')
        wid = str(w.window['id'])
        call('niri', 'msg', 'action', 'move-window-to-floating', '--id', wid)
        call('niri', 'msg', 'action', 'move-floating-window', '--id', wid, '-x', '0', '-y', '0')
        time.sleep(0.6)
        original = w.text()
        end = original.index('a') + 1
        first = w.body().getCharacterExtents(0, pyatspi.WINDOW_COORDS)
        last = w.body().getCharacterExtents(end - 1, pyatspi.WINDOW_COORDS)
        (OUT / 'pointer-geometry.json').write_text(json.dumps({'first': list(first), 'last': list(last)}, indent=2))
        # AccessKit window coordinates are physical pixels. Floating origin is fixed above.
        w.focus_guard()
        fx, fy, fw, fh = first
        lx, ly, lw, lh = last
        window = next(n for n in json.loads(call('niri', 'msg', '--json', 'windows')) if n['id'] == w.window['id'])
        wx, wy = window['layout']['tile_pos_in_workspace_view']
        scale = next(iter(json.loads(call('niri', 'msg', '--json', 'outputs')).values()))['logical']['scale']
        output = next(iter(json.loads(call('niri', 'msg', '--json', 'outputs')).values()))['logical']
        # An absolute uinput pointer bypasses ydotool's acceleration-dependent relative reset.
        with UInput({ecodes.EV_KEY: [ecodes.BTN_LEFT, ecodes.BTN_RIGHT], ecodes.EV_ABS: [
            (ecodes.ABS_X, AbsInfo(0, 0, output['width'], 0, 0, 0)),
            (ecodes.ABS_Y, AbsInfo(0, 0, output['height'], 0, 0, 0))]},
            name='Scholium acceptance pointer', input_props=[ecodes.INPUT_PROP_POINTER]) as pointer:
            time.sleep(1)
            def move(x, y):
                pointer.write(ecodes.EV_ABS, ecodes.ABS_X, round(wx + x / scale))
                pointer.write(ecodes.EV_ABS, ecodes.ABS_Y, round(wy + y / scale))
                pointer.syn()
            move(fx, fy + fh // 2)
            time.sleep(0.2)
            pointer.write(ecodes.EV_KEY, ecodes.BTN_LEFT, 1)
            pointer.syn()
            time.sleep(0.2)
            try:
                for step in range(1, 11):
                    x = fx + (lx + lw - fx) * step // 10
                    y = fy + fh // 2 + (ly - fy) * step // 10
                    w.focus_guard()
                    move(x, y)
                    time.sleep(0.05)
            finally:
                pointer.write(ecodes.EV_KEY, ecodes.BTN_LEFT, 0)
                pointer.syn()
        time.sleep(0.4)
        assert w.body().getNSelections() == 1, 'mouse drag did not select text'
        assert tuple(w.body().getSelection(0)) == (0, end), list(w.body().getSelection(0))
        w.key(29, 45)
        assert w.text() == original[end:]
        w.key(29, 44)
        assert w.text() == original
        scroll_viewport(w)


def scroll_viewport(w):
    w.select(0, 0)
    w.type('SCROLL ' * 20)
    time.sleep(0.6)
    left_before = w.body().getCharacterExtents(0, pyatspi.WINDOW_COORDS)[0]
    # Typst now wraps at page width; typing no longer requires horizontal overflow.
    # Verify the compiled caret's vertical movement independently of viewport scrolling.
    first_y = w.body().getCharacterExtents(0, pyatspi.WINDOW_COORDS)[1]
    last_y = w.body().getCharacterExtents(119, pyatspi.WINDOW_COORDS)[1]
    assert last_y > first_y, 'Typst did not wrap long text'
    w.focus_guard()
    call('ydotool', 'mousemove', '--wheel', '-x', '8', '-y', '0')
    time.sleep(0.8)
    left_after = w.body().getCharacterExtents(0, pyatspi.WINDOW_COORDS)[0]
    w.focus_guard()
    call('ydotool', 'mousemove', '--wheel', '-x', '-8', '-y', '0')
    time.sleep(0.8)
    left_reverse = w.body().getCharacterExtents(0, pyatspi.WINDOW_COORDS)[0]
    (OUT / 'scroll.json').write_text(json.dumps({'before_x': left_before, 'after_x': left_after, 'reverse_x': left_reverse}, indent=2))
    assert left_after != left_before or left_reverse != left_before, 'horizontal wheel did not move viewport'


def prop(name, value):
    call('busctl', '--user', 'set-property', 'org.a11y.Bus', '/org/a11y/bus', 'org.a11y.Status', name, 'b', value)


def visual_comparison():
    with Window('visual-comparison') as w:
        w.action('Typst')
        source = w.source().queryText().getText(0, -1)
        (OUT / 'generated.typ').write_text(source)
        w.action('启用后台预览')
        wait_for(lambda: any(re.search(r'预览 revision Some\((\d+)\) / 正文 \1$',
                 n.name or '') for n in w.nodes()), 'preview unavailable', timeout=60)
        # Capture the same unmodified model revision on both sides.
        (OUT / 'comparison.typ').write_text(
            '#set page(width: auto, height: auto, margin: 12pt)\n'
            '#set text(font: "Source Han Serif CN", size: 20pt)\n' + source)


def empty_slot():
    with Window('empty-slot') as w:
        call('fcitx5-remote', '-c')
        before = w.text()
        assert '□' not in before
        w.select(len(before), len(before))
        w.key(106)  # Move from the last math leaf into the empty trailing text leaf.
        assert any('node: NodeId(4), byte: 0' in (n.name or '') for n in w.nodes()), 'empty leaf not focused'
        w.type('END')
        assert w.text() == before + 'END', 'invisible empty leaf not editable'


def continuous_typing():
    with Window('continuous-typing') as w:
        call('fcitx5-remote', '-c')
        before = w.text()
        w.select(0, 0)
        w.focus_guard()
        previous = w.path.read_text().count('[typst-editor-tiles]')
        text = 'continuousinputtest'
        started = time.monotonic()
        process = subprocess.Popen(['ydotool', 'type', '--key-delay', '40', text])
        samples = []
        captured = False
        while process.poll() is None:
            nodes = w.nodes()
            body = next(n for n in nodes if n.name == '正文结构编辑器')
            extents = body.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
            samples.append({'elapsed_ms': (time.monotonic() - started) * 1000,
                            'origin': [extents.x, extents.y],
                            'adopted': w.path.read_text().count('[typst-editor-tiles]') - previous})
            if not captured and samples[-1]['adopted'] >= 3:
                call('niri', 'msg', 'action', 'screenshot-window', '--id', str(w.window['id']),
                     '--path', str(OUT / 'continuous-typing-during.png'), '-p', 'false')
                captured = True
            time.sleep(0.02)
        assert process.returncode == 0
        w.wait_current()
        assert w.text() == text + before, 'continuous input lost or reordered text'
        assert samples and samples[-1]['adopted'] >= 3, 'no intermediate scenes during typing'
        assert len({tuple(s['origin']) for s in samples}) == 1, 'canvas moved during compilation'
        updates = [int(n) for n in re.findall(r'uploaded=(\d+)', w.path.read_text())][previous:]
        assert updates and all(n < 70 for n in updates), 'typing uploaded an entire single-page raster'
        (OUT / 'continuous-typing-samples.json').write_text(json.dumps(samples, indent=2))


def session_recovery():
    path = OUT / 'recovery.session.json'
    path.unlink(missing_ok=True)
    previous = os.environ.get('SCHOLIUM_SPIKE_SESSION')
    os.environ['SCHOLIUM_SPIKE_SESSION'] = str(path)

    def durable(w):
        wait_for(lambda: any(n.name == '会话已持久化' for n in w.nodes()) and
                 not any(n.name == '有未持久化修改' for n in w.nodes()), 'checkpoint not durable')

    try:
        with Window('session-save-crash') as w:
            call('fcitx5-remote', '-c')
            w.select(0, 0)
            w.type('RECOVER')
            body = w.text()
            w.action('保存会话')
            durable(w)
            w.action('重新打开会话')
            assert w.text() == body
            assert w.source().queryComponent().grabFocus()
            w.key(29, 30)
            w.type('\\frac{unfinished')
            draft = w.source().queryText().getText(0, -1)
            durable(w)
            assert json.loads(path.read_text())['source'] == draft
            assert w.text() == body, 'draft changed authoritative body'
            w.crash = True
        with Window('session-recovered') as w:
            assert w.text() == body, 'body lost after SIGKILL'
            assert w.source().queryText().getText(0, -1) == draft, 'invalid draft lost'
            w.action('重新打开会话')
            assert w.source().queryText().getText(0, -1) == draft
            path.write_text('external-corruption')
            w.select(0, 0)
            w.type('KEEP')
            wait_for(lambda: any('文件已被外部修改' in (n.name or '') for n in w.nodes()), 'external conflict missing')
            assert w.text() == 'KEEP' + body
            assert path.read_text() == 'external-corruption', 'external change overwritten'
        with Window('session-corrupt-open') as w:
            wait_for(lambda: any('会话错误' in (n.name or '') for n in w.nodes()), 'load error missing')
            w.select(0, 0)
            before = w.text()
            w.type('EDITABLE')
            assert w.text() == 'EDITABLE' + before
            assert path.read_text() == 'external-corruption'
    finally:
        if previous is None:
            os.environ.pop('SCHOLIUM_SPIKE_SESSION', None)
        else:
            os.environ['SCHOLIUM_SPIKE_SESSION'] = previous


def session_typst_recovery():
    path = OUT / 'typst.session.json'
    path.unlink(missing_ok=True)
    previous = os.environ.get('SCHOLIUM_SPIKE_SESSION')
    os.environ['SCHOLIUM_SPIKE_SESSION'] = str(path)
    try:
        with Window('session-typst-close') as w:
            call('fcitx5-remote', '-c')
            w.action('Typst')
            assert w.source().queryComponent().grabFocus()
            w.key(29, 30)
            w.type('unfinished [')
            draft = w.source().queryText().getText(0, -1)
            w.select(0, 0)
            w.type('STALE')
            body = w.text()
            wait_for(lambda: any(n.name == '会话已持久化' for n in w.nodes()) and
                     not any(n.name == '有未持久化修改' for n in w.nodes()), 'Typst draft not durable')
            w.graceful = True
        with Window('session-typst-stale-recovered') as w:
            assert w.text() == body
            assert w.source().queryText().getText(0, -1) == draft
            assert json.loads(path.read_text())['dialect'] == 'Typst'
            w.action('应用源码')
            assert w.text() == body, 'stale draft replaced body'
            assert w.source().queryText().getText(0, -1) == draft
    finally:
        if previous is None:
            os.environ.pop('SCHOLIUM_SPIKE_SESSION', None)
        else:
            os.environ['SCHOLIUM_SPIKE_SESSION'] = previous


def large_source_window():
    path = OUT / 'large-source.session.json'
    path.unlink(missing_ok=True)
    previous = os.environ.get('SCHOLIUM_SPIKE_SESSION')
    os.environ['SCHOLIUM_SPIKE_SESSION'] = str(path)
    try:
        with Window('large-source-seed') as w:
            w.action('保存会话')
            wait_for(lambda: path.exists(), 'seed not saved')
            w.graceful = True
        snapshot = json.loads(path.read_text())
        source = 'source 0123456789\n' * 100_000
        snapshot['source'] = source
        path.write_text(json.dumps(snapshot, ensure_ascii=False))
        start = time.monotonic()
        with Window('large-source-window') as w:
            opened = time.monotonic() - start
            call('fcitx5-remote', '-c')
            editor = w.source()
            text = editor.queryText()
            assert text.getText(0, -1) == source
            assert editor.queryComponent().grabFocus()
            wait_for(lambda: editor.getState().contains(pyatspi.STATE_FOCUSED),
                     'large source did not receive focus', timeout=30)
            w.key(29, 107)  # Ctrl+End scrolls to the last line.
            wait_for(lambda: text.caretOffset == len(source), 'end navigation failed', timeout=30)
            before = time.monotonic()
            w.type('TAIL')
            edited = time.monotonic() - before
            wait_for(lambda: text.getText(0, -1) == source + 'TAIL', 'tail edit missing', timeout=30)
            # The caret must be on screen, not merely changed in the buffer.
            bounds = w.app[0].queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
            wait_for(lambda: bounds.y <= text.getCharacterExtents(
                len(source) + 3, pyatspi.DESKTOP_COORDS)[1] < bounds.y + bounds.height,
                'tail caret is outside viewport', timeout=30)
            w.key(29, 102)
            w.type('HEAD')
            assert text.getText(0, -1) == 'HEAD' + source + 'TAIL'
            status = Path(f'/proc/{w.process.pid}/status').read_text()
            memory = re.search(r'^VmHWM:\s+(.*)$', status, re.M).group(1)
            (OUT / 'large-source-timing.json').write_text(json.dumps({
                'lines': 100_000, 'bytes': len(source), 'open_seconds': opened,
                'tail_edit_observation_seconds': edited, 'peak_rss': memory,
                'timing_includes_harness_waits': True}, indent=2))
            w.graceful = True
        assert json.loads(path.read_text())['source'] == 'HEAD' + source + 'TAIL'
    finally:
        if previous is None:
            os.environ.pop('SCHOLIUM_SPIKE_SESSION', None)
        else:
            os.environ['SCHOLIUM_SPIKE_SESSION'] = previous


def replica_collaboration():
    previous = os.environ.get('SCHOLIUM_SPIKE_REPLICAS')
    os.environ['SCHOLIUM_SPIKE_REPLICAS'] = '1'
    try:
        with Window('replica-collaboration') as w:
            call('fcitx5-remote', '-c')

            def accepted(name):
                prefix = name + ' 已接受正文：'
                return next(n.name[len(prefix):] for n in w.nodes() if (n.name or '').startswith(prefix))

            def entry(index):
                return [n for n in w.nodes() if n.getRoleName() == 'entry'][index]

            def prepend(index, text):
                assert entry(index).queryComponent().grabFocus()
                time.sleep(0.3)
                w.key(29, 102)
                w.type(text)

            original = accepted('Alice')
            prepend(0, 'LOCAL')
            w.action('应用 Alice')
            prepend(0, 'SECOND')
            w.action('应用 Alice')
            prepend(1, 'REMOTE')
            w.action('应用 Bob')
            assert accepted('Alice') != accepted('Bob'), 'delivery was not delayed'
            w.action('逆序并重复交付')
            assert accepted('Alice') == accepted('Bob')
            assert all(word in accepted('Alice') for word in ['LOCAL', 'SECOND', 'REMOTE'])
            w.action('撤销 Alice')
            w.action('交付消息')
            assert accepted('Alice') == accepted('Bob')
            assert 'SECOND' not in accepted('Bob') and 'REMOTE' in accepted('Bob')
            w.action('撤销 Alice')
            w.action('逆序并重复交付')
            assert accepted('Alice') == accepted('Bob') == 'REMOTE' + original
            prepend(1, 'OLD')
            draft = entry(1).queryText().getText(0, -1)
            w.action('请求语言切换')
            w.action('应用 Bob')
            assert accepted('Bob') == 'REMOTE' + original
            w.action('完成语言切换')
            assert entry(1).queryText().getText(0, -1) == draft
            w.action('应用 Bob')
            assert any('SourceEpochStale' in (n.name or '') for n in w.nodes())
            w.action('重新生成 Bob')
            prepend(1, 'NEW')
            w.action('应用 Bob')
            w.action('逆序并重复交付')
            assert accepted('Alice') == accepted('Bob') == 'NEWREMOTE' + original
            assert entry(1).queryComponent().grabFocus()
            time.sleep(0.3)
            w.key(29, 102)
            call('fcitx5-remote', '-s', 'rime')
            call('fcitx5-remote', '-o')
            time.sleep(0.5)
            w.type('nihao')
            wait_for(lambda: any('Bob 输入法组合中' in (n.name or '') for n in w.nodes()), 'Bob composition missing')
            w.action('请求语言切换')
            w.action('完成语言切换')
            assert any('等待消息交付及两端输入法结束' in (n.name or '') for n in w.nodes())
            assert any('活动语言 Typst · epoch 2' in (n.name or '') for n in w.nodes())
            w.key(57)
            wait_for(lambda: not any('Bob 输入法组合中' in (n.name or '') for n in w.nodes()), 'Bob composition did not finish')
            composed = entry(1).queryText().getText(0, -1)
            assert composed != accepted('Bob'), 'unsubmitted Chinese draft missing'
            call('fcitx5-remote', '-c')
            w.action('完成语言切换')
            w.action('应用 Bob')
            assert any('SourceEpochStale' in (n.name or '') for n in w.nodes())
            assert entry(1).queryText().getText(0, -1) == composed
            assert accepted('Alice') == accepted('Bob') == 'NEWREMOTE' + original
    finally:
        if previous is None:
            os.environ.pop('SCHOLIUM_SPIKE_REPLICAS', None)
        else:
            os.environ['SCHOLIUM_SPIKE_REPLICAS'] = previous


def team_ime_barrier():
    previous = os.environ.get('SCHOLIUM_SPIKE_TEAM')
    os.environ['SCHOLIUM_SPIKE_TEAM'] = '1'
    try:
        with Window('team-ime') as w:
            call('fcitx5-remote', '-c')
            assert w.source().queryComponent().grabFocus()
            time.sleep(0.3)
            w.key(29, 102)
            original = w.text()
            draft = w.source().queryText().getText(0, -1)
            call('fcitx5-remote', '-s', 'rime')
            call('fcitx5-remote', '-o')
            time.sleep(0.5)
            w.type('nihao')
            wait_for(lambda: any('输入法组合中' in (n.name or '') for n in w.nodes()), 'composition barrier missing')
            for name in ['Bob', '请求切换到 Typst', '应用源码', '丢弃草稿并从正文生成']:
                # AccessKit's AT-SPI adapter reports ENABLED for some disabled button roles.
                # Exercise the action and verify the authority/draft instead of trusting that flag.
                try:
                    w.named(name).queryAction().doAction(0)
                except NotImplementedError:
                    pass
                time.sleep(0.3)
                assert any('输入法组合中' in (n.name or '') for n in w.nodes()), f'{name} interrupted composition'
                assert any('Alice 许可 epoch 1' in (n.name or '') for n in w.nodes()), f'{name} changed actor/epoch'
                assert not any('正在切换到' in (n.name or '') for n in w.nodes()), f'{name} started language switch'
            assert w.text() == original, 'preedit published to shared body'
            w.key(1)
            wait_for(lambda: not any('输入法组合中' in (n.name or '') for n in w.nodes()), 'cancel did not release barrier')
            assert w.source().queryText().getText(0, -1) == draft
            w.type('nihao')
            w.key(57)
            wait_for(lambda: not any('输入法组合中' in (n.name or '') for n in w.nodes()), 'commit did not release barrier')
            committed = w.source().queryText().getText(0, -1)
            assert committed != draft and any('\u4e00' <= c <= '\u9fff' for c in committed[:2])
            assert w.text() == original, 'IME bypassed explicit source commit'
            call('fcitx5-remote', '-c')
            w.action('Bob')
            assert w.source().queryText().getText(0, -1) == draft, 'Alice composition leaked into Bob'
            w.action('Alice')
            assert w.source().queryText().getText(0, -1) == committed
            w.action('应用源码')
            assert w.text() != original and any('已接受 1' in (n.name or '') for n in w.nodes())
    finally:
        if previous is None:
            os.environ.pop('SCHOLIUM_SPIKE_TEAM', None)
        else:
            os.environ['SCHOLIUM_SPIKE_TEAM'] = previous


def team_language_gate():
    previous = os.environ.get('SCHOLIUM_SPIKE_TEAM')
    os.environ['SCHOLIUM_SPIKE_TEAM'] = '1'
    try:
        with Window('team-language') as w:
            call('fcitx5-remote', '-c')

            def prepend(text):
                assert w.source().queryComponent().grabFocus()
                time.sleep(0.3)
                w.key(29, 102)
                w.type(text)

            def contains(message):
                return any(message in (n.name or '') for n in w.nodes())

            original = w.text()
            for member, text in [('Alice', 'AAA'), ('Bob', 'BBB'), ('Carol', 'CCC')]:
                w.action(member)
                prepend(text)
                w.action('应用源码')
            accepted = 'CCCBBBAAA' + original
            assert w.text() == accepted, 'same-language clients failed'
            w.action('Alice')
            prepend('DRAFT')
            draft = w.source().queryText().getText(0, -1)
            w.action('模拟离线')
            w.action('应用源码')
            assert contains('NetworkUnreachable') and w.text() == accepted
            w.action('模拟离线')
            w.action('请求切换到 Typst')
            w.action('应用源码')
            assert contains('冻结') and w.text() == accepted
            w.action('保留草稿并确认切换')
            w.action('完成团队切换')
            assert contains('DrainIncomplete')
            for member in ['Bob', 'Carol']:
                w.action(member)
                w.action('保留草稿并确认切换')
            w.action('完成团队切换')
            w.action('Alice')
            assert w.source().queryText().getText(0, -1) == draft
            w.action('获取当前许可')
            w.action('应用源码')
            assert contains('SourceEpochStale') and w.text() == accepted
            w.action('丢弃草稿并从正文生成')
            prepend('WRONGLANG')
            w.action('应用源码')
            assert contains('DialectNotActive') and w.text() == accepted
            w.action('丢弃草稿并从正文生成')
            w.action('Typst')
            prepend('TYPST')
            w.action('应用源码')
            assert w.text() == 'TYPST' + accepted
            assert contains('已接受 4')
    finally:
        if previous is None:
            os.environ.pop('SCHOLIUM_SPIKE_TEAM', None)
        else:
            os.environ['SCHOLIUM_SPIKE_TEAM'] = previous


saved = {name: call('busctl', '--user', 'get-property', 'org.a11y.Bus', '/org/a11y/bus',
                    'org.a11y.Status', name).split()[-1] for name in ['IsEnabled', 'ScreenReaderEnabled']}
ime_name = call('fcitx5-remote', '-n')
ime_state = call('fcitx5-remote')
service_active = subprocess.run(['systemctl', '--user', 'is-active', '--quiet', 'ydotool']).returncode == 0
results = []
try:
    for name in saved:
        prop(name, 'true')
    call('systemctl', '--user', 'start', 'ydotool')
    for case in [ime, source_dialects, math_edit, accessibility, screen_reader, pointer_selection, unicode_clipboard, slot_selection, multipage_preview, visual_comparison, empty_slot, continuous_typing, large_document_edit, session_recovery, session_typst_recovery, large_source_window, team_language_gate, team_ime_barrier, replica_collaboration]:
        if len(sys.argv) > 2 and case.__name__ not in sys.argv[2:]:
            continue
        try:
            case()
            result = {'case': case.__name__, 'result': 'Pass'}
        except Exception as error:
            traceback.print_exc()
            result = {'case': case.__name__, 'result': 'Fail', 'reason': f'{type(error).__name__}: {error}'}
        results.append(result)
        print(json.dumps(result, ensure_ascii=False), flush=True)
        if case is multipage_preview and result['result'] == 'Pass':
            try:
                frame_cpu_measurement()
                timing = {'case': 'frame_cpu_measurement', 'result': 'Pass'}
            except Exception as error:
                timing = {'case': 'frame_cpu_measurement', 'result': 'Fail',
                          'reason': f'{type(error).__name__}: {error}'}
            results.append(timing)
            print(json.dumps(timing, ensure_ascii=False), flush=True)
finally:
    call('fcitx5-remote', '-s', ime_name or 'keyboard-us')
    if ime_state in ['1', '2']:
        call('fcitx5-remote', '-o' if ime_state == '2' else '-c')
    for name, value in saved.items():
        prop(name, value)
    if not service_active:
        call('systemctl', '--user', 'stop', 'ydotool')
    (OUT / 'results.json').write_text(json.dumps(results, ensure_ascii=False, indent=2))
sys.exit(any(result['result'] != 'Pass' for result in results))
