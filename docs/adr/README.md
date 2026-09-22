# Architecture Decision Records

ADR 记录会约束多个模块、难以逆转或影响数据兼容性的决定。主设计文档描述当前目标，ADR 解释为何
选择某种实现以及如何替换。

文件名格式：`ADR-NNNN-short-title.md`，模板见 [ADR-0000-template.md](ADR-0000-template.md)。
前缀 `ADR-` 不可省略：`docs/adr/` 与 `docs/spikes/` 的编号空间**各自独立且重叠**（都有 0001 起），
裸编号无法唯一确定目标；跨引用一律写「ADR NNNN」或「报告 NNNN」。状态使用
Proposed、Accepted、Superseded、Rejected。替代旧 ADR 时新增文件并相互链接，不改写历史结论。
每个 ADR 必须包含背景、验证方法、候选方案、决策、后果和替换方案；决策要有实验证据，原始数据放
`docs/spikes/`。

## 已建立

| ADR | 状态 | 当前结论 |
|---|---|---|
| [0001：团队单一源码语言与混合项目](ADR-0001-mixed-source-team-editing.md) | Proposed | 用户要求已确认；协调协议、桥接与存储实现待阶段 0 验证 |
| [0002：原生技术栈与 UI 验证顺序](ADR-0002-native-ui-validation-order.md) | Accepted | 语言约束与验证顺序已确认；框架选定见 [ADR 0006](ADR-0006-native-ui-framework.md)（egui） |
| [0003：全栈验证范围与 WASM 优先兼容](ADR-0003-wasm-and-stack-validation.md) | Accepted | 目标与验证方向已确认，具体库与浏览器发行未选定 |
| [0004：项目许可证](ADR-0004-project-license.md) | Accepted | 采用 MIT OR Apache-2.0 双许可，依赖许可政策由 `cargo deny` 强制 |
| [0005：允许 Apache-2.0 WITH LLVM-exception](ADR-0005-llvm-exception-license.md) | Accepted | 该例外权限严格宽于 Apache-2.0，加入允许清单；`deny.toml` 已建 |
| [0006：原生桌面框架选型 —— egui](ADR-0006-native-ui-framework.md) | Accepted | 采用 egui/eframe 0.36.2；Iced 缺可访问性、GPUI 缺文本输入控件、EUI-NEO 不投入、Slint 被许可证阻断 |
| [0007：Typst 生成、编译集成与位置映射](ADR-0007-typst-integration.md) | Accepted | 锚点 + `Introspector::position` 双向定位；持久 World；异步编译 + 结果门；锁定 typst 0.15.1 |
| [0008：源码 reconcile 策略与构建工具链隔离](ADR-0008-reconcile-and-toolchain-isolation.md) | Accepted | 行+范围归因、不确定即冲突、Raw 逐字保留；LaTeX 必须跑在 OS 级沙箱内 |
| [0010：允许 BSL-1.0 依赖](ADR-0010-bsl-license.md) | Accepted | Boost Software License 是宽松许可（与 MIT 同类），加入允许清单 |
| [0011：允许字体资产许可（OFL-1.1 / Ubuntu-font-1.0）](ADR-0011-font-asset-licenses.md) | Accepted | 仅限未修改字体；须登记来源与许可；同时修正 bans 对路径依赖的误伤 |
| [0009：CRDT 引擎选型](ADR-0009-crdt-engine.md) | **Accepted** | **选定 Loro 1.16.0**：只有它同时具备可移动树与能过滤远端输入的内置 undo，且都端到端跑通（报告 0014）；性能对比留待阶段 1 首个迭代 |

| [0012：Typst worker 与运行时挂载](ADR-0012-typst-worker-isolation.md) | Accepted | 混合构建 Typst 编译/内省/导出进入受控子进程；固定工具及资源白名单（执行者 Codex） |
| [0013：Typst 编辑场景](ADR-0013-typst-editor-scene.md) | Accepted（spike） | 左侧使用隔离编译的图像与同 revision glyph 几何；过期画面禁用旧坐标交互 |

## 待补（阶段出口依赖）

[ADR 0023](ADR-0023-stage0-feasibility-boundary.md)依据用户要求固定阶段 0 可行性边界，纠正下表的阶段归属。

[ADR 0021](ADR-0021-native-accessibility-patches.md)记录源码视口与可追溯 egui/AccessKit 补丁。
[ADR 0022](ADR-0022-process-tree-resource-limits.md)规定 cgroup 进程树和 tmpfs 临时空间预算。

[ADR 0017](ADR-0017-pdf-probe-runtime.md)为 PDF 产物探针固定独立最小用途清单。
[ADR 0018](ADR-0018-vector-fidelity-bridge.md)区分语义转换与源引擎矢量载体，规定行内度量和链接注释覆盖层。
[ADR 0019](ADR-0019-recovery-worker-isolation.md)将恢复实验的编译/探针隔离，并规定跨进程稳定签名。
[ADR 0020](ADR-0020-native-source-replicas.md)限定原生源码权威双副本与受信消息队列实验。

[ADR 0016](ADR-0016-team-window-gate-spike.md)限定团队门禁的单窗口模拟适配，不替代正式协作协议。

[ADR 0015](ADR-0015-ui-session-recovery-spike.md)限定实验会话恢复，不关闭正式 WAL/快照与 schema 决策。

下表是必须补齐的 ADR。"缺文件"表示尚未建立；关闭阶段是该 ADR 必须完成的最晚阶段，对应验证项见
[路线图](../plan/ROADMAP.md)。每项在开始前必须指定负责人并登记到本表，未指定不得开工。

| 主题 | 关闭阶段 | 依赖验证项 | 状态 |
|---|---|---|---|
| LaTeX/Typst parser 的正式实现 | 1，源码理解扩展前 | 正式源码适配器 | P0 子集策略由 ADR 0008 裁决；正式实现待阶段 1，依据 ADR 0023 |
| CRDT engine（Loro → Yrs → Automerge） | 0 | 阶段 0 第 4 项 | **已关闭**：ADR 0009 Accepted，选定 Loro 1.16.0（报告 0014） |
| WAL、快照与内容寻址格式 | 1，存储开工前 | 正式 storage；先比较 SQLite/rusqlite → redb | 缺文件；P0 恢复机制证据保留，阶段归属依据 ADR 0023 |
| 原生语义文档持久化 schema | 1，存储开工前 | 原生身份/历史恢复 | 缺文件；ADR 0015 的实验 DTO 不转正，阶段归属依据 ADR 0023 |
| Markdown 方言基线 | 3 | 阶段 3 | 缺文件 |
| sync-server 数据库与 blob 存储 | 5 | 阶段 5 | 缺文件 |
| 端到端加密是否进入首发 | 5 | 阶段 5 | 缺文件 |
| HTML/DOCX/EPUB 转换后端 | 6 | 阶段 6 | 缺文件 |

阶段 0 各项验证完成后，对应 ADR 必须在同一提交建立并回链报告；验证失败时 ADR 记录失败数据与替换路径。

[ADR 0024](ADR-0024-local-paragraph-integration.md)：基础本地段落与 UI 接入边界，不替代 shared SDG/存储/parser 门禁。
