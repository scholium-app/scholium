# 窗口检查：只聚焦可见窗口，a11y 等待公式名稳定

## 问题

PR #108 的 CI 在两个与改动无关的窗口检查上失败：

- `native`（中文）：`xdotool windowfocus` 报 `BadMatch (X_SetInputFocus)`。17 个检查用 `xdotool search --pid` 取第一个窗口，
  可能拿到尚未映射的辅助窗口，聚焦不可见窗口即失败。
- `a11y`（英文）：`08-math-object-and-slot-status` 读到的公式名为空。检查在 math 角色节点出现的第一帧就取名，
  此时名称还没有跟上刚键入的分子。

## 改动

- 所有检查的窗口查找改为 `xdotool search --sync --onlyvisible --pid`：等待直到出现可见窗口。
- a11y 第 8 步等待“角色为 math 且名称等于预期”的节点，断言本身不变。

没有放宽任何断言或阈值。

## 验证

本机全部窗口检查中英双语 64/64 通过。
