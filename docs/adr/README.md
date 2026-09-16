# Architecture Decision Records

ADR 记录会约束多个模块、难以逆转或影响数据兼容性的决定。主设计文档描述当前目标，ADR 解释为何
选择某种实现以及如何替换。

文件名格式：`NNNN-short-title.md`，模板见 [0000-template.md](0000-template.md)。状态使用
Proposed、Accepted、Superseded、Rejected。替代旧 ADR 时新增文件并相互链接，不改写历史结论。
每个 ADR 必须包含背景、验证方法、候选方案、决策、后果和替换方案；决策要有实验证据，原始数据放
`docs/spikes/`。

## 已建立

| ADR | 状态 | 当前结论 |
|---|---|---|
| [0001：团队单一源码语言与混合项目](0001-mixed-source-team-editing.md) | Proposed | 用户要求已确认；协调协议、桥接与存储实现待阶段 0 验证 |
| [0002：原生技术栈与 UI 验证顺序](0002-native-ui-validation-order.md) | Accepted | 语言约束与验证顺序已确认，具体框架未选定 |
| [0003：全栈验证范围与 WASM 优先兼容](0003-wasm-and-stack-validation.md) | Accepted | 目标与验证方向已确认，具体库与浏览器发行未选定 |
| [0004：项目许可证](0004-project-license.md) | Accepted | 采用 MIT OR Apache-2.0 双许可，依赖许可政策由 `cargo deny` 强制 |
| [0005：允许 Apache-2.0 WITH LLVM-exception](0005-llvm-exception-license.md) | Accepted | 该例外权限严格宽于 Apache-2.0，加入允许清单；`deny.toml` 已建 |

## 待补（阶段出口依赖）

下表是必须补齐的 ADR。"缺文件"表示尚未建立；关闭阶段是该 ADR 必须完成的最晚阶段，对应验证项见
[路线图](../ROADMAP.md)。每项在开始前必须指定负责人并登记到本表，未指定不得开工。

| 主题 | 关闭阶段 | 依赖验证项 | 状态 |
|---|---|---|---|
| 原生桌面框架、结构编辑与源码编辑内核最终选型 | 0 | 阶段 0 第 1 项 | 缺文件 |
| Typst generator、编译集成与 SourceMap | 0 | 阶段 0 第 2 项 | 缺文件 |
| LaTeX/Typst parser、reconcile 策略与构建工具链 | 0 | 阶段 0 第 3、7 项 | 缺文件 |
| CRDT engine（Loro → Yrs → Automerge） | 0 | 阶段 0 第 4 项 | 缺文件 |
| WAL、快照与内容寻址格式 | 0 | 阶段 0 第 5 项 | 缺文件 |
| 原生语义文档持久化 schema | 0 | 阶段 0 第 5 项 | 缺文件 |
| Markdown 方言基线 | 3 | 阶段 3 | 缺文件 |
| sync-server 数据库与 blob 存储 | 5 | 阶段 5 | 缺文件 |
| 端到端加密是否进入首发 | 5 | 阶段 5 | 缺文件 |
| HTML/DOCX/EPUB 转换后端 | 6 | 阶段 6 | 缺文件 |

阶段 0 各项验证完成后，对应 ADR 必须在同一提交建立并回链报告；验证失败时 ADR 记录失败数据与替换路径。
