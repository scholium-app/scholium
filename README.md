# Scholium /「注疏」

科学写作桌面应用。原生 UI，Typst 排版内核，结构化数学编辑。

> scholium：古典手稿页边的注疏。欧几里得《几何原本》的 scholia
> 是希腊数学史最重要的传世材料之一。

## 状态

规划阶段。尚无可运行代码。

实施方案见 [docs/PLAN.md](docs/PLAN.md)。

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
