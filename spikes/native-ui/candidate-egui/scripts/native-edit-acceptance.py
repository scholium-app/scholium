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
        self.path = OUT / f'{name}.log'
        self.log = self.path.open('w')
        self.process = subprocess.Popen(
            [str(BINARY)],
            stdout=self.log, stderr=self.log)
        self.app = None

    def __enter__(self):
        try:
            self.window = wait_for(lambda: next((w for w in json.loads(call('niri', 'msg', '--json', 'windows'))
                                   if w.get('pid') == self.process.pid), None), 'window missing')
            call('niri', 'msg', 'action', 'focus-window', '--id', str(self.window['id']))
            self.app = wait_for(lambda: next((a for a in pyatspi.Registry.getDesktop(0)
                                            if a.get_process_id() == self.process.pid), None), 'AT-SPI missing')
            wait_for(lambda: any(n.name == '正文结构编辑器' for n in self.nodes()), 'body missing')
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
    for case in [ime, source_dialects, math_edit, accessibility, pointer_selection, unicode_clipboard, slot_selection, multipage_preview, visual_comparison]:
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
