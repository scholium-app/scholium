"""Attach raw engine tags/box roles to the host's revision tree, in scaled points."""

MIN_WIDTH = 2 * 65536
MIN_HEIGHT = 6 * 65536


def union(a, b):
    return [min(a[0], b[0]), min(a[1], b[1]), max(a[2], b[2]), max(a[3], b[3])]


def materialize(unit, raw):
    nodes = {n["id"]: n for n in unit.nodes}
    result = {"nodes": {}, "glyphs": [], "slots": [], "revision": unit.revision}

    def attach(ident, box, origin):
        while ident in nodes:
            node = nodes[ident]
            key = str(ident)
            previous = result["nodes"].get(key)
            result["nodes"][key] = {**node, "bbox": union(previous["bbox"], box) if previous else list(box),
                                    "geometry_origin": origin}
            ident = node["parent"]

    for glyph in raw.get("glyphs", []):
        if glyph["source_id"] in nodes:
            result["glyphs"].append(glyph)
            attach(glyph["source_id"], glyph["bbox"], "glyph")
    for box in raw.get("boxes", []):
        owner = nodes.get(box["owner"])
        if not owner:
            continue
        targets = [n for n in nodes.values() if n["parent"] == owner["id"] and n["kind"] == box["role"]]
        # A fraction noad inherits its outer frac owner. A script box inherits
        # the nucleus tag. Neither is a byte-range registry in the engine.
        for target in targets:
            attach(target["id"], box["bbox"], "packed-field-box")
    for boundary in raw.get("boundaries", []):
        if not boundary["on"]:
            continue
        ident = boundary.get("owner")
        if ident in nodes and nodes[ident]["kind"] == "cell":
            attach(ident, boundary["bbox"], "cell-math-boundary")
        # amsmath's optional root index is an inner math pair rather than a
        # radical degree field. Its zero-size Math-on box is an actual anchor.
        if ident in nodes and nodes[ident]["kind"] == "rad" and boundary["bbox"][1] == boundary["bbox"][3]:
            for degree in nodes.values():
                if degree["parent"] == ident and degree["kind"] == "degree" and degree["start"] == degree["stop"]:
                    attach(degree["id"], boundary["bbox"], "degree-math-boundary")
        formula = boundary.get("formula")
        if formula in nodes:
            attach(formula, boundary["bbox"], "formula-math-boundary")
    for node in nodes.values():
        if node["start"] != node["stop"]:
            continue
        value = result["nodes"].get(str(node["id"]))
        if not value:
            continue
        left, top, right, bottom = value["bbox"]
        # A host-only outline/caret target, never a TeX layout node or PDF mark.
        x, y = (left + right) / 2, (top + bottom) / 2
        hit = [x - MIN_WIDTH / 2, min(top, y - MIN_HEIGHT / 2),
               x + MIN_WIDTH / 2, max(bottom, y + MIN_HEIGHT / 2)]
        result["slots"].append({"source_id": node["id"], "bbox": hit, "anchor": [x, y],
                                "origin": value["geometry_origin"]})
    return result


def merge_raw(pages):
    result = {"glyphs": [], "boxes": [], "boundaries": []}
    for page in pages:
        raw = page["dl"].get("math_edit") or {}
        for key in result:
            result[key].extend(raw.get(key, []))
    return result
