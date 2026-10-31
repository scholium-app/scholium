# 2026-10-30 — Session core phase 2

## 范围

从 `origin/main` 的 phase-1 session core 提取抽象输入边界。app 将提交文字和不需要 Scene 几何的 egui 按键翻译为
`scholium_session::typst::Input`；核心保存页面 pt 视口、栅格规格和指针连续状态，并通过 `Outcome` 返回语义 caret、
selection、overlay 与新布局意图。IME preedit、屏幕坐标换算、绘制和 AccessKit 仍属于 app。

## 验证

- `scholium-session` 新增抽象文字/按键/指针回放与失败回滚测试；失败输入恢复文档、光标、历史、保存基线和指针状态。
- 共享场景位于 `spikes/typst-main-editor/scenarios/`；session 测试逐文件解析并回放所有场景，窗口入口为 `parity-check.py`。
- 既有 app 测试断言和窗口阈值未修改；迁移文字输入后，水平跨正文/数学叶测试发现并修复叶尾 UTF-8 边界，原断言保持不变。

## 行数

`crates/scholium-app/src/typst_editor.rs`：425 → 425 行。`crates/scholium-session/src/**/*.rs`：2299 → 2830 行；新增
事件类型与测试位于独立模块，所有文件低于 600 行。临时基线记录在未提交的 `target/t17-lines-before.json`。

工作区 fmt、clippy、test、rustdoc、规模、文档链接、layer、UI 字串、cargo deny、双语言窗口检查（30/30）和四项
stock Typst 像素对照均通过；既有测试断言、窗口阈值与 workflow 未修改。
