# 总体架构

## 1. 运行形态

桌面应用采用“原生 UI + Source Studio + Rust 核心”的分层结构。原生 UI 负责页面式结构编辑、
源码编辑器、工具栏、预览和面板；核心负责语义文档、源文件、CRDT、历史、格式投影、排版和同步。
两者通过带版本的命令/事件协议通信。同进程 Rust 通道优先；构建隔离仍使用子进程，独立 core 进程需验证后决定。
应用逻辑优先 Rust，必要时通过 C ABI 或经验证的 C++ 桥接引入 C/C++/Zig，不使用 npm/JS 编辑器或 WebView。
UI 按 [原生 UI 验证计划](NATIVE_UI_VALIDATION.md) 验证 Iced → GPUI → C++ EUI-NEO → Slint 或 egui。
框架状态只保存视图投影，原生编辑控件不能成为第二份权威文档。

```text
Desktop UI      visual editor · source studio · timeline · preview · citations
     │
     │  command/event protocol（带版本信封，见第 6 节）
     ▼
Core process    session · document · history · collab · format · source · render · build · storage
     │
     ├── local filesystem
     └── sync protocol ── relay service
```

不让前端直接写文件，也不让同步服务参与格式解析。核心进程是单机状态协调者；共享分支的源码语言门禁
由服务端持久化协调，客户端不能仅凭本机 UI 状态开放另一语言写入。

## 2. 分层与依赖

```text
model
├── document ── history
├── collab ──── history/document
├── format ──── latex/typst/markdown/bib
├── render ──── typst adapter + build
└── storage

app → document/history/collab/format/render/storage

sync-server → collab protocol/write-set validator + storage DTO
```

- `model` 不依赖文件系统、网络、UI 或具体 CRDT。
- `document` 维护语义图、焦点、结构命令和源码绑定，不依赖 UI。
- `history` 依赖抽象更新与语义 patch，不依赖 Yrs/Automerge 私有类型。
- `collab` 封装共享树与共享文本；其他模块只看 `SharedDocument` 接口。
- 格式适配器不直接启动外部进程；构建由 `build` 统一隔离。
- `render` 生成 Typst 快速预览与 SourceMap；LaTeX 最终构建仍走 `build`。
- `app` 只编排，不包含解析器、历史算法或权限规则。

## 3. 核心数据流

### 视觉结构输入

1. UI 发送 `ApplySemanticEdit(document, base_revision, selection, edit)`。
2. document 校验焦点、槽位、节点 capability 和前置条件，生成语义 patch。
3. collab 在带 actor/origin 的共享树事务中应用 patch，返回 update 与新 revision。
4. history 写入动作元数据、forward/inverse anchors 和 update hash。
5. storage 先追加日志并 fsync，再异步物化原生文档与选定源码 projection。
6. render 按新 revision 生成 Typst 源码、SourceMap 和快速预览。
7. 若启用 LaTeX 验证，build 在后台生成/构建对应 revision。

### 源码输入

1. UI 发送 `ApplySourceEdits(binding, base_revision, editor_edits)`。
   请求另带 scope、dialect、edit_epoch、permit_id 和写集。门禁覆盖源码文件、共享生成草稿和 ForeignSource 原文。
   共享提交在协调者验证许可与实际写集后才应用；断线/无许可的输入持久化为本地草稿或 fork。
2. SourceAuthority 项目直接更新共享文本，格式适配器增量解析并刷新语义投影。
3. StructuredAuthority 项目先形成 dirty source generation，解析新旧 CST 差异并生成 ReconcilePlan。
4. 可逆修改自动转为 SemanticPatch；Raw/Degraded 修改要求确认；无法安全映射的修改拒绝并保留草稿。
5. 接受后的语义或文本动作与视觉输入进入同一 history/collab/storage/render 管线。

纯语义动作可与活动语言源码协作；改写宏、模板、外语原文的语义动作必须先走同一写集门禁。
SourceAuthority 的视觉操作会写权威文本，因此也必须按实际 patch 检查活动语言；纯语义例外只用于 SDG。
团队语言切换使用 Active → Draining → 新 epoch Active 的控制流程，等已许可写入冲刷并隔离旧客户端后才开放
新语言。协议/持久化原子边界及断网取舍见 [混合源码与团队编辑](MIXED_SOURCE_EDITING.md)。

任何一步失败都返回结构化错误。日志落盘前不得向 UI 宣称操作已持久化。

### 远端更新

1. sync 校验会话、项目、actor、序号和权限。
2. collab 应用幂等 update；重复包不产生新动作。
3. history 记录远端动作元数据，但不加入本地 undo scope。
4. storage 追加日志；session 根据当前视图发布 SemanticDelta 或 TextEdit。
5. document/format/render/build 与本地输入走同一条派生管线。

