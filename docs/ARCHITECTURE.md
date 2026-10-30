# 总体架构

## 1. 运行形态

桌面应用采用“原生 UI + Source Studio + Rust 核心”的分层结构。原生 UI 负责页面式结构编辑、
源码编辑器、工具栏、预览和面板；核心负责语义文档、源文件、CRDT、历史、格式投影、排版和同步。
两者通过带版本的命令/事件协议通信。同进程 Rust 通道优先；构建隔离仍使用子进程，独立 core 进程需验证后决定。
应用逻辑优先 Rust，必要时通过 C ABI 或经验证的 C++ 桥接引入 C/C++/Zig，不使用 npm/JS 编辑器或 WebView。
UI 按 [原生 UI 验证计划](plan/NATIVE_UI_VALIDATION.md) 验证 Iced → GPUI → C++ EUI-NEO → Slint 或 egui。
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

不让前端直接写文件，也不让同步服务参与格式解析。核心进程是单机状态协调者。
项目语言在创建/导入时固定为 Typst 或 LaTeX，工作区按语言装配独立内核；内核不互相调用或共享文档模型。
Typst 内核负责 SDG 权威的原生结构项目及 .typ 源码项目；LaTeX 内核以 .tex 源码为权威。
跨语言只经「转换为…」建立新项目并提供兼容报告，见 [ADR 0041](adr/ADR-0041-separate-latex-typst-kernels.md)。

## 2. 分层与依赖

```text
app → 按 ProjectLanguage 装配内核与共享底座
├── Typst 内核 → model(SDG)/document/typst
├── LaTeX 内核 → latex(source/CST、预览适配)
└── 共享底座 → storage/history/collab/build/诊断面板

render → 对应内核的预览事件与定位
转换/导出 → format(CST → IR → 目标生成器) → build
sync-server → collab protocol + storage DTO
```

- `model` 的共享身份/项目元数据与 Typst SDG 类型区分职责，不依赖文件系统、网络、UI 或具体 CRDT。
- Typst 内核的 `document` 维护语义图、焦点、结构命令和 Typst 源码绑定，不依赖 UI。
- `history` 依赖抽象更新与语义 patch，不依赖 Yrs/Automerge 私有类型。
- `collab` 封装共享树与共享文本；其他模块只看 `SharedDocument` 接口。
- 格式适配器不直接启动外部进程；跨格式只走一次性转换/导出，构建由 `build` 统一隔离。
- `render` 按项目语言展示对应内核的结果与定位，预览和最终输出使用同一引擎。
- 窗口壳、命令注册表、i18n、storage、history、collab、build 沙箱与诊断面板属于共享底座。
- `app` 只编排，不包含解析器、历史算法或权限规则。

## 3. 核心数据流

### Typst 视觉结构输入

1. UI 发送 `ApplySemanticEdit(document, base_revision, selection, edit)`。
2. document 校验焦点、槽位、节点 capability 和前置条件，生成语义 patch。
3. collab 在带 actor/origin 的共享树事务中应用 patch，返回 update 与新 revision。
4. history 写入动作元数据、forward/inverse anchors 和 update hash。
5. storage 先追加日志并 fsync，再异步物化原生文档与选定源码 projection。
6. Typst 内核按新 revision 生成排版与 SourceMap，render 展示预览；最终输出同样使用 Typst。
7. 导出 LaTeX 源码时生成转换报告并由目标工具链验证，不改变当前项目语言。

### 源码输入

1. UI 发送 `ApplySourceEdits(binding, base_revision, editor_edits)`。
   请求按固定项目语言路由到对应内核，校验当前权限、revision 与 patch 前置条件。
   离线输入保留持久化与同步状态，不能因重连或冲突丢失草稿。
2. SourceAuthority 项目直接更新共享文本，对应内核增量解析并刷新诊断与定位。LaTeX 预览不经过 SDG/Typst。
3. Typst StructuredAuthority 项目先形成 dirty source generation，解析新旧 CST 差异并生成 ReconcilePlan。
4. 可逆修改自动转为 SemanticPatch；Raw/Degraded 修改要求确认；无法安全映射的修改拒绝并保留草稿。
5. 接受后的语义或文本动作与视觉输入进入同一 history/collab/storage/render 管线。

