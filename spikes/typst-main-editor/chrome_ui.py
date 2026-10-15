"""Click window chrome by command id, never by coordinate.

The app's `SCHOLIUM_EDITOR_AUDIT` output carries a `controls` map: command id (or an
`ui.*` chrome id) to every visible control that runs it, with its rectangle in window
pixels. Checks import this module, build a `Chrome` over their own state/xdotool helpers
and call `click("format.bold")`, `menu("ui.menu.format", "format.clear")` and so on, so a
layout or language change never invalidates a check.

Surfaces: menubar, menu (open menu rows), toolbar, overflow (open "»" list), focus, tab,
status. Ids of chrome that is not a command start with `ui.` (`ui.menu.file`,
`ui.toolbar.overflow`, `ui.menu.format.paragraph_style`, `ui.tab`).
"""
import re
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
CATALOGS = REPO / "crates" / "scholium-app" / "i18n"
TAGS = {"zh-Hans": "zh-Hans", "zh": "zh-Hans", "en": "en"}


def catalog(locale):
    """`{ "command.format.bold.title": "粗体", ... }` parsed from the shipped .ftl file."""
    entries, message = {}, None
    for line in (CATALOGS / f"{TAGS.get(locale, 'zh-Hans')}.ftl").read_text(encoding="utf-8").splitlines():
        if line.startswith(("#", " ")) and not line.lstrip().startswith("."):
            continue
        attribute = re.match(r"\s+\.(\w+)\s*=\s*(.*)", line)
        top = re.match(r"([\w-]+)\s*=\s*(.*)", line)
        if attribute and message:
            entries[f"{message}.{attribute.group(1)}"] = attribute.group(2)
        elif top:
            message = top.group(1)
            if top.group(2):
                entries[message] = top.group(2)
    return entries


def title(locale, command_id):
    """The catalog title of a command in `locale`."""
    return catalog(locale)["command-" + command_id.replace(".", "-") + ".title"]


class Chrome:
    def __init__(self, state, xdo, window, timeout=30):
        """`state()` returns the last AUDIT dict, `xdo(*args)` runs xdotool, `window` is the
        X window id."""
        self.state, self.xdo, self.window, self.timeout = state, xdo, window, timeout

    def entries(self, command_id, surface=None):
        current = self.state() or {}
        found = (current.get("controls") or {}).get(command_id, [])
        return [e for e in found if surface is None or e["surface"] == surface]

    def wait(self, command_id, surface=None, enabled=None):
        """The control once its rectangle has stopped moving (a popup lays out twice)."""
        end = time.monotonic() + self.timeout
        previous = None
        while time.monotonic() < end:
            found = [e for e in self.entries(command_id, surface) if enabled is None or e["enabled"] == enabled]
            if found:
                if previous is not None and found[0]["rect"] == previous["rect"]:
                    return found[0]
                previous = found[0]
                time.sleep(0.2)
            else:
                previous = None
                time.sleep(0.05)
        raise RuntimeError(f"no control {command_id} on {surface}")

    def point(self, command_id, surface=None):
        x0, y0, x1, y1 = self.wait(command_id, surface)["rect"]
        return round((x0 + x1) / 2), round((y0 + y1) / 2)

    def hover(self, command_id, surface=None):
        x, y = self.point(command_id, surface)
        self.xdo("mousemove", "--window", self.window, x, y)

    def click(self, command_id, surface=None):
        self.hover(command_id, surface)
        time.sleep(0.05)
        self.xdo("click", 1)

    def gone(self, command_id, surface, timeout=10):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            if not self.entries(command_id, surface):
                return True
            time.sleep(0.05)
        return False

    def menu(self, menu_id, *path):
        """Open `menu_id` (e.g. `ui.menu.format`) and click the entries of `path`; every
        entry but the last is a submenu row that opens on hover."""
        self.click(menu_id, "menubar")
        for step in path[:-1]:
            self.hover(step, "menu")
        self.click(path[-1], "menu")

    def overflow(self, command_id):
        self.click("ui.toolbar.overflow", "toolbar")
        self.click(command_id, "overflow")
