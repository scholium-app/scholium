"""Fail-closed acceptance of fault observations, independent of process exit codes."""


def e8_verdict(row):
    scenario = row.get("scenario")
    if scenario in ("background kill", "upstream timeout"):
        failed = any(e.get("compile", {}).get("state") == "Failed" for e in row.get("layouts", []))
        injected = bool(row.get("killed")) if scenario == "background kill" else row.get("timeout_seen")
        accepted = bool(injected and failed and row.get("recovered"))
    elif scenario == "malformed host commands":
        cases = row.get("cases", [])
        accepted = bool(cases) and all(c.get("alive") and c.get("reply", {}).get("reply") == "error"
                                      for c in cases) and not row.get("written_outside_snapshot")
    else:
        accepted = False
    return {"pass": accepted, "scenario": scenario}
