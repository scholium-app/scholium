# Scholium 项目总览

Scholium 是面向研究写作的本地优先协作工作区，以即时排版的结构化编辑为默认体验，同时直接编辑
LaTeX、Typst、Markdown 和 BibTeX，提供非线性历史、多人协作、双后端预览、格式转换和“复制为”。

本文件只回答“做什么”和“从哪里开始”。详细设计见 [设计文档索引](../README.md)。

## 核心原则

- 一个项目只有一个权威模型：原生项目以语义文档图为真，源码项目以源文件为真。
- 可视化编辑、源码编辑和命令输入统一生成语义 Action。
- Typst 负责低延迟预览；选择 LaTeX 发布时，最终结果以 LaTeX 验证构建为准。
- 打开和保存原生格式不得吞掉注释、空白、自定义宏或未知语法。
- CRDT 收敛、用户动作和版本 DAG 是不同层次，不允许混成一个历史栈。
- 撤销和 revert 都生成新的补偿变更，不倒退或重写共享历史。
- 离线编辑是正常状态；服务端不负责内容合并。
- 跨格式转换允许明确降级，不允许静默丢失。
- 不搬运废弃原型、Mogan 或 TeXmacs 的实现。

源码编辑语言、正文权威、排版宿主、导出目标独立。团队在同一共享分支只同时编辑一种源码语言；
两种编译器可并行。混用覆盖正文、数学、图表、宏、模板和跨片段引用，两种格式均可打开保存和输出。
共享分支断网后的源码修改进入草稿/fork，重连后显式合入。

## 原生 UI 验证

按 Iced → GPUI → C++ EUI-NEO → Slint 或 egui 依次验证。应用以 Rust 为主，必要原生组件通过 C/C++/Zig 接口接入。
旧 ProseMirror、CodeMirror/Monaco、Tauri/WebView 方向废止，npm 实验不能作为新路线通过证据。
共同验收见 [原生 UI 验证计划](NATIVE_UI_VALIDATION.md)，最终框架需实测后另立选型 ADR。

## 实现模块

正式 workspace 由 model、document、storage、history、collab、format、latex、typst、markdown、bib、
render、build、app 和 sync-server 组成。职责与 API 见 [模块索引](../modules/README.md)。

## 实施顺序

桌面壳与主要布局参照 [P1 排版工作区规划](P1_UI.md)，其中包含两种工作方式的参考图、
必须保持的交互原则、可调整细节及评审检查。该基准允许基于原生平台与实际可用性迭代，不要求像素复刻。

先验证结构编辑、源码 reconcile、团队语言切换、完整范围的混合构建、CRDT 与安全恢复，再建立最小编辑/
文件交换闭环；随后扩展结构输入、混用覆盖、历史、协作和产品化。源码打开保存与双目标输出不推迟到最后。
每个阶段必须通过出口条件，详见 [路线图](ROADMAP.md) 与 [混合源码与团队编辑](../MIXED_SOURCE_EDITING.md)。

## 废弃原型处理

废弃原型的 `crates/`、`spike/`、锁文件和专用 CI/审计配置已经删除；源码仍可从 Git 历史恢复，
实验结论保存在 `docs/archive/`。正式实现从空 workspace 建立新模块，不复制旧实现后改名。

## 全栈与可移植性

按[全栈候选](TECH_STACK.md)验证 UI 以外的解析、编译、协作、存储、渲染和服务端；候选不等于最终依赖。优先支持[WASM 核心与浏览器适配](WASM.md)，保持原生桌面路线及已指定 GUI 顺序；完整浏览器发行范围待验证。LaTeX 参考[Mogan 转换分层与模板适配](../research/MOGAN_LATEX.md)，独立 Rust 实现。
