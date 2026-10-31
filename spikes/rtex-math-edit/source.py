"""Restricted original-fixture parser, UTF-8 source ranges and attribute instrumentation.

This is deliberately not a general LaTeX parser. Unknown commands fail closed. Existing
groups remain groups; only single-token script arguments acquire braces. No atom is
wrapped in an extra math group (which would change binary/operator spacing).
"""

import json
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
SYMBOLS = {"alpha": "α", "sum": "∑", "left": "(", "right": ")"}
TOKEN = re.compile(r"\\[A-Za-z]+|\\\\|[^\s]", re.UNICODE)
ATTR = 65000


class Parser:
    def __init__(self, text, offset=0, next_id=1):
        self.text, self.offset = text, offset
        self.tokens = list(TOKEN.finditer(text))
        self.i, self.next_id = 0, next_id
        self.nodes = []
        self.insertions = []

    def new(self, kind, start, end, parent, char=None):
        ident = self.next_id
        self.next_id += 1
        self.nodes.append({"id": ident, "kind": kind, "start": self.byte(start),
                           "stop": self.byte(end), "parent": parent, "char": char})
        return ident

    def byte(self, pos):
        return self.offset + len(self.text[:pos].encode())

    def peek(self):
        return self.tokens[self.i].group() if self.i < len(self.tokens) else None

    def take(self):
        token = self.tokens[self.i]
        self.i += 1
        return token

    def tag(self, pos, ident):
        self.insertions.append((pos, f"\\attribute{ATTR}={ident} "))

    def group(self, kind, parent, close="}"):
        opening = self.take()
        expected = "{" if close == "}" else "["
        if opening.group() != expected:
            raise ValueError(f"expected {expected}: {self.text}")
        start = opening.end()
        ident = self.new(kind, start, start, parent)
        self.sequence(ident, close)
        closing = self.take()
        if closing.group() != close:
            raise ValueError("unclosed group")
        node = next(n for n in self.nodes if n["id"] == ident)
        node["stop"] = self.byte(closing.start())
        return ident

    def sequence(self, parent, close=None):
        last = parent
        while self.peek() is not None and (self.peek() not in close if isinstance(close, tuple) else self.peek() != close):
            value = self.peek()
            if value in ("^", "_"):
                self.take()
                kind = "sup" if value == "^" else "sub"
                if self.peek() == "{":
                    self.group(kind, last)
                else:
                    token = self.tokens[self.i]
                    slot = self.new(kind, token.start(), token.end(), last)
                    # ^2 and ^{2} are equivalent; assignments must be inside the argument.
                    self.insertions.append((token.start(), "{"))
                    self.atom(slot)
                    self.insertions.append((token.end(), "}"))
                continue
            if value == "{":
                self.group("row", parent)
            elif value in ("&", r"\\"):
                self.take()
            else:
                last = self.atom(parent)

    def atom(self, parent):
        token = self.take()
        value, start, end = token.group(), token.start(), token.end()
        command = value[1:] if value.startswith("\\") else None
        kinds = {"frac": "frac", "sqrt": "rad", "text": "text", "mathbb": "font",
                 "left": "fence", "right": "fence", "begin": "matrix"}
        if command and command not in kinds and command not in SYMBOLS:
            raise ValueError(f"unsupported macro {value}")
        kind = kinds.get(command, "atom")
        ident = self.new(kind, start, end, parent, SYMBOLS.get(command, value) if kind == "atom" else None)
        self.tag(start, ident)
        if command == "frac":
            self.group("num", ident)
            self.group("den", ident)
        elif command == "sqrt":
            if self.peek() == "[":
                self.group("degree", ident, "]")
            self.group("radicand", ident)
        elif command in ("text", "mathbb"):
            self.group("text-row" if command == "text" else "font-row", ident)
        elif command in ("left", "right"):
            delimiter = self.take()
            node = next(n for n in self.nodes if n["id"] == ident)
            node.update(stop=self.byte(delimiter.end()), char=delimiter.group())
        elif command == "begin":
            self.matrix(ident)
        node = next(n for n in self.nodes if n["id"] == ident)
        if command in ("frac", "sqrt", "text", "mathbb", "begin"):
            node["stop"] = self.byte(self.tokens[self.i - 1].end())
        return ident

    def matrix(self, parent):
        opening = self.text.index("}", self.tokens[self.i].start())
        if self.text[self.tokens[self.i].start():opening + 1] != "{pmatrix}":
            raise ValueError("only pmatrix supported")
        while self.tokens[self.i].end() <= opening + 1:
            self.i += 1
        while self.peek() != r"\end":
            start = self.tokens[self.i].start()
            cell = self.new("cell", start, start, parent)
            self.sequence(cell, ("&", r"\\", r"\end"))
            node = next(n for n in self.nodes if n["id"] == cell)
            node["stop"] = self.byte(self.tokens[self.i].start())
            if self.peek() != r"\end":
                self.take()
        self.take()
        for _ in range(len("pmatrix") + 2):
            self.take()

    def parse(self):
        root = self.new("formula", 0, len(self.text), None)
        self.sequence(root)
        out = self.text
        for pos, order, value in sorted(((p, i, v) for i, (p, v) in enumerate(self.insertions)), reverse=True):
            out = out[:pos] + value + out[pos:]
        return out, self.nodes


def fixtures(font):
    preamble = (r"\documentclass{ctexart}" + "\n" + r"\usepackage{amsmath,amssymb}" + "\n")
    if font == "unicode":
        preamble += r"\usepackage{unicode-math}\setmathfont{Latin Modern Math}" + "\n"
    source = preamble + r"\begin{document}" + "\n"
    tagged_body, nodes, cases = [], [], []
    wrappers = {"inline": ("$", "$"), "display": (r"\[", r"\]"),
                "equation": (r"\begin{equation}", r"\end{equation}"),
                "align": (r"\begin{align}", r"\end{align}")}
    for case in json.loads((HERE / "fixtures/cases.json").read_text()):
        before, after = wrappers[case["mode"]]
        prefix = f"Case {case['name']}: " + before
        offset = len((source + prefix).encode())
        parser = Parser(case["tex"], offset, len(nodes) + 1)
        tagged, parsed = parser.parse()
        cases.append({**case, "start": offset, "stop": offset + len(case["tex"].encode()),
                      "root": parsed[0]["id"], "nodes": [n["id"] for n in parsed]})
        nodes.extend(parsed)
        source += prefix + case["tex"] + after + "\n\n"
        tagged_body.append(prefix + tagged + f"\\attribute{ATTR}=-2147483647 " + after + "\n\n")
    source += r"\end{document}" + "\n"
    tagged = preamble + r"\begin{document}" + "\n" + "".join(tagged_body) + r"\end{document}" + "\n"
    return source, tagged, nodes, cases
