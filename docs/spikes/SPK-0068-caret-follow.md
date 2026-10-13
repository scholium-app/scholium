# SPK-0068：视图跟随光标（键入看不见光标）

- 日期：2026-10-13；结论：**Pass**（无头测试与实机检查均通过；滚轮/滚动条/点击不被撤销）。
- 对应：[P1 排版工作区规划](../plan/P1_UI.md)的页面编辑；不改变阶段 0 判定。
- 基线：main `a9df8b1`；分支 `ation_ciger/fix/caret-follow`。

## 复现与根因

Xvfb、scale 1.5：键入 “Top paragraph.”，再 45 次（Return + “Body line number N with some words.”）。页面增长、
光标在第 46 行，滚动区仍停在顶部，用户在不可见区域输入。更早的 main 同样如此：缺少功能，不是回归。

## 改动

- `typst_editor/follow.rs`：`Follow`（待跟随标记与可选的文档端点）、`Input`（事件分类）、`reveal`（纯函数，
  光标矩形加上下左右各 `FOLLOW_MARGIN_LINES` = 2 行的余量；Ctrl+Home/End 另并入页面顶部含外边距/底部）、
  `apply_follow`（用 `scroll_to_rect_animation(.., None, none)` 最小幅度滚动，无动画）。
- 触发：`event()` 里文字、粘贴、剪切、IME 提交与按键事件使光标位置变化，或 Ctrl+Home/End；`intents()` 里撤销/重做
  与使光标移动的功能区命令。指针按下与滚轮事件丢弃待跟随；点击不滚动；滚轮/滚动条本身不设置任何标记。
- 只在 Scene 为当前版本时执行；否则保持待跟随，等新 Scene 采纳后用新 caret 几何滚动，绝不对准旧位置。
  滚动改变的可见范围经既有路径（[0067](SPK-0067-sharp-raster.md) 的 `track_raster`/`set_view`）请求新区域条带。
- AUDIT 增加 `follow`：caret 与可见范围（pt）、余量行数、是否待跟随。

## 证据

| 项 | 结果 |
| --- | --- |
| 无头测试（`follow_tests.rs`，先失败后通过：停用 `apply_follow` 时 5 项失败） | 键入越过视口底部后光标可见且留 2 行余量；滚轮上滚后空闲 5 帧不被拉回、再键入一个字符即回到光标；Ctrl+Home 顶部可见、Ctrl+End 到达页面底部且光标可见；在页面顶部边缘连续 ArrowUp 每步都保持余量；编辑后 Scene 未采纳时不滚动（待跟随保持），采纳后滚动并清除标记；点击不滚动；`reveal` 仅对端点键并入页面边缘 |
| 实机（`caret-follow-check.py`，Xvfb 1280x900，`WINIT_X11_SCALE_FACTOR=1.5`） | 5/5：键入 46 行后 AUDIT caret 在可见范围内且留余量；25 次滚轮上滚后空闲 1.5 s 可见范围不变；再键入一个字符光标回到视口；Ctrl+Home 露出页面顶部；Ctrl+End 回到末尾光标 |

测试注意：egui 的滚轮是按帧缓释的平滑滚动，且页面在顶端钳位时多余的滚轮量不会立即丢弃；测试用小步滚轮并等可见范围稳定后再断言。
