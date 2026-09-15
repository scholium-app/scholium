# Architecture Decision Records

ADR 记录会约束多个模块、难以逆转或影响数据兼容性的决定。主设计文档描述当前目标，ADR 解释为何
选择某种实现以及如何替换。

首批必须完成：

- [0001：团队单一源码语言与混合项目](0001-mixed-source-team-editing.md)（Proposed）：用户要求已确认，实现待验证。

- [0002：原生技术栈与 UI 验证顺序](0002-native-ui-validation-order.md)（Accepted）：约束及顺序已确认，具体框架未选定。
- 原生桌面框架、结构编辑和源码编辑内核的最终选型（须实验结果）。
- CRDT engine。
- WAL、快照和内容寻址格式。
- 原生语义文档持久化 schema。
- Typst generator、编译集成与 SourceMap。
- LaTeX/Typst parser、reconcile 策略与构建工具链。
- Markdown 方言基线。
- HTML/DOCX/EPUB 转换后端。
- sync-server 数据库与 blob 存储。
- 端到端加密是否进入首发。

文件名格式：`NNNN-short-title.md`。状态使用 Proposed、Accepted、Superseded、Rejected。替代旧 ADR 时新增
文件并相互链接，不改写历史结论。

- [0003：全栈验证范围与 WASM 优先兼容](0003-wasm-and-stack-validation.md)（Accepted）：目标和验证方向已确认，具体库与浏览器发行未选定。
