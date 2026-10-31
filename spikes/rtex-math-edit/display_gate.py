"""Host display policy: pixels and geometry advance together only on clean revisions."""

import hashlib
import json


class DisplayGate:
    def __init__(self, raster):
        self.raster = raster
        self.current_revision = {}
        self.shown = {}
        self.diagnostics = []

    def begin(self, unit, revision):
        self.current_revision[unit] = revision

    def accept(self, unit, revision, event, geometry):
        kind = event.get("event")
        clean = (kind == "ParagraphUpdate" and event.get("status") == "ok") or \
            (kind == "LayoutUpdate" and event.get("compile", {}).get("state") == "Ok")
        if not clean or revision != self.current_revision.get(unit):
            self.diagnostics.append({"unit": unit, "revision": revision, "kind": kind,
                                     "compile": event.get("compile"), "status": event.get("status")})
            return False
        raster, _ = self.raster.draw(event)
        # The raster worker is the same E4 renderer. Fingerprint its complete
        # render input plus geometry; rejected events never invoke that worker.
        digest = hashlib.sha256(json.dumps([event, geometry], sort_keys=True).encode()).hexdigest()
        self.shown[unit] = {"revision": revision, "fingerprint": digest, "geometry": geometry,
                            "raster": raster}
        return True
