"""Tiny structural caret over parsed UTF-8 boundaries and current engine geometry (sp)."""


class Caret:
    def __init__(self, nodes, geometry):
        self.nodes = {n["id"]: n for n in nodes}
        self.geometry = geometry
        self.leaves = [n for n in nodes if n.get("char")]
        self.order = sorted({n[k] for n in self.leaves for k in ("start", "stop")} |
                            {n["start"] for n in nodes if n["start"] == n["stop"]})

    def click(self, point):
        hits = []
        for glyph in self.geometry.get("glyphs", []):
            left, top, right, bottom = glyph["bbox"]
            if left <= point[0] <= right and top <= point[1] <= bottom:
                hits.append(glyph)
        if not hits:
            return None
        glyph = min(hits, key=lambda g: (g["bbox"][2] - g["bbox"][0]) *
                    (g["bbox"][3] - g["bbox"][1]))
        atom = self.nodes[glyph["source_id"]]
        midpoint = (glyph["bbox"][0] + glyph["bbox"][2]) / 2
        return atom["start"] if point[0] < midpoint else atom["stop"]

    def move(self, offset, direction):
        candidates = [p for p in self.order if (p > offset if direction == "right" else p < offset)]
        return (min(candidates) if direction == "right" else max(candidates)) if candidates else offset

    def vertical(self, ident, direction):
        node = self.nodes[ident]
        while node["parent"]:
            if node["kind"] in ("num", "den", "sub", "sup", "degree", "radicand"):
                break
            node = self.nodes[node["parent"]]
        pairs = {("num", "down"): "den", ("den", "up"): "num",
                 ("sub", "up"): "sup", ("sup", "down"): "sub",
                 ("degree", "down"): "radicand", ("radicand", "up"): "degree"}
        target = pairs.get((node["kind"], direction))
        sibling = next((n for n in self.nodes.values() if n["parent"] == node["parent"] and
                        n["kind"] == target), None)
        if not sibling:
            return None
        box = self.geometry.get("nodes", {}).get(str(sibling["id"]), {}).get("bbox")
        return sibling["start"] if box else None


def audit(nodes, geometry):
    caret = Caret(nodes, geometry)
    checks = []
    for glyph in geometry.get("glyphs", []):
        n = caret.nodes[glyph["source_id"]]
        left, top, right, bottom = glyph["bbox"]
        if not n.get("char"):
            continue
        for ratio, expected in ((0.25, n["start"]), (0.75, n["stop"])):
            actual = caret.click((left + (right - left) * ratio, (top + bottom) / 2))
            checks.append({"kind": "click", "id": n["id"], "expected": expected, "actual": actual})
    # Source order is structural order, independent of hlist order (sup often paints first).
    for a, b in zip(caret.order, caret.order[1:]):
        checks.extend([{"kind": "right", "expected": b, "actual": caret.move(a, "right")},
                       {"kind": "left", "expected": a, "actual": caret.move(b, "left")}])
    for kind, direction, target in (("num", "down", "den"), ("den", "up", "num"),
                                    ("sub", "up", "sup"), ("sup", "down", "sub")):
        for n in nodes:
            sibling = next((s for s in nodes if s["kind"] == target and s["parent"] == n["parent"]), None)
            if n["kind"] == kind and sibling:
                checks.append({"kind": direction, "id": n["id"], "expected": sibling["start"],
                               "actual": caret.vertical(n["id"], direction)})
    return {"checks": len(checks), "passed": sum(c["expected"] == c["actual"] for c in checks),
            "failures": [c for c in checks if c["expected"] != c["actual"]]}, checks
