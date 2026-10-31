"""Real sandbox faults, display gate, input retention, and M5 spot samples."""
import hashlib
import json
import subprocess
import time
from display_gate import DisplayGate
from geometry2 import merge_raw
from runtime import BASE, PATCHED, start, converged, exchange, write_json, Raster, percentiles
from source import fixtures


class BitmapRaster(Raster):
    """Persist immutable display bitmaps in addition to warm-worker measurements."""
    def __init__(self, directory):
        super().__init__(dpi=300)
        self.directory = directory

    def draw(self, event):
        reply, done = super().draw(event)
        folder = self.directory / str(self.seq)
        folder.mkdir(parents=True)
        value = event
        if event["event"] == "ParagraphUpdate":
            value = dict(event["dl"])
            value.update(page=1, page_width=value["width"] + 65536,
                         page_height=value["height"] + value["depth"] + 65536)
        path = folder / "render.json"
        write_json(path, value)
        subprocess.run([str(__import__("measure").RASTER_BIN), "pages", str(path), str(folder),
                        "--dpi", "300"], check=True, capture_output=True)
        reply["bitmaps"] = [{"path": str(p.relative_to(self.directory.parent)),
            "sha256": hashlib.sha256(p.read_bytes()).hexdigest(), "bytes": p.stat().st_size}
            for p in sorted(folder.glob("page-*.pgm"))]
        if not reply["bitmaps"]:
            raise RuntimeError("clean event produced no retained bitmap")
        return reply, done


def collect(session, seconds):
    events = []
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        try:
            events.append(session.wait(lambda _: True, until - time.monotonic()))
        except TimeoutError:
            break
    return events


def gate_trials(args, font):
    from round2 import project, snapshot
    from runtime import math_offset
    results = []
    # Same eight trials as round 1: two fonts x two backends x two fault kinds.
    for variant, clone in (('plain', BASE), ('A', PATCHED)):
        source, tagged, _, cases = fixtures(font)
        text = tagged if variant == 'A' else source
        path = project(args, f'{font}-{variant}-gate-input', text)
        session, build = start(args, clone, path, f'{font}-{variant}-gate')
        raster = BitmapRaster(args.evidence / f"{font}-{variant}-bitmaps")
        gate = DisplayGate(raster)
        revision = 1
        try:
            pages, layout, _ = converged(session, build)
            gate.begin(0, revision)
            assert gate.accept(0, revision, layout, merge_raw(pages))
            offset = math_offset(text, cases[0])
            for name, prefix in (('unbalanced-brace', '{'), ('undefined-macro', r'\scholiumUndefined ')):
                before = gate.shown[0]
                screens, messages = [], []

                def consume(event):
                    if event.get('event') in ('ParagraphUpdate', 'LayoutUpdate'):
                        rev = event.get('versions', {}).get('source_revision')
                        accepted = gate.accept(0, rev, event, event.get('dl', {}).get('math_edit') or {})
                        screens.append({'event': event['event'], 'source_revision': rev,
                            'compile': event.get('compile'), 'status': event.get('status'),
                            'accepted': accepted, 'shown_revision': gate.shown[0]['revision'],
                            'shown_fingerprint': gate.shown[0]['fingerprint']})
                    if event.get('event') == 'Diagnostics':
                        messages.extend(event['items'])

                revision += 1
                gate.begin(0, revision)
                bad, update = exchange(session, offset, offset, prefix)
                if update: consume(update)
                for event in collect(session, 1.5): consume(event)
                revision += 1
                gate.begin(0, revision)
                typed, update = exchange(session, offset + len(prefix), offset + len(prefix), 'q')
                if update: consume(update)
                for event in collect(session, 1.5): consume(event)
                retained = gate.shown[0] is before
                stale_rejected = not gate.accept(0, 1, layout, merge_raw(pages))
                revision += 1
                gate.begin(0, revision)
                repair, update = exchange(session, offset, offset + len(prefix), '')
                if update: consume(update)
                expected = text.encode()[:offset] + b'q' + text.encode()[offset:]
                actual, exported = snapshot(session, build)
                for event in collect(session, .1): consume(event)
                recovered = gate.shown[0]['revision'] == revision
                results.append({'variant': variant, 'fault': name, 'bad': bad, 'typed': typed, 'repair': repair,
                    'last_good_object_retained_while_invalid': retained, 'stale_rejected': stale_rejected,
                    'recovered_revision_shown': recovered, 'q_export_exact': actual == expected,
                    'diagnostics': messages, 'screens': screens,
                    'old_raster_reply': before['raster'], 'old_fingerprint': before['fingerprint']})
                revision += 1
                gate.begin(0, revision)
                _, update = exchange(session, offset, offset + 1, '')
                if update: consume(update)
                restored, _ = snapshot(session, build)
                results[-1]['restore_export_exact'] = restored == text.encode()
                write_json(args.evidence / f'{font}-gate-trials.json', results)
        finally:
            raster.close()
            session.close()
    return {'trials': len(results), 'retained': sum(r['last_good_object_retained_while_invalid'] for r in results),
            'recovered': sum(r['recovered_revision_shown'] for r in results),
            'q_exports': sum(r['q_export_exact'] for r in results),
            'stale_rejected': sum(r['stale_rejected'] for r in results),
            'diagnostics_trials': sum(bool(r['diagnostics']) for r in results)}


def latency_spot(args, font):
    from round2 import document, project, changed
    doc = document(font)
    path = project(args, font + '-spot-input', doc.source(True))
    session, build = start(args, PATCHED, path, font + '-spot')
    raster = Raster(dpi=300)
    rows = []
    try:
        converged(session, build)
        for index in range(4):
            original = doc.units[index].expr
            for i in range(22):
                unit = doc.units[index]
                expr = original + ('+x' if i % 2 == 0 else '')
                sent = time.monotonic()
                command, new = doc.edit(index, unit.expr_start, unit.expr_start + len(unit.expr.encode()), expr)
                host_ms = (time.monotonic() - sent) * 1000
                record, event, _ = changed(session, command, build)
                reply, done = raster.draw(event)
                if i >= 2:
                    rows.append({'font': font, 'mode': new.mode, 'i': i-2, 'edit': record,
                        'retag_ms': host_ms, 'total_to_raster_ms': (done-sent)*1000, 'raster':reply})
        write_json(args.evidence / f'{font}-spot.json', rows)
    finally:
        raster.close()
        session.close()
    return {mode: {'host':percentiles([r['retag_ms'] for r in rows if r['mode']==mode]),
                   'to_raster':percentiles([r['total_to_raster_ms'] for r in rows if r['mode']==mode]),
                   'fast_ok':sum(r['edit'].get('routed')=='fast' and r['edit'].get('status')=='ok'
                                 for r in rows if r['mode']==mode)}
            for mode in ('inline','display','equation','align')}
