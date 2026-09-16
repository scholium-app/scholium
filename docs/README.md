# Scholium 设计文档

这里是正式实现的设计依据。`PLAN.md` 只保存项目总览，不再承载全部设计。

## 阅读顺序

先读 [项目总览](PLAN.md) 了解"做什么"和"从哪里开始"，再按下列顺序深入。

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
11. [路线图](ROADMAP.md)：阶段、交付物、出口条件和依赖关系。
12. [模块索引](modules/README.md)：每个 crate、应用和服务的内部设计。
13. [ADR 索引](adr/README.md)：经过验证的技术选择与替代方案。
14. [Liii STEM 体验调研](research/LIIISTEM_EXPERIENCE.md)：参考交互、取舍与映射。

## 文档状态

- 开发规范与硬性约束见仓库根 [AGENT.md](../AGENT.md)；文档纪律以它为准。
- [`archive/`](archive/README.md) 保存废弃原型的背景、实验结果和阶段记录。
- 历史记录不构成正式架构约束；只有本索引列出的文档定义新实现。
- 尚未验证的技术选择必须标记为“待验证”，不能写成既成事实；验证报告见 [`spikes/`](spikes/README.md)。

## 当前基本假设

- 默认体验是即时排版的结构编辑；源码模式是完整、可写的第一等工作台。
- 原生结构项目以语义文档图为真；外部源码项目以 lossless CST/source 为真。
- Typst 提供快速交互预览，LaTeX 可作为最终发布与验证后端。
- 桌面 UI、核心与服务端优先 Rust，必要时使用 C/C++/Zig；采用原生窗口/文本/绘制，不采用 npm 或 WebView。
- UI 按 [原生 UI 验证计划](NATIVE_UI_VALIDATION.md) 的顺序验证后**已选定 egui / eframe 0.36.2**（[ADR 0006](adr/0006-native-ui-framework.md)）：Iced 缺可访问性、GPUI 缺文本输入控件、EUI-NEO 无无障碍不投入、Slint 被许可证政策阻断。
- 标准源文件可脱离 Scholium 使用；协作历史是增强层，不是文件可读性的前提。

- 同一共享项目分支在团队范围内只允许一种 LaTeX/Typst 源码语言写入，编译器不受此限制。
- 混合项目保留两种原文；标准目标包与双工具链重建包分别验证可移植性，不能把混合源码当成标准单文件。

如果产品方向改变，应先修改 `PRODUCT.md`，再检查架构和所有模块文档，不允许只改 UI。

## 技术验证入口

- [原生 UI 验证计划](NATIVE_UI_VALIDATION.md)：固定候选顺序、共同验收集与报告要求。
- [全栈候选与验证顺序](TECH_STACK.md)：库候选、替代方案和验收，不限于 GUI。
- [WASM 兼容计划](WASM.md)：共享核心、浏览器宿主与桌面能力边界。
- [阶段 0 验证报告](spikes/README.md)：报告规则、模板与当前状态。
- [Mogan LaTeX 实现参考](research/MOGAN_LATEX.md)：固定提交源码依据与独立 Rust 实现方向。
