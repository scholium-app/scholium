#!/usr/bin/env python3
"""AT-SPI 无障碍树探针。

用途：在真实桌面上列出各应用暴露给辅助技术的对象树，判断某个候选窗口
**是否发布了可访问对象**（角色、名称、文本与选区接口）。

按 `docs/NATIVE_UI_VALIDATION.md` 第 4 节，可访问性必须做真实系统辅助功能检查，
不能用逻辑测试替代。

用法：
    scripts/a11y-probe.py [窗口标题关键字] [最大深度]
"""

import sys

import pyatspi

DEFAULT_KEYWORD = "Scholium"
DEFAULT_DEPTH = 4
MAX_CHILDREN = 40

INTERESTING_STATES = (
    "SHOWING",
    "VISIBLE",
    "FOCUSED",
    "ENABLED",
    "SELECTABLE",
    "SELECTED",
    "EDITABLE",
    "FOCUSABLE",
)


def state_names(node):
    """返回节点命中的关键状态名。"""
    names = []
    try:
        states = node.getState()
    except Exception:
        return names
    for name in INTERESTING_STATES:
        state = getattr(pyatspi.StateType, name, None)
        if state is None:
            continue
        try:
            if states.contains(state):
                names.append(name)
        except Exception:
            pass
    return names


def describe(node):
    """一行描述一个可访问对象。"""
    try:
        name = node.name
    except Exception:
        name = "<读取失败>"
    try:
        role = node.getRoleName()
    except Exception:
        role = "?"
    try:
        description = node.description
    except Exception:
        description = ""
    return "{} name={!r} desc={!r} states=[{}]".format(
        role, name, description, ",".join(state_names(node))
    )


def walk(node, depth, limit, out):
    """深度优先遍历并收集描述行。"""
    indent = "  " * depth
    out.append("{}{}".format(indent, describe(node)))
    if depth >= limit:
        return
    try:
        count = node.childCount
    except Exception:
        return
    for index in range(min(count, MAX_CHILDREN)):
        try:
            child = node.getChildAtIndex(index)
        except Exception:
            continue
        if child is not None:
            walk(child, depth + 1, limit, out)


def text_interfaces(node):
    """检查节点是否暴露文本与选区接口。"""
    found = []
    for interface in ("Text", "EditableText", "Selection", "Hypertext", "Action"):
        try:
            node.getQueryInterface()
        except Exception:
            pass
    try:
        text = node.queryText()
        found.append("Text(len={})".format(text.characterCount))
    except Exception:
        pass
    try:
        node.querySelection()
        found.append("Selection")
    except Exception:
        pass
    try:
        node.queryEditableText()
        found.append("EditableText")
    except Exception:
        pass
    return found


def a11y_status():
    """读取会话无障碍开关。

    AccessKit 只在 IsEnabled 为真时才向 AT-SPI 注册，因此这个开关是测试前提：
    关着的时候，即使框架支持无障碍，也测不到任何对象——会造成假阴性。
    """
    import subprocess

    def query(prop):
        try:
            out = subprocess.run(
                [
                    "gdbus", "call", "--session", "--dest", "org.a11y.Bus",
                    "--object-path", "/org/a11y/bus",
                    "--method", "org.freedesktop.DBus.Properties.Get",
                    "org.a11y.Status", prop,
                ],
                capture_output=True, text=True, timeout=5,
            ).stdout
            return "true" in out.lower()
        except Exception:
            return None

    return query("IsEnabled"), query("ScreenReaderEnabled")


def find_matching(desktop, keyword):
    """找到匹配关键字的对象，返回 (应用名, 节点)。

    先按**应用名**匹配（AccessKit 应用常以 crate 名注册，窗口名为空），
    再退回按窗口标题匹配。这一步曾经写错：只按标题匹配时，egui 明明注册了却被判为"没有对象"。
    """
    matches = []
    lowered = keyword.lower()
    for app_index in range(desktop.childCount):
        try:
            app = desktop.getChildAtIndex(app_index)
        except Exception:
            continue
        if app is None:
            continue
        try:
            app_name = app.name or ""
        except Exception:
            app_name = ""
        if lowered in app_name.lower():
            matches.append((app_name, app))
            continue
        for window_index in range(app.childCount):
            try:
                window = app.getChildAtIndex(window_index)
            except Exception:
                continue
            if window is None:
                continue
            try:
                window_name = window.name or ""
            except Exception:
                window_name = ""
            if lowered in window_name.lower():
                matches.append((app_name, window))
    return matches


def main():
    keyword = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_KEYWORD
    limit = int(sys.argv[2]) if len(sys.argv) > 2 else DEFAULT_DEPTH

    desktop = pyatspi.Registry.getDesktop(0)
    applications = []
    for index in range(desktop.childCount):
        try:
            app = desktop.getChildAtIndex(index)
        except Exception:
            continue
        if app is not None:
            try:
                applications.append(app.name)
            except Exception:
                applications.append("?")
    print("桌面上的应用（{} 个）：{}".format(len(applications), ", ".join(applications)))

    enabled, screen_reader = a11y_status()
    print(
        "会话无障碍开关：IsEnabled={} ScreenReaderEnabled={}".format(enabled, screen_reader)
    )
    if enabled is False:
        print()
        print("警告：会话无障碍处于关闭状态。AccessKit 类框架此时不会向 AT-SPI 注册，")
        print("      测到的'没有对象'是假阴性。先启用再测：")
        print(
            "      gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \\\n"
            "        --method org.freedesktop.DBus.Properties.Set org.a11y.Status IsEnabled '<true>'"
        )

    matches = find_matching(desktop, keyword)
    if not matches:
        print()
        print("结果：未找到匹配 {!r} 的可访问对象（已按应用名与窗口标题匹配）。".format(keyword))
        if enabled is False:
            print("含义：**在无障碍关闭的前提下**，该候选没有发布对象树；此结论不充分。")
        else:
            print("含义：该候选没有向辅助技术发布任何对象树——屏幕阅读器看不到它的内容。")
        return

    for app_name, node in matches:
        try:
            node_name = node.name
        except Exception:
            node_name = "?"
        print()
        print("找到匹配对象：应用={!r} 名称={!r}".format(app_name, node_name))
        out = []
        walk(node, 0, limit, out)
        print("可访问对象树（深度 {}，共 {} 行）：".format(limit, len(out)))
        print("\n".join(out))
        print("该对象暴露的接口：{}".format(text_interfaces(node) or "无"))


if __name__ == "__main__":
    main()
