# scholium-document

## 职责

维护原生项目的 Semantic Document Graph，定义结构节点、固定槽位、TreeCursor/Selection、焦点与 capability，
验证并应用 SemanticEdit，管理 StructuredAuthority/SourceAuthority 绑定和源码 reconcile。

## 非职责

不选择 CRDT 算法，不绘制页面，不生成具体 LaTeX/Typst 字符串，不写文件，不管理版本分支。

## 内部结构

```text
src/
├── graph.rs        节点、children、引用边、tombstone
├── node/           text、block、math、table、figure、citation、raw
├── cursor.rs       TreeAnchor、slot、selection、navigation
├── focus.rs        最内层焦点、祖先链、能力交集
├── edit.rs         SemanticEdit 验证与纯 patch 规划
├── commands.rs     insert/wrap/unwrap/cycle/row-column 等语义
├── invariants.rs   schema 与引用完整性验证
├── binding.rs      authority mode、SourceBinding、generation 状态
├── reconcile.rs    SourceDiff → SemanticPatch/Raw/Conflict
└── projection.rs   SourceAuthority CST → visual graph projection
```

## 节点模型

节点由 `NodeId + NodeKind + AttrMap + Slots` 构成。每种 kind 声明槽位 schema，例如 Fraction 固定 numerator/
denominator，Script 固定 base/sub/sup，Figure 固定 body/caption，Theorem 固定 title/body。可变列表使用命名
sequence slot，禁止调用方按裸 child index 猜语义。

文本存在 Text leaf；结构边存在 CRDT sequence；引用以目标 label/resource 的稳定关系表达。Raw 节点保存方言、
原源码、作用域和 fallback，不允许视觉操作穿透未知语义；可运行混合组件另用 ForeignSource 与显式作用域合约。

## 公共接口

- `focus_at(TreeAnchor) -> FocusContext`，包含祖先链和有效命令。
- `plan_edit(revision, selection, SemanticEdit) -> DocumentPatch`。
- `apply_patch` 只由 collab transaction adapter 调用。
- `navigate(cursor, Direction/StructuralDirection) -> TreeCursor`。
- `source_lens(node, dialect) -> GeneratedFragment + SourceMap`。
- `plan_reconcile(binding, edited_source) -> ReconcilePlan`。
- `validate(graph) -> Vec<InvariantViolation>`，发布构建前不得有 fatal violation。

## 不变量

- 所有非 tombstone 节点从 document root 可达；NodeId 永不复用。
- 固定槽永远存在，可为空占位节点，避免编辑时结构坍塌。
- TreeCursor 指向合法槽边界或文本相对位置，不指向渲染坐标。
- SourceAuthority 投影节点没有 reversible capability 时只读。
- StructuredAuthority 的 dirty source 未 reconcile 前不改变 graph。
- unwrap 必须定义内容保留策略；无安全策略的节点不提供该命令。

## 失败处理

- 前置条件、槽位或 capability 校验失败时返回结构化错误，不部分应用编辑。
- 没有内容保留策略的 unwrap 不提供命令；调用方请求时明确拒绝。
- `dirty` 生成源码未 reconcile 前不改变 graph；冲突进入预览而非静默取舍。
- SourceAuthority 投影中无可逆 capability 的区域拒绝写操作。
- invariant 校验出现 fatal violation 时阻止发布构建，并指出违规节点。
- tombstone 仍被保留 checkpoint 或活跃设备引用时不得压缩。

## 测试门禁

每种节点 schema、结构导航、空槽、嵌套选区、焦点能力、wrap/unwrap、variant cycle、矩阵行列、Raw 边界、
source lens 与 reconcile。随机 SemanticEdit 后 graph invariant 始终成立。

## 团队源码语言与混合项目合约

管理 ForeignSource 的内容/宏定义/模板作用域、原文资源引用与导入/导出符号，不负责运行组件。
plan_edit/plan_reconcile 返回实际源码写集；修改原文、宏、模板的视觉操作也需团队许可，纯语义正文不受语言门禁。
共享生成草稿只有活动语言可写；私有草稿独立持久化并带原基线。切换后重建 generation，旧草稿不能覆盖新正文。
新增组件 scope、交叉引用、语义命令绕过门禁、草稿重定位和模板冲突测试。

共同要求见 [混合源码与团队编辑](../MIXED_SOURCE_EDITING.md) 与 [ADR 0001](../adr/ADR-0001-mixed-source-team-editing.md)。

## 阶段 0 验证核心的选区合约

验证核心的 `Selection` 同时接受文本端点与槽位端点；文档顺序必须区分槽位编号、子节点插入位置及子树内外，
不能仅按所属节点的前序序号排序。非法字素位置、槽位下标和已脱离节点均拒绝。
`selection_edit::deletion/replacement` 返回应用于同一 revision 的原子编辑计划与存活光标；完整覆盖子树可摘除，
部分结构保留，数学槽位保留可编辑子节点。纯文本复制只连接叶子原文，不表示结构化数学剪贴板。
验证候选通过同步 batch 应用计划；该接口不替代正式 `DocumentPatch` 的 revision 与 CRDT 协议。

## 基础段落接入边界

LocalSession 是单人内存段落接入；直接 apply 仅限此临时适配器，正式共享树仍须经 collab transaction adapter。
详见 [ADR 0024](../adr/ADR-0024-local-paragraph-integration.md)。

## 本地范围替换

`LocalSession::apply` 接受 `BlockEdit::ReplaceRange`，先验证目标、范围顺序、UTF-8 边界、
单块容量与块数，再一次替换跨越的块。非法端点返回 `EditError::InvalidRange`，未知节点返回
`WrongTarget`，超限返回 `Capacity`；失败不修改快照、revision 或动作记录。
首块身份和块种类保留，范围之外的节点不变；换行新建后续块 ID，一次替换只产生一个动作。
该范围基于标记文本，不能冒充完整数学 TreeCursor/槽位语义；见 [ADR 0029](../adr/ADR-0029-direct-page-editing.md)。
