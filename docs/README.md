# Scholium 设计文档

这里是正式实现的设计依据（**规范，现在时**）。计划见 [`plan/`](plan/PLAN.md)（将来时），开发日志见 [`log/`](log/README.md)（过去时）。

## 阅读顺序

先读 [项目总览](plan/PLAN.md) 了解"做什么"和"从哪里开始"，再按下列顺序深入。

1. [产品定义](PRODUCT.md)：两种工作形态、用户工作流和首发边界。
2. [编辑体验](EXPERIENCE.md)：视觉编辑、模式、焦点、数学输入和 Source Studio。
3. [总体架构](ARCHITECTURE.md)：进程、分层、依赖和关键数据流。
4. [数据模型](DATA_MODEL.md)：项目、资源、语义图、位置、诊断和标识符。
5. [排版与转换](RENDERING_CONVERSION.md)：Typst 快速预览、LaTeX 最终构建和双向转换。
   [混合源码与团队编辑](MIXED_SOURCE_EDITING.md)：团队语言互斥、完整混用范围、文件操作与两种目标输出。
6. [协议设计](PROTOCOLS.md)：UI/core 命令事件、同步帧、版本与错误。
7. [历史与协作](HISTORY_COLLABORATION.md)：CRDT、动作、撤销、分支、同步。
8. [格式系统](FORMATS.md)：LaTeX、Typst、Markdown、BibTeX、导入导出和复制。
9. [安全模型](SECURITY.md)：不可信项目、构建隔离、服务端权限。
10. [测试策略](TESTING.md)：属性测试、夹具、收敛、恢复和兼容性。
11. [路线图](plan/ROADMAP.md)：阶段、交付物、出口条件和依赖关系。
    [P1 排版工作区规划](plan/P1_UI.md)：桌面布局实施基准、两种模式参考图、允许调整的细节与评审检查。
12. [模块索引](modules/README.md)：每个 crate、应用和服务的内部设计。
13. [ADR 索引](adr/README.md)：经过验证的技术选择与替代方案。
    [原生依赖登记](NATIVE_DEPENDENCIES.md)：C/C++/Zig 与需编译本地代码的依赖（ABI / 所有权 / 线程 / 销毁顺序）。
14. [Liii STEM 体验调研](research/LIIISTEM_EXPERIENCE.md)：参考交互、取舍与映射。

## 文档状态

- 开发规范与硬性约束见仓库根 [AGENT.md](../AGENT.md)；文档纪律以它为准。
- [`archive/`](archive/README.md) 保存废弃原型的背景、实验结果和阶段记录。
- 历史记录不构成正式架构约束；只有本索引列出的文档定义新实现。
- 尚未验证的技术选择必须标记为“待验证”，不能写成既成事实；验证报告见 [`spikes/`](spikes/README.md)。
- **目录即体裁**（检索前先判体裁，不要把不同体裁当同一依据）：

  | 目录 | 体裁 | 时态 | 能否作为依据 |
  |---|---|---|---|
  | 本目录顶层、[`modules/`](modules/README.md) | 规范 | 现在时：系统应该怎样 | 能（`modules/` 缺其约定的七节时不能） |
  | [`plan/`](plan/PLAN.md) | 计划 | 将来时：要做什么、什么算通过 | 只定义判据，不能当现状 |
  | [`log/`](log/README.md) | 开发日志 | 过去时：做到哪了 | **不能**，只作追溯 |
  | [`spikes/`](spikes/README.md) | 验证证据 | 过去时：可复现结论 | 只作追溯；当前判定见 [报告 0012](spikes/SPK-0012-exit-criteria.md) |
  | [`adr/`](adr/README.md) | 裁决 | 已定 | 能 |
  | `research/` | 外部资料 | — | 只作参考 |
  | [`archive/`](archive/README.md) | 弃用原型 | 已弃 | **不能**，不得作为实现规范引用 |
- **状态单一来源**：其他文件里出现的进度、结论与出口判定一律只是副本，不构成当前判定；
  与[报告 0012](spikes/SPK-0012-exit-criteria.md) 冲突时以 0012 为准。

## 当前基本假设

- 默认体验是即时排版的结构编辑；源码模式是完整、可写的第一等工作台。
- 原生结构项目以语义文档图为真；外部源码项目以 lossless CST/source 为真。
- Typst 提供快速交互预览，LaTeX 可作为最终发布与验证后端。
- 桌面 UI、核心与服务端优先 Rust，必要时使用 C/C++/Zig；采用原生窗口/文本/绘制，不采用 npm 或 WebView。
- UI 按 [原生 UI 验证计划](plan/NATIVE_UI_VALIDATION.md) 的顺序验证后**已选定 egui / eframe 0.36.2**（[ADR 0006](adr/ADR-0006-native-ui-framework.md)）：Iced 缺可访问性、GPUI 缺文本输入控件、EUI-NEO 无无障碍不投入、Slint 被许可证政策阻断。
- 标准源文件可脱离 Scholium 使用；协作历史是增强层，不是文件可读性的前提。

- 同一共享项目分支在团队范围内只允许一种 LaTeX/Typst 源码语言写入，编译器不受此限制。
- 混合项目保留两种原文；标准目标包与双工具链重建包分别验证可移植性，不能把混合源码当成标准单文件。

如果产品方向改变，应先修改 `PRODUCT.md`，再检查架构和所有模块文档，不允许只改 UI。

## 技术验证入口

- [阶段 0 出口条件逐条判定](spikes/SPK-0012-exit-criteria.md)：**当前阶段状态的权威页**；条件本身的定义见[路线图](plan/ROADMAP.md)。
- [原生 UI 验证计划](plan/NATIVE_UI_VALIDATION.md)：固定候选顺序、共同验收集与报告要求。
- [全栈候选与验证顺序](plan/TECH_STACK.md)：库候选、替代方案和验收，不限于 GUI。
- [WASM 兼容计划](plan/WASM.md)：共享核心、浏览器宿主与桌面能力边界。
- [阶段 0 验证报告](spikes/README.md)：报告规则、模板与当前状态。
  [最新工作区复核](spikes/SPK-0016-working-tree-status.md)：新增实现、复现命令与未关闭的出口条件。
- [Mogan LaTeX 实现参考](research/MOGAN_LATEX.md)：固定提交源码依据与独立 Rust 实现方向。


扩展设计：

- [AI 与 MCP 扩展设计](plan/AI_MCP.md)：Pi agent、AI proposal、MCP server、权限与审计。
- [WASM 插件与 WIT 组件计划](plan/WASM_PLUGINS.md)：Component Model、WIT world、能力隔离和跨语言兼容。

- [会话与扩展组织计划](plan/EXTENSION_ORGANIZATION.md)：session、Pi/MCP 适配器、插件契约与运行时边界。
