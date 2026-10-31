#!/usr/bin/env python3
"""T22b fresh evidence: identity veto, host provenance, packed empty slots, display policy."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time

from caret import audit, Caret
from evaluate import glyph_rows, verify, structure_report
from geometry2 import materialize, merge_raw
from revision import UnitRevision, HostDocument
from runtime import PATCHED, BASE, converged, start, exchange, sandbox, write_json, Raster, percentiles
from source import fixtures


def preamble(font):
    return fixtures(font)[0].split(r'\begin{document}')[0]


def document(font):
    cases = json.loads((Path(__file__).parent / 'fixtures/cases.json').read_text())
    cases += [{'mode': 'inline', 'tex': r'\sqrt{}+\sqrt[]{}'}, {'mode': 'inline', 'tex': ''}]
    return HostDocument(preamble(font), [UnitRevision.build(c['tex'], c['mode'],
                        prefix=f"Case {i}: ", id_start=i * 1000000 + 1) for i, c in enumerate(cases)])


def project(args, name, source):
    path = args.work / 'projects' / name
    path.mkdir(parents=True)
    (path / 'main.tex').write_text(source)
    return path


def compile_pages(args, path, name):
    result, work = sandbox(args, PATCHED, path, name, ['/usr/bin/lualatex', '-interaction=nonstopmode',
                        '-halt-on-error', '-output-directory=/work', '/project/main.tex'])
    if result['exit'] == 0:
        subprocess.run(['pdftoppm', '-r', '300', '-gray', str(work / 'main.pdf'), str(work / 'page')],
                       check=True, capture_output=True)
        result['pages'] = [hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(work.glob('page-*.pgm'))]
    return result


def identity(args, font):
    doc = document(font)
    result, pages = {}, {}
    for tagged in (False, True):
        name = f'r2-{font}-' + ('A' if tagged else 'plain')
        path = project(args, name, doc.source(tagged))
        result[name] = {'verify': verify(args, PATCHED if tagged else BASE, path, name + '-verify')}
        result[name]['full'] = compile_pages(args, path, name + '-full')
        live = project(args, name + '-live-input', doc.source(tagged))
        session, build = start(args, PATCHED if tagged else BASE, live, name + '-live')
        try:
            pages[tagged], _, _ = converged(session, build)
        finally:
            session.close()
    result['identity'] = {'glyphs_rules_equal': glyph_rows(pages[False]) == glyph_rows(pages[True]),
        'items': len(glyph_rows(pages[True])),
        'all_pages_pixels_equal': result[f'r2-{font}-plain']['full'].get('pages') ==
                                 result[f'r2-{font}-A']['full'].get('pages')}
    raw = merge_raw(pages[True])
    write_json(args.evidence / f'{font}-raw-pages.json', pages[True])
    structures, checks = [], []
    for unit in doc.units:
        geometry = materialize(unit, raw)
        report, rows = audit(unit.nodes, geometry)
        for slot in geometry['slots']:
            actual = Caret(unit.nodes, geometry).click(slot['anchor'])
            rows.append({'kind': 'slot-click', 'expected': next(n['start'] for n in unit.nodes
                         if n['id'] == slot['source_id']), 'actual': actual})
        structures.append({'expr': unit.expr, **structure_report(unit.nodes, geometry),
                           'slots': geometry['slots']})
        checks.extend(rows)
    result['structure'] = structures
    result['caret'] = {'checks': len(checks), 'passed': sum(c['expected'] == c['actual'] for c in checks),
                       'failures': [c for c in checks if c['expected'] != c['actual']]}
    write_json(args.evidence / f'{font}-identity-caret.json', checks)
    write_json(args.evidence / f'{font}-identity.json', result)
    return result


# 34 actual unit replacements per font, with deletion as well as insertion.
STATES = [r'\frac{a}{b}', r'\frac{}{b}', r'\frac{}{}', 'x', 'x^{}', 'x^{2}',
    'x^{2}_{}', 'x^{2}_{i}', 'x_{i}', 'x', r'\sqrt{}', r'\sqrt{x}', r'\sqrt[3]{x}',
    r'\sqrt[]{x}', r'\sqrt{x+\frac{a}{b}}', 'x', r'\left(x\right)',
    r'\left(\frac{a}{b}\right)', r'\left(\right)', 'x',
    r'\begin{pmatrix}a\end{pmatrix}', r'\begin{pmatrix}a&\end{pmatrix}',
    r'\begin{pmatrix}a&b\end{pmatrix}', r'\begin{pmatrix}a&b\\\end{pmatrix}',
    r'\begin{pmatrix}a&b\\&d\end{pmatrix}', r'\begin{pmatrix}a&b\\c&d\end{pmatrix}',
    r'\begin{pmatrix}a&b\end{pmatrix}', 'x', r'\frac{x^{}}{\sqrt{}}',
    r'\frac{x^{2}_{i}}{\sqrt{y}}', r'\frac{\frac{a}{b}}{c}',
    r'\frac{a}{b}', '', 'x']


def snapshot(session, build):
    session.send({'cmd': 'export_pdf', 'out': '/work/host-check.pdf'})
    event = session.wait(lambda e: e.get('event') == 'PdfExported', 40)
    return (build / 'build/export-src/main.tex').read_bytes(), event


def changed(session, command, build):
    record, update = exchange(session, **command, timeout=30)
    if not update or update['event'] != 'ParagraphUpdate':
        # Structural unit edits may use the background path; don't pretend fast.
        pages, layout, _ = converged(session, build)
        return record, layout, merge_raw(pages)
    return record, update, update.get('dl', {}).get('math_edit') or {}


def structural(args, font):
    doc = HostDocument(preamble(font), [UnitRevision.build('x'), UnitRevision.build('w', id_start=1000001)])
    path = project(args, f'r2-{font}-structural-input', doc.source(True))
    session, build = start(args, PATCHED, path, f'r2-{font}-structural')
    rows = []
    raster = Raster(dpi=300)
    try:
        converged(session, build)
        for i, expr in enumerate(STATES):
            old = doc.units[0]
            before = time.monotonic()
            command, unit = doc.edit(0, old.expr_start, old.expr_start + len(old.expr.encode()), expr)
            host_ms = (time.monotonic() - before) * 1000
            record, event, raw = changed(session, command, build)
            geometry = materialize(unit, raw)
            _, checks = audit(unit.nodes, geometry)
            for slot in geometry['slots']:
                checks.append({'kind': 'slot-click', 'expected': next(n['start'] for n in unit.nodes
                    if n['id'] == slot['source_id']), 'actual': Caret(unit.nodes, geometry).click(slot['anchor'])})
            for empty in unit.nodes:
                if empty['start'] == empty['stop']:
                    checks.append({'kind': 'empty-slot-available', 'expected': empty['start'],
                        'actual': empty['start'] if str(empty['id']) in geometry['nodes'] else None})
            # Independent literal counterexample, not the old source registry.
            counterexample = None
            if i == 0:
                b = next((g for g in geometry['glyphs'] if next(n for n in unit.nodes
                     if n['id'] == g['source_id']).get('char') == 'b'), None)
                actual = None if not b else Caret(unit.nodes, geometry).click(
                    (b['bbox'][0] + (b['bbox'][2] - b['bbox'][0]) * .75, sum(b['bbox'][1::2]) / 2))
                counterexample = {'expected': unit.expr_start + len(r'\frac{a}{b') , 'actual': actual}
                checks.append({'kind': 'round1-b', **counterexample})
            drawn, done = raster.draw(event)
            snapshot_bytes, export = snapshot(session, build)
            expected = doc.preamble + '\\begin{document}\nUnit: $ ' + expr + '$\n\nUnit: $ w$\n\n\\end{document}\n'
            saved = doc.source().encode()
            # Save only the authoritative untagged text, at every revision.
            (args.evidence / f'{font}-revision-{i+2:02d}.tex').write_bytes(saved)
            row = {'step': i + 1, 'revision': unit.revision, 'expr': expr, 'edit': record,
                'host_retag_ms': round(host_ms, 3),
                'to_raster_ms': round(host_ms + record.get('update_ms', 0) +
                    (done - session.received_at) * 1000, 3),
                'raster': drawn, 'checks': checks, 'geometry': geometry,
                'plain_export_exact': saved == expected.encode(),
                'engine_buffer_exact': snapshot_bytes == doc.source(True).encode(),
                'offset_map_roundtrip': all(unit.offsets.plain(unit.offsets.tagged(p)) == p
                                            for p in range(len(unit.plain.encode()) + 1)),
                'no_foreign_ids': all(g['source_id'] in {n['id'] for n in unit.nodes} for g in geometry['glyphs']),
                'counterexample': counterexample}
            rows.append(row)
            write_json(args.evidence / f'{font}-structural.json', rows)
            print(font, 'edit', i+1, record.get('routed'), len(checks),
                  sum(c['actual'] != c['expected'] for c in checks), flush=True)
    finally:
        raster.close()
        session.close()
    return {'edits': len(rows), 'assertions': sum(len(r['checks']) for r in rows),
        'failures': [{'step': r['step'], **c} for r in rows for c in r['checks'] if c['expected'] != c['actual']],
        'exports': sum(r['plain_export_exact'] and r['engine_buffer_exact'] for r in rows),
        'maps': sum(r['offset_map_roundtrip'] for r in rows),
        'host_retag_ms': percentiles([r['host_retag_ms'] for r in rows])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run-id', required=True)
    parser.add_argument('--evidence', type=Path, required=True)
    parser.add_argument('--phase', choices=['identity', 'structural', 'gate', 'latency'], required=True)
    args = parser.parse_args()
    args.evidence = args.evidence.resolve()
    args.evidence.mkdir(parents=True, exist_ok=True)
    args.work = Path(__file__).parent.resolve() / '.work' / args.run_id
    args.work.mkdir(parents=True, exist_ok=False)
    args.cache = args.work.parent / 'font-cache'
    results = {}
    if args.phase in ('gate', 'latency'):
        from round2_live import gate_trials, latency_spot
    action = {'identity': identity, 'structural': structural}
    for font in ('cm', 'unicode'):
        if args.phase == 'gate':
            results[font] = gate_trials(args, font)
        elif args.phase == 'latency':
            results[font] = latency_spot(args, font)
        else:
            results[font] = action[args.phase](args, font)
        write_json(args.evidence / f'{args.phase}-summary.json', results)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
