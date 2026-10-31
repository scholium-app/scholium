"""Tests for measurement validity, independent of the acceptance verdict."""

import unittest

from alignment import expected_atoms
from runtime import math_offset
from source import Parser, fixtures


class MeasurementContracts(unittest.TestCase):
    def test_keystrokes_target_nuclei_in_all_modes_and_variants(self):
        for font in ("cm", "unicode"):
            source, tagged, _, cases = fixtures(font)
            for text in (source, tagged):
                for case in cases[:4]:
                    offset = math_offset(text, case)
                    self.assertEqual(text.encode()[offset:offset + 1],
                                     b"a" if case["name"] == "fraction" else b"x")

    def test_nested_slots_and_chinese_ranges_use_source_bytes(self):
        _, nodes = Parser(r"\frac{中}{x_i^2}", offset=10).parse()
        chinese = next(n for n in nodes if n.get("char") == "中")
        self.assertEqual((chinese["start"], chinese["stop"]), (16, 19))
        self.assertEqual([n["kind"] for n in nodes if n["kind"] in ("num", "den", "sub", "sup")],
                         ["num", "den", "sub", "sup"])

    def test_fallback_includes_generated_radical_and_matrix_fences(self):
        _, nodes = Parser(r"\sqrt{x}+\begin{pmatrix}a\end{pmatrix}").parse()
        self.assertEqual([n["char"] for n in expected_atoms(nodes)], ["√", "x", "+", "(", "a", ")"])

    def test_unknown_macros_fail_closed(self):
        with self.assertRaises(ValueError):
            Parser(r"\arbitrary{x}").parse()


if __name__ == "__main__":
    unittest.main()
