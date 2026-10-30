# 数据模型

## 1. 标识符

所有持久化标识符使用随机 128-bit ID 并带类型包装：

- `ProjectId`：一个协作和历史命名空间。
- `ResourceId`：项目内逻辑资源，重命名路径后保持不变。
- `DocumentId`：一个原生语义文档或源码入口的逻辑身份。
- `NodeId`：原生语义图中的稳定节点身份。
- `ActorId`：一个用户在一个设备身份下的写入者。
- `ActionId`：一个用户可理解、可撤销的动作。
- `ChangeId`：内容寻址的不可变历史记录。
- `CheckpointId`：命名版本节点。
- `RevisionId`：资源当前状态的短生命周期版本，用于拒绝过期派生结果。

协作、构建与导出任务同样使用类型包装标识符，不用裸字符串或自增整数：

- `BranchId`：命名分支引用；共享编辑范围 = `ProjectId + BranchId`。
- `UserId` / `DeviceId`：账号身份与设备身份；`ActorId` 绑定设备身份，不跨设备克隆。
- `DraftId`：未提交源码草稿。
- `GenerationId`：一次生成源码基线；reconcile 只在同一 generation 内有效。
- `PlanId`：plan/confirm 两阶段命令的计划标识，confirm 必须回带。
- `ConflictId`：冲突预览与合并决议。
- `BuildId` / `ExportId`：一次构建或导出任务。

路径不是身份，显示名不是身份，墙上时间不参与因果排序。协议 DTO 与领域合约中出现的标识字段一律使用
上述类型包装，不使用裸字符串或整数。

## 2. 项目与资源

```rust
struct Project {
    id: ProjectId,
    root: ProjectRoot,
    language: ProjectLanguage,
    authority: AuthorityMode,
    resources: ResourceCatalog,
    entrypoints: Vec<EntryPoint>,
    build_profiles: Vec<BuildProfile>,
    layout_profiles: Vec<LayoutProfile>,
}

enum ProjectLanguage { Typst, LaTeX }

enum AuthorityMode {
    Structured { document: DocumentId },
    Source { entrypoint: ResourceId, dialect: Dialect },
}

struct Resource {
    id: ResourceId,
    path: RelativePath,
    kind: ResourceKind,
    encoding: TextEncoding,
    line_ending: LineEnding,
}
```

ProjectLanguage 在项目创建/导入时固定，项目内只有一种排版语言。Typst 可选 Structured 或 .typ Source；
LaTeX 只采用 .tex SourceAuthority。Markdown/BibTeX 是资源，不改变项目语言。
AuthorityMode 只决定该语言的权威形态；SDG 属于 Typst 内核，LaTeX 不共享其文档模型。
LayoutProfile 描述本引擎的排版/模板，ExportRequest 描述单次导出目标，不改变项目语言。
以下为领域合约草案，序列化和持久化版本待 ADR 验证：

```text
SourceDraft { draft_id, actor_id, binding_id, dialect, base_generation,
              base_revisions, source_resource, durability, visibility }
ExportRequest { input_snapshot, target_format, layout_profile, packaging, draft_policy }
```

草稿持久化与语义提交状态分离，不因重新生成或重连而丢失原输入。跨语言转换创建独立项目并提供逐节点报告。
混合组件与团队语言切换已推迟，见 [项目语言、跨格式转换与文件操作](MIXED_SOURCE_EDITING.md)
与 [ADR 0041](adr/ADR-0041-separate-latex-typst-kernels.md)。

首发文本统一在内存中使用 UTF-8。打开其他编码时记录原编码；未编辑可原样复制，编辑后是否转为
UTF-8 必须由项目策略决定并提示。行尾风格按文件保存。

二进制附件不进入文本 CRDT。它们按内容 hash 存储，路径目录保存引用；并发替换同一路径时保留
两个 blob 并产生资源冲突，不能用 last-write-wins 静默丢弃。

## 3. 语义文档图

Typst 原生项目使用稳定 NodeId 的有序树/图：文档块形成树，label、citation、资源和注释形成引用边。

```text
Document
  FrontMatter
  Block*: Heading | Paragraph | List | Quote | Code | MathBlock
          | Figure | Table | Theorem | Proof | RawBlock
Inline*: Text | Emphasis | Strong | Code | Math | Link | Citation
         | Reference | Footnote | RawInline
Math: Row | Symbol | Fraction | Attach(base, sub?, sup?) | Root | Delimited | Matrix
      | Cases | Aligned | Operator | Accent | RawMath
```

文本叶子与 children sequence 使用 CRDT；节点属性使用带冲突保留的共享 map。删除节点形成 tombstone，
只在所有保留 checkpoint 和活跃设备均不再引用时压缩。

