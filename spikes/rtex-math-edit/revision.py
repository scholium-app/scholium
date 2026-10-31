"""Host-only revision provenance. All public offsets refer to untagged UTF-8 bytes."""

from dataclasses import dataclass
import re

from source import ATTR, Parser

UNSET = -2147483647
FORMULA_ATTR = 65001
WRAPPERS = {"inline": ("$ ", "$"), "display": (r"\[", r"\]"),
            "equation": (r"\begin{equation}", r"\end{equation}"),
            "align": (r"\begin{align}", r"\end{align}")}


def attribute(ident, which=ATTR):
    return f"\\attribute{which}={ident} "


@dataclass
class OffsetMap:
    tagged_to_plain: list
    plain_to_tagged: dict

    def tagged(self, plain_byte):
        return self.plain_to_tagged[plain_byte]

    def plain(self, tagged_byte):
        return self.tagged_to_plain[tagged_byte]


def instrument(text, edits):
    """Insert-only instrumentation, retaining an exact byte-boundary map both ways."""
    data, out, reverse, forward = text.encode(), bytearray(), [0], {}
    insertions = {}
    for pos, value in edits:
        insertions.setdefault(pos, []).append(value.encode())
    for pos in range(len(data) + 1):
        for value in insertions.get(pos, []):
            out.extend(value)
            reverse.extend([pos] * len(value))
        forward[pos] = len(out)
        if pos < len(data):
            out.append(data[pos])
            reverse.append(pos + 1)
    return out.decode(), OffsetMap(reverse, forward)


def parse_expression(expr, offset, first_id):
    parser = Parser(expr, offset, first_id)
    _, nodes = parser.parse()
    # Tag field contents even when empty. LuaTeX propagates the owning noad's tag
    # to packed math fields; the host pairs their role with its revision tree.
    for node in nodes:
        if node["kind"] in ("num", "den", "sub", "sup", "radicand", "degree", "cell"):
            at = node["start"] - offset
            parser.insertions.append((len(expr.encode()[:at].decode()), attribute(node["id"])))
    edits = [(len(expr[:pos].encode()), value) for pos, value in parser.insertions]
    # Field tags precede the first atom tag at a shared boundary.
    fields = {n["id"] for n in nodes if n["kind"] in
              ("num", "den", "sub", "sup", "radicand", "degree", "cell")}

    def priority(pair):
        pos, value = pair
        if value.startswith("\\attribute"):
            ident = int(re.search(r"=(\d+)", value)[1])
            return pos, 0 if ident in fields else 1
        return pos, -1 if value == "{" else -2

    edits.sort(key=priority)
    return nodes, edits


@dataclass
class UnitRevision:
    revision: int
    plain: str
    tagged: str
    nodes: list
    offsets: OffsetMap
    mode: str
    expr: str
    expr_start: int

    @classmethod
    def build(cls, expr, mode="inline", revision=1, prefix="Unit: ", id_start=1):
        before, after = WRAPPERS[mode]
        lead = prefix + before
        expr_start = len(lead.encode())
        nodes, edits = parse_expression(expr, expr_start, id_start)
        plain = lead + expr + after + "\n\n"
        inserts = [(len(prefix.encode()), attribute(nodes[0]["id"], FORMULA_ATTR)),
                   (expr_start, attribute(nodes[0]["id"]))]
        inserts.extend((expr_start + pos, value) for pos, value in edits)
        inserts.extend([(expr_start + len(expr.encode()), attribute(UNSET)),
                        (len((lead + expr + after).encode()), attribute(UNSET, FORMULA_ATTR))])
        tagged, offsets = instrument(plain, inserts)
        return cls(revision, plain, tagged, nodes, offsets, mode, expr, expr_start)

    def replace(self, start, end, text, id_start=None):
        data = self.plain.encode()
        updated = data[:start] + text.encode() + data[end:]
        end_expr = len(updated) - len((WRAPPERS[self.mode][1] + "\n\n").encode())
        expr = updated[self.expr_start:end_expr].decode()
        prefix_size = self.expr_start - len(WRAPPERS[self.mode][0].encode())
        return UnitRevision.build(expr, self.mode, self.revision + 1,
                                  updated[:prefix_size].decode(), id_start if id_start is not None else max(n["id"] for n in self.nodes) + 1)


class HostDocument:
    def __init__(self, preamble, units):
        self.preamble, self.units = preamble, units
        self.history = {(i, u.revision): u for i, u in enumerate(units)}
        self.next_id = max(n["id"] for u in units for n in u.nodes) + 1

    def source(self, tagged=False):
        return self.preamble + r"\begin{document}" + "\n" + \
            "".join(u.tagged if tagged else u.plain for u in self.units) + r"\end{document}" + "\n"

    def unit_start(self, index, tagged=False):
        return len((self.preamble + r"\begin{document}" + "\n").encode()) + \
            sum(len((u.tagged if tagged else u.plain).encode()) for u in self.units[:index])

    def edit(self, index, start, end, text):
        old = self.units[index]
        new = old.replace(start, end, text, self.next_id)
        self.next_id = max(n["id"] for n in new.nodes) + 1
        position = self.unit_start(index, tagged=True)
        self.units[index] = new
        self.history[index, new.revision] = new
        return {"start": position, "end": position + len(old.tagged.encode()), "text": new.tagged}, new
