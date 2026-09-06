# Scholium /「注疏」

科学写作桌面应用。原生 UI，Typst 排版内核，结构化数学编辑。

> scholium：古典手稿页边的注疏。欧几里得《几何原本》的 scholia
> 是希腊数学史最重要的传世材料之一。

## 状态

P1 骨架已进入可运行验收阶段。当前桌面程序支持 Typst 原生排版、点击定位光标、
键盘与中文 IME 输入、撤销/重做；底层 `EditOp`、SourceMap 和 MockAgent 链路已有测试。

实施方案见 [docs/PLAN.md](docs/PLAN.md)。
P1 逐项执行情况见 [docs/P1_STATUS.md](docs/P1_STATUS.md)。

## 运行

需要 Rust nightly 和 Vulkan/Metal/DX12 图形驱动：

```bash
cargo run -p scholium-shell
```

Linux 优先查找 Noto CJK，macOS/Windows 会使用系统 CJK 字体。缺少可用中文字体时
启动日志会明确警告，而不是静默显示 `.notdef` 方框。

当前编辑快捷键：方向键/Home/End 移动，Backspace/Delete 删除，
`Ctrl/Cmd+Z` 撤销，`Ctrl/Cmd+Y` 或 `Ctrl/Cmd+Shift+Z` 重做。

## 设计要点

- **Typst 作为排版库，不是导出器。** 直接消费 `typst::compile` 产出的 `Frame`，
  靠每个 `Glyph` 携带的 span 反查到文档 AST 节点，用 vello 自绘。
  单引擎，编辑态即最终排版。
- **原生 UI。** winit + wgpu + vello + parley，无 Web 运行时。
- **结构化数学编辑。** 光标住在 AST 里，支持进入分母、矩阵单元格导航、
  Tab 变体循环等 Mogan 风格输入手感。
- **AI 接口先行。** 所有编辑经由统一的 `EditOp` 通道，AI 只能提交 `Proposal`，
  由宿主决定是否应用。接口在 P1 定型，provider 实现放到 1.0 之后。

## 许可证

未定。参见 PLAN.md §9——不移植任何 TeXmacs/Mogan 代码以保留选择权。