### 外部文件修改

1. 仅 SourceAuthority 项目观察权威文件；StructuredAuthority 的生成源码不接受外部静默回写。
2. watcher 读取稳定后的文件，计算与上次物化版本的 diff。
3. 若内存无未物化更改，将 diff 作为 `Origin::External` 动作应用。
4. 若两边都变更，创建外部变更预览；能安全重定位则合入，否则要求选择或合并。
5. 永远不把磁盘文件直接覆盖进 CRDT 状态。

外部修改若涉及非活动语言或无有效许可，先保存为待合入草稿；不得通过 watcher 绕过团队门禁。

### 混合构建与文件输出

format 从不可变 SDG/source/组件快照产生转换计划；build 执行外语组件、宿主排版和有限轮引用桥接；render 展示
带原文位置的结果。工具链按依赖并行，不被编辑语言互斥限制。storage 保存组件原文、绑定和草稿；export 写临时
目标目录并验证后发布。标准目标包和双工具链重建包分别记录依赖；未解决正文/模板/引用不能正式发布。

## 4. 一致性边界

- 单个文档的一次语义动作或源码动作是原子的。
- 同语言跨文件重构使用 `WorkspaceTransaction`：先验证所有 patch，再逐资源提交；日志中有共同 group id。
- 跨语言重构拆成带恢复状态的多步工作流，每步按当前语言许可提交；整体不宣称原子，未完成时阻止受影响的
  正式输出，并提供继续/补偿路径。
- 文件系统无法提供多文件原子替换，因此使用 write-ahead manifest；恢复时完成或回滚物化。
- 原生项目的共享树与源码项目的共享文本都通过同一 Action/Checkpoint 语义管理。
- CRDT update 是幂等传输单元；`Action` 是 UI 和撤销单元；`Checkpoint` 是版本图节点。
- 派生物带 `RevisionId`，过期解析或构建结果直接丢弃。

## 5. 进程与线程

- UI 主线程只渲染和发送命令。
- core 使用异步 runtime；每个打开项目有一个串行 session actor，避免跨模块锁顺序。
- CPU 密集解析、diff 和导出进入受限 worker pool。
- 外部构建进入独立子进程/容器，具备超时、内存和文件访问限制。
- sync-server 不运行排版/格式语义，但增加源码控制记录的顺序提交、许可隔离和结构级写集校验。
  控制记录校验与共享源码更新持久化同一原子边界，不能信任客户端自报语言；具体校验方案为阶段 0 门禁。

## 6. 协议

UI/core 与 client/server 都使用显式版本信封：

```text
Envelope { protocol_version, request_id, project_id, actor_id, payload }
```

新增字段必须可忽略；破坏性变化提升协议版本。错误包含稳定 code、用户消息、调试上下文和可选
源位置。协议 DTO 与领域类型分离，禁止直接序列化内部 struct 充当长期协议。

## 7. 可观测性

- 结构化日志默认不包含正文、引用密钥和访问令牌。
- 每次动作、解析、构建、同步有 correlation id。
- 指标包含输入确认延迟、日志落盘延迟、解析耗时、构建耗时、update 大小、重连次数和冲突数。
- 用户可导出脱敏诊断包；必须在 UI 中预览将被收集的文件和字段。

## 8. 架构决策门禁

结构编辑器框架、源码编辑器、CRDT、Typst 集成、TeX 工具链和转换后端都必须先完成 spike 与 ADR。模块边界先稳定，
具体第三方库允许替换；任何库类型泄漏到 `model` 或长期存储格式都视为门禁失败。
阶段 0 已关闭的门禁：[原生框架](adr/0006-native-ui-framework.md)（egui）、[Typst 集成](adr/0007-typst-integration.md)、[reconcile 与工具链隔离](adr/0008-reconcile-and-toolchain-isolation.md)、[CRDT 引擎](adr/0009-crdt-engine.md)（Loro）；团队语言协调与全范围混合构建的验证见[报告 0009](spikes/0009-team-language.md) 与[报告 0010](spikes/0010-mixed-build.md)，**协调协议与桥接的正式实现仍待做**（见 [ADR 0001](adr/0001-mixed-source-team-editing.md)，仍为 Proposed）。

## 全栈选型与 WASM 宿主

候选统一维护于[TECH_STACK](TECH_STACK.md)，移植边界见[WASM](WASM.md)。model、格式纯逻辑、history 与协议层不得直接依赖文件、进程、GUI、具体数据库或 Tokio 多线程运行时。原生用 Tokio/CPU workers 和 OS 适配；浏览器用 futures/Worker、异步存储及资源注入适配，同一语义与协议测试跨端运行。可移植核心不泄露平台句柄，C/C++/Zig 依赖必须单独验证 WASM 产物。
