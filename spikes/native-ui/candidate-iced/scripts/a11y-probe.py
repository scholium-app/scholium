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


def find_matching(desktop, keyword):
    """找到标题匹配关键字的窗口，并返回 (应用名, 窗口节点)。"""
    matches = []
    for app_index in range(desktop.childCount):
        try:
            app = desktop.getChildAtIndex(app_index)
        except Exception:
            continue
        if app is None:
            continue
        try:
            app_name = app.name
        except Exception:
            app_name = "?"
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
            if keyword.lower() in window_name.lower():
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

    matches = find_matching(desktop, keyword)
    if not matches:
        print()
        print("结果：未找到标题含 {!r} 的可访问窗口。".format(keyword))
        print("含义：该候选没有向辅助技术发布任何对象树——屏幕阅读器看不到它的内容。")
        return

    for app_name, window in matches:
        print()
        print("找到匹配窗口：应用={!r} 标题={!r}".format(app_name, window.name))
        out = []
        walk(window, 0, limit, out)
        print("可访问对象树（深度 {}，共 {} 行）：".format(limit, len(out)))
        print("\n".join(out))
        print("窗口自身暴露的接口：{}".format(text_interfaces(window) or "无"))


if __name__ == "__main__":
    main()
