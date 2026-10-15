#!/usr/bin/env python3
"""Real-window AT-SPI acceptance of the editor body; run through run.sh, never on a desktop bus."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import chrome_ui  # noqa: E402

locale = os.environ.get("SCHOLIUM_LOCALE", "zh-Hans")
catalog = chrome_ui.catalog(locale)
binary = Path(sys.argv[1]).resolve()
output = Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
if any(output.iterdir()):
    raise SystemExit("output directory must be empty")
env = dict(os.environ, SCHOLIUM_SESSION_FILE=str(output / "doc.sqlite"), SCHOLIUM_EDITOR_AUDIT="1",
           WINIT_UNIX_BACKEND="x11", WINIT_X11_SCALE_FACTOR="1")
env.pop("WAYLAND_DISPLAY", None)
log_path = output / "app.log"
results = {}


def audits():
    return [json.loads(l[6:]) for l in log_path.read_text().split("\n")[:-1] if l.startswith("AUDIT ")]


def wait(predicate, what, timeout=30):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        value = predicate()
        if value:
            return value
        time.sleep(0.05)
    raise RuntimeError(f"timeout: {what}")


def find(node, test, depth=0):
    if node is None or depth > 12:
        return None
    if test(node):
        return node
    for i in range(node.get_child_count()):
        hit = find(node.get_child_at_index(i), test, depth + 1)
        if hit is not None:
            return hit
    return None


def app_root():
    desktop = Atspi.get_desktop(0)
    for i in range(desktop.get_child_count()):
        child = desktop.get_child_at_index(i)
        if child is not None and child.get_name() == "scholium-app":
            return child
    return None


def body():
    root = app_root()
    return root and find(root, lambda n: n.get_name() == "正文" and "Text" in n.get_interfaces())


def text_of(node):
    return Atspi.Text.get_text(node, 0, -1)


def xdo(*args):
    subprocess.run(["xdotool", *map(str, args)], check=True, env=env)


def paste(value):
    subprocess.run(["xclip", "-selection", "clipboard"], input=value.encode(), env=env, check=True)
    xdo("key", "--clearmodifiers", "ctrl+v")


def record(name, ok, **detail):
    results[name] = {"pass": bool(ok), **detail}
    print(("PASS " if ok else "FAIL ") + name, flush=True)
    if not ok:
        raise AssertionError(f"{name}: {detail}")


def states(node):
    state = node.get_state_set()
    return {name: state.contains(getattr(Atspi.StateType, name))
            for name in ("ENABLED", "SENSITIVE", "FOCUSED", "EDITABLE", "MULTI_LINE")}


with log_path.open("w") as log:
    app = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
try:
    wait(lambda: audits() and audits()[-1]["current"], "first scene")
    # AccessKit activates on the ScreenReaderEnabled change signal, not on its initial value.
    subprocess.run(["busctl", "--user", "set-property", "org.a11y.Bus", "/org/a11y/bus",
                    "org.a11y.Status", "ScreenReaderEnabled", "b", "true"], check=True, env=env)
    window = subprocess.check_output(["xdotool", "search", "--pid", str(app.pid)], env=env, text=True).split()[0]
    xdo("windowfocus", window)
    node = wait(body, "body Text object")
    record("01-body-text-object", node.get_role_name() in ("entry", "text") and text_of(node) == "",
           role=node.get_role_name(), states=states(node), interfaces=list(node.get_interfaces()))

    xdo("type", "--delay", "15", "Hello ")
    paste("中文 👩‍🔬")
    wait(lambda: text_of(body()) == "Hello 中文 👩‍🔬", "typed text in AT-SPI")
    node = body()
    caret = Atspi.Text.get_caret_offset(node)
    record("02-typed-text-and-caret", caret == len("Hello 中文 👩‍🔬"), text=text_of(node), caret=caret)

    paste("\nsecond line\n\nfourth")
    wait(lambda: text_of(body()) == "Hello 中文 👩‍🔬\nsecond line\n\nfourth", "paragraphs in AT-SPI")
    record("03-paragraph-breaks", True, text=text_of(body()))

    node = body()
    Atspi.Text.set_selection(node, 0, 0, 5)
    state = wait(lambda: audits()[-1]["selection"] is not None and audits()[-1], "AT-SPI selection applied")
    sel = Atspi.Text.get_selection(node, 0)
    record("04-assistive-selection", (sel.start_offset, sel.end_offset) == (0, 5),
           selection=state["selection"], read_back=[sel.start_offset, sel.end_offset])
    xdo("type", "Q")
    wait(lambda: text_of(body()).startswith("Q 中文"), "selection replaced")
    record("05-selection-replaced-once", True, text=text_of(body()))

    # Continuous typing: every sample must still expose the Text interface.
    # The same accessible object must stay valid; re-finding it would hide a re-created node.
    failures, samples, held = 0, 0, body()
    burst = subprocess.Popen(["xdotool", "type", "--delay", "40", " continuous typing burst"], env=env)
    while burst.poll() is None:
        samples += 1
        try:
            text_of(held)
        except Exception:  # noqa: BLE001 - any loss of the interface is the failure
            failures += 1
        time.sleep(0.01)
    pending = sum(not a["current"] for a in audits())
    record("06-text-survives-pending-layout", failures == 0 and samples > 10,
           samples=samples, failures=failures, pending_frames=pending)

    wait(lambda: audits()[-1]["current"], "settled")
    node = body()
    ext = Atspi.Text.get_character_extents(node, 0, Atspi.CoordType.WINDOW)
    record("07-character-extents", 0 <= ext.x < 1280 and 0 <= ext.y < 900 and ext.width > 0 and ext.height > 0,
           extents=[ext.x, ext.y, ext.width, ext.height])

    xdo("key", "--clearmodifiers", "ctrl+End")
    xdo("type", " $")
    xdo("type", "12")
    xdo("key", "--clearmodifiers", "ctrl+slash")
    root = app_root()
    math = wait(lambda: find(app_root(), lambda n: n.get_role_name() == "math"), "math object")
    status = wait(lambda: find(app_root(), lambda n: (n.get_name() or "").startswith("分数分母")), "slot status")
    record("08-math-object-and-slot-status",
           math.get_name() == "公式：分数（分子：12；分母：空）" and status.get_name() == "分数分母：空",
           math=math.get_name(), status=status.get_name(), status_role=status.get_role_name())

    # Every control exposes its catalog title (in the run's language) as its AT-SPI name.
    def named(command_id):
        name = chrome_ui.title(locale, command_id)
        return find(app_root(), lambda n: n.get_name() == name and n.get_role_name() != "text")

    # at-spi2-core renamed "push button" to "button" (2.62 locally, older on CI runners).
    button_roles = ("button", "push button")
    toggle_roles = ("check box", "toggle button")
    strike, bold, undo = named("format.strike"), named("format.bold"), named("edit.undo")
    redo = named("edit.redo")  # nothing to redo yet: disabled with a reason
    record("09-disabled-controls-are-not-enabled",
           strike is not None and not states(strike)["ENABLED"] and not states(strike)["SENSITIVE"]
           and redo is not None and not states(redo)["ENABLED"]
           and bold is not None and states(bold)["ENABLED"],
           strike=states(strike) if strike else None, redo=states(redo) if redo else None,
           bold=states(bold) if bold else None)
    record("10-only-checkable-commands-are-toggles",
           bold.get_role_name() in toggle_roles and undo.get_role_name() in button_roles
           and strike.get_role_name() in button_roles,
           bold=bold.get_role_name(), undo=undo.get_role_name(), strike=strike.get_role_name())
    wanted = {
        "menubar": [catalog[f"menu-{m}"] for m in
                    ("file", "edit", "insert", "format", "document", "view", "tools", "help")],
        "toolbar": [chrome_ui.title(locale, c) for c in
                    ("format.bold", "format.italic", "edit.undo", "insert.formula", "tools.search")],
        "focus": [chrome_ui.title(locale, c) for c in
                  ("view.mode.visual", "view.mode.source", "insert.exit_formula")],
        "tab": [chrome_ui.title(locale, c) for c in ("file.new", "file.close")],
        "status": [chrome_ui.title(locale, c) for c in ("view.zoom.in", "view.zoom.out", "view.zoom.fit_width")],
    }
    missing = {surface: [n for n in names
                         if find(app_root(), lambda node, n=n: node.get_name() == n) is None]
               for surface, names in wanted.items()}
    record("11-every-control-is-named-by-its-catalog-title",
           not any(missing.values()), locale=locale, missing=missing)
finally:
    (output / "summary.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
    if app.poll() is None:
        app.terminate()
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired:
            # Never let a stuck app mask the assertion that already failed.
            app.kill()