每个节点声明 target capabilities：LaTeX/Typst 的 Exact/Equivalent/RawPreserved/Degraded/Dropped，以及 visual-edit、
source-lens、focus-toolbar、cycle、unwrap、search-pattern 等交互能力。
Raw 节点在同方言输出时无损保留，跨方言降级或阻止导出，不以保留原文宣称成功转换。

## 4. 文本与树位置

系统同时存在四种位置：

- `EditorRange`：UTF-16 code-unit，供编辑器协议使用。
- `SourceRange`：某个不可变 `RevisionId` 上的 UTF-8 byte range，供解析器和诊断使用。
- `RelativeAnchor`：CRDT 相对位置，供光标、评论、动作 inverse 和跨更新定位使用。
- `TreeAnchor`：NodeId、slot、child affinity 与可选文本 anchor，供视觉光标和结构选区使用。

转换必须显式带资源与 revision。禁止裸 `usize` 穿过模块边界。无法重定位的 anchor 返回
`Detached`，由调用方决定提示、近似定位或取消操作。

## 5. Lossless CST 与源码绑定

每个格式适配器返回自己的 CST，但遵守共同约束：

- 所有 token 覆盖完整源文件，包含 trivia、注释和错误 token。
- 节点保存 `SourceRange`，不复制大段正文。
- 解析失败产生 error node，后续内容仍尽量可解析。
- 未修改节点序列化时直接切片原文；语义编辑只重写最小安全祖先范围。
- CST 仅对对应 revision 有效，不能跨 revision 缓存裸节点引用。

`SourceBinding` 记录 dialect、authority、generation id、CST revision 和 NodeId ↔ SourceRange 多值映射。
StructuredAuthority 的生成源码可进入 Dirty 状态，但未 reconcile 前不能覆盖语义图。
共享草稿遵守项目权限，私有离线草稿不能直接成为共享 generation。草稿独立落盘，重启保留原基线并
重新检测冲突，不能以最新生成文本覆盖用户输入。

## 6. 转换 IR

转换 IR 是语义图的无 ID 快照或源码 CST 的投影，用于跨格式操作，不是协作/保存格式：

```text
Document
  metadata
  blocks: Heading | Paragraph | List | Quote | Code | Math
          | Figure | Table | Theorem | RawBlock
Inline
  Text | Emphasis | Strong | Code | Math | Link | Citation
  | Footnote | Reference | RawInline
```

每个节点包含 `provenance`、可选 `SourceRange`、属性和 capability flags。无法规范化的内容进入
`RawBlock/RawInline { source_format, source }`，导回原格式时无损；导向其他格式时降级或阻止导出。

跨语言仅经转换 IR 与目标生成器，逐节点报告 Exact/Equivalent/RawPreserved/Degraded/Dropped，
包含未投影原文与依赖；原文来源标识只用于报告，不形成第二权威副本。
IR 不包含排版引擎对象、CRDT item id、长期 NodeId 或 UI 状态。

## 7. 诊断与能力

```rust
struct Diagnostic {
    code: DiagnosticCode,
    severity: Severity,
    message: LocalizedMessage,
    primary: Location,
    related: Vec<Location>,
    fixes: Vec<SemanticEdit>,
}
```

诊断 code 稳定，文本可本地化。fix 必须声明适用 revision 和前置条件，过期后重新计算。

每个 CST/IR 节点可报告 `Capabilities`，例如 rename、convert、preview、copy-as、structured-edit。
UI 依据能力显示功能，不依据节点名字猜测。

## 8. 意图与语义编辑

`Intent` 是动作的人类可读分类；`SemanticEdit` 是格式无关请求；`TextPatch` 是适配器输出。

```text
Intent::Typing | Paste | Format | RenameSymbol | InsertCitation
               | Import | ExternalChange | Revert | Merge

SemanticEdit::ReplaceText | Wrap | SetAttribute | RenameSymbol
                          | InsertCitation | ReplaceMath | CycleVariant
                          | InsertRow | InsertColumn | Unwrap | ReconcileSource
```

TextPatch 包含有序、不重叠的替换、原 revision、precondition hash 和格式适配器解释。应用前再次验证
precondition，失败则重算或进入冲突预览。

## 9. 项目元数据

`.scholium/project.toml` 只保存可共享的声明式配置：固定项目语言、入口、格式、构建 profile、忽略规则和资源映射。
本机路径、令牌、设备 ID、窗口布局和未发布协作密钥进入用户配置目录，不得提交到项目。

StructuredAuthority 项目还保存版本化语义正文；SourceAuthority 项目正文仍在标准源文件。
`.scholium/history/` 保存本地日志、快照和索引。删除历史不会破坏已物化正文，但会失去未物化动作、
非线性版本和协作身份，应用必须在删除前明确提示。
