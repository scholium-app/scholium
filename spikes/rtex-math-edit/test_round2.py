"""Regression contracts for the three round-1 driver defects."""

import unittest

from revision import HostDocument, UnitRevision
from display_gate import DisplayGate


class RevisionContracts(unittest.TestCase):
    def test_structure_replacement_tags_new_atoms_and_keeps_source_map_host_side(self):
        old = UnitRevision.build("x")
        doc = HostDocument("", [old])
        command, new = doc.edit(0, old.expr_start, old.expr_start + 1, r"\frac{a}{b}")
        self.assertEqual(command["start"], doc.unit_start(0, True))
        self.assertEqual(command["end"] - command["start"], len(old.tagged.encode()))
        atoms = [n for n in new.nodes if n.get("char") in ("a", "b")]
        self.assertEqual(len(atoms), 2)
        self.assertNotEqual(atoms[0]["id"], atoms[1]["id"])
        self.assertNotIn("directlua", doc.source(True))
        self.assertNotIn("attribute", doc.source())

    def test_tagged_untagged_offsets_round_trip_every_utf8_boundary(self):
        unit = UnitRevision.build(r"x^2+\frac{中}{文}")
        for offset in range(len(unit.plain.encode()) + 1):
            self.assertEqual(unit.offsets.plain(unit.offsets.tagged(offset)), offset)

    def test_revisions_do_not_collide_between_units(self):
        units = [UnitRevision.build("x", id_start=1), UnitRevision.build("y", id_start=1000001)]
        doc = HostDocument("", units)
        for index in range(2):
            unit = doc.units[index]
            doc.edit(index, unit.expr_start, unit.expr_start + 1, "z")
        self.assertTrue(set(n["id"] for n in doc.units[0].nodes).isdisjoint(
            n["id"] for n in doc.units[1].nodes))
        self.assertEqual(len(doc.history), 4)

    def test_dirty_and_stale_layouts_keep_exact_raster_geometry_object(self):
        class Renderer:
            calls = 0
            def draw(self, event):
                self.calls += 1
                return {"pixels": 42}, 0
        renderer = Renderer()
        gate = DisplayGate(renderer)
        good = {"event": "LayoutUpdate", "compile": {"state": "Ok"}}
        gate.begin(0, 1)
        self.assertTrue(gate.accept(0, 1, good, {"revision": 1}))
        shown = gate.shown[0]
        gate.begin(0, 2)
        self.assertFalse(gate.accept(0, 1, good, {}))
        dirty = {"event": "LayoutUpdate", "compile": {"state": "CompiledWithErrors"}}
        self.assertFalse(gate.accept(0, 2, dirty, {}))
        self.assertIs(gate.shown[0], shown)
        self.assertEqual(renderer.calls, 1)
        self.assertEqual(len(gate.diagnostics), 2)

    def test_single_script_closes_before_the_next_atom_tag(self):
        unit = UnitRevision.build('y^2+z')
        plus = next(n for n in unit.nodes if n.get('char') == '+')
        self.assertIn('}\\attribute65000=' + str(plus['id']) + ' +', unit.tagged)

    def test_nested_structure_tag_follows_its_field_tag(self):
        unit = UnitRevision.build(r'\frac{x^{}}{\sqrt{}}')
        rad = next(n for n in unit.nodes if n['kind'] == 'rad')
        den = next(n for n in unit.nodes if n['kind'] == 'den')
        self.assertIn(f"\\attribute65000={den['id']} \\attribute65000={rad['id']} \\sqrt", unit.tagged)

    def test_empty_fields_have_explicit_tags_and_formulas_reset_both_attributes(self):
        unit = UnitRevision.build(r"\frac{}{}+x^{}")
        for node in unit.nodes:
            if node["kind"] in ("num", "den", "sup"):
                self.assertIn(f"\\attribute65000={node['id']} ", unit.tagged)
        self.assertIn("\\attribute65000=-2147483647", unit.tagged)
        self.assertIn("\\attribute65001=-2147483647", unit.tagged)


if __name__ == "__main__":
    unittest.main()
