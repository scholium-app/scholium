"""Fallback B: TeX encoding-table decoding and source/glyph sequence alignment.

No engine patch. Physical boxes bound a formula; sequence alignment is intentionally
conservative about missing symbols and does not pretend a fraction's paint order is
source order. Truth labels are used only for scoring, never fed to the aligner.
"""

import difflib
from functools import lru_cache
import re
import subprocess
import unicodedata

NAMES = {"alpha": "α", "summationdisplay": "∑", "summationtext": "∑", "radical": "√",
         "parenleft": "(", "parenright": ")", "plus": "+", "equal": "="}


def expected_atoms(nodes):
    """Source atoms plus glyphs TeX synthesizes for radicals and matrix fences."""
    result = []
    for node in nodes:
        if node.get("char"):
            result.append(node)
        elif node["kind"] == "rad":
            result.append({**node, "char": "√"})
        elif node["kind"] == "matrix":
            result.extend(({**node, "char": "("},
                           {**node, "start": node["stop"], "char": ")"}))
    return sorted(result, key=lambda n: (n["start"], n["stop"], n["id"]))


def encodings():
    out = {}
    for prefix, filename in (("cmmi", "texmital.enc"), ("cmsy", "texmsym.enc"), ("cmex", "texmext.enc")):
        path = subprocess.check_output(["kpsewhich", filename], text=True).strip()
        text = re.sub(r"%[^\n]*", "", open(path).read()).split("[", 1)[1].split("]", 1)[0]
        out[prefix] = re.findall(r"/([^\s/]+)", text)
    return out


@lru_cache(maxsize=16)
def opentype_symbols(filename):
    from fontTools.ttLib import TTFont
    font = TTFont(filename)
    cmap = font.getBestCmap()
    names = {name: unicodedata.normalize("NFKC", chr(code)) for code, name in cmap.items()}
    if "MATH" in font:
        variants = font["MATH"].table.MathVariants
        for direction in ("Vert", "Horiz"):
            coverage = getattr(variants, direction + "GlyphCoverage")
            constructions = getattr(variants, direction + "GlyphConstruction")
            if not coverage:
                continue
            for base, construction in zip(coverage.glyphs, constructions):
                if base not in names:
                    continue
                for record in construction.MathGlyphVariantRecord:
                    names[record.VariantGlyph] = names[base]
    result = {index: names[name] for index, name in enumerate(font.getGlyphOrder()) if name in names}
    font.close()
    return result


def decode(code, font, tables, index=None):
    name = font.get("name", "")
    for prefix, table in tables.items():
        if name.startswith(prefix):
            label = table[code] if 0 <= code < len(table) else ""
            if len(label) == 1:
                return label
            for stem, value in NAMES.items():
                if label.startswith(stem):
                    return value
            return "�"
    if index is not None and font.get("filename", "").endswith(".otf") and code >= 0xF0000:
        return opentype_symbols(font["filename"]).get(index, "�")
    if 0 <= code <= 0x10FFFF:
        return unicodedata.normalize("NFKC", chr(code))
    return "�"


def align(glyphs, atoms, fonts, tables):
    text = [decode(g["char"], fonts[str(g["font"])], tables, g.get("index")) for g in glyphs]
    expected = [n["char"] for n in atoms]
    mapping = {}
    for block in difflib.SequenceMatcher(a=text, b=expected, autojunk=False).get_matching_blocks():
        for offset in range(block.size):
            mapping[block.a + offset] = atoms[block.b + offset]["id"]
    return mapping, text


def score(nodes, cases, geometry, fonts, baseline_pages):
    tables = encodings()
    by_id = {n["id"]: n for n in nodes}
    all_glyphs = geometry.get("glyphs", [])
    baseline = {}
    baseline_fonts = {}
    for page in baseline_pages:
        dl = page["dl"]
        baseline_fonts.update(dl["fonts"])
        for line in dl["lines"]:
            for g in line["items"]:
                if g[0] == "g":
                    key = (dl["fonts"][str(g[1])]["name"], g[2], g[4], g[5], g[6])
                    baseline[key] = {"font": g[1], "char": g[2], "index": g[3]}
    b_correct, b_mapped, a_correct, details = 0, 0, 0, []
    for case in cases:
        # Formula crop is deliberately supplied by A for benchmark pairing. B receives
        # only the crop's glyphs with ownership stripped; automatic crop discovery is untested.
        glyphs = [g for g in all_glyphs if g["source_id"] in case["nodes"]]
        atoms = expected_atoms([n for n in nodes if n["id"] in case["nodes"]])
        blind = []
        for g in glyphs:
            b = g["bbox"]
            # Top/bottom are font-specific; pair the unchanged outline by x and advance.
            matches = [(abs(k[3] - (b[1] + b[3]) / 2), v) for k, v in baseline.items()
                       if k[0] == fonts[str(g["font"])]["name"] and k[1] == g["char"] and
                       k[2] == b[0] and k[4] == b[2] - b[0]]
            matches.sort(key=lambda pair: pair[0])
            assert matches and (len(matches) == 1 or matches[0][0] < matches[1][0]), (case["name"], g)
            blind.append(matches[0][1])
        mapping, decoded = align(blind, atoms, baseline_fonts, tables)
        rows = []
        for i, g in enumerate(glyphs):
            truth = by_id[g["source_id"]]
            # Generated radicals/fences have a command owner; compare to that command.
            allowed = truth.get("char")
            good_a = decoded[i] == allowed or (truth["kind"] == "rad" and decoded[i] == "√") or \
                (truth["kind"] == "matrix" and decoded[i] in ("(", ")"))
            a_correct += good_a
            correct = mapping.get(i) == g["source_id"]
            b_correct += correct
            b_mapped += i in mapping
            rows.append({"truth_id": g["source_id"], "decoded": decoded[i], "A_label_valid": good_a,
                         "B_id": mapping.get(i), "B_exact": correct})
        details.append({"case": case["name"], "glyphs": len(glyphs), "rows": rows})
    total = len(all_glyphs)
    return {"total": total, "A_owned": total, "A_label_valid": a_correct,
            "B_mapped": b_mapped, "B_exact": b_correct, "details": details}