同语言源码与结构编辑通过 CRDT 协作。LaTeX 实时适配层以 LuaLaTeX + rtex 为候选（待评估），
仅向上暴露段落更新、页面更新、定位与诊断；pdfLaTeX/XeLaTeX 使用后台整编并标明预览来源与延迟。
混合组件、跨引擎桥接和团队语言切换已按 ADR 0041 推迟，文件操作见
[项目语言、跨格式转换与文件操作](MIXED_SOURCE_EDITING.md)。

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

外部修改与普通编辑使用同一权限、revision 和冲突校验，不能通过 watcher 静默覆盖共享正文。

### 转换与文件输出

format 从不可变 SDG 或 source/CST 快照产生转换 IR 与目标源码，逐节点报告
Exact/Equivalent/RawPreserved/Degraded/Dropped；build 在沙箱中执行目标工具链。
Raw 同方言无损保留，跨方言降级或阻止导出。storage 保存权威正文与草稿；export 验证临时目录后发布，
不改变原项目语言或权威内容。混合重建包与跨引擎资源桥接已按 ADR 0041 推迟。

## 4. 一致性边界

- 单个文档的一次语义动作或源码动作是原子的。
- 同语言跨文件重构使用 `WorkspaceTransaction`：先验证所有 patch，再逐资源提交；日志中有共同 group id。
- 跨语言转换创建独立目标项目，提供兼容报告，失败保留原项目且不发布半成品。
- 文件系统无法提供多文件原子替换，因此使用 write-ahead manifest；恢复时完成或回滚物化。
- 原生项目的共享树与源码项目的共享文本都通过同一 Action/Checkpoint 语义管理。
- CRDT update 是幂等传输单元；`Action` 是 UI 和撤销单元；`Checkpoint` 是版本图节点。
- 派生物带 `RevisionId`，过期解析或构建结果直接丢弃。

## 5. 进程与线程

- UI 主线程只渲染和发送命令。
- core 使用异步 runtime；每个打开项目有一个串行 session actor，避免跨模块锁顺序。
- CPU 密集解析、diff 和导出进入受限 worker pool。
- 外部构建进入独立子进程/容器，具备超时、内存和文件访问限制。
- sync-server 不运行排版/格式语义，执行身份、权限 epoch、配额与幂等持久化校验。

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
阶段 0 已关闭的门禁：[原生框架](adr/ADR-0006-native-ui-framework.md)（egui）、[Typst 集成](adr/ADR-0007-typst-integration.md)、[reconcile 与工具链隔离](adr/ADR-0008-reconcile-and-toolchain-isolation.md)、[CRDT 引擎](adr/ADR-0009-crdt-engine.md)（Loro）。团队语言协调与混合构建已按 [ADR 0041](adr/ADR-0041-separate-latex-typst-kernels.md) 推迟，
[报告 0009](spikes/SPK-0009-team-language.md) 与[报告 0010](spikes/SPK-0010-mixed-build.md) 仅作追溯记录，不约束后续阶段出口。

## 全栈选型与 WASM 宿主

候选统一维护于[TECH_STACK](plan/TECH_STACK.md)，移植边界见[WASM](plan/WASM.md)。model、格式纯逻辑、history 与协议层不得直接依赖文件、进程、GUI、具体数据库或 Tokio 多线程运行时。原生用 Tokio/CPU workers 和 OS 适配；浏览器用 futures/Worker、异步存储及资源注入适配，同一语义与协议测试跨端运行。可移植核心不泄露平台句柄，C/C++/Zig 依赖必须单独验证 WASM 产物。

## 扩展边界的后续提案

直接接入 Pi agent、MCP server 与 WIT 插件的组织方向见 [组织计划](plan/EXTENSION_ORGANIZATION.md)和 [ADR 0025](adr/ADR-0025-extension-boundaries.md)（Proposed）。计划将可复用 ProjectSession 从 app 编排中提取，UI 与外部适配器共用写入入口；当前 LocalSession 仍受 ADR 0024 限定。
