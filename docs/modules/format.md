# scholium-format

## 职责

定义 FormatAdapter、调度解析与增量重解析、统一符号/依赖/诊断、管理 CST/SDG 到转换 IR 的投影、
目标生成、生成源码 reconcile、导出报告、Copy As 和 capability registry。

## 非职责

不实现具体语法，不直接写文件，不启动 TeX/Pandoc 等进程，不持有 CRDT，不决定 UI 布局。

## 内部结构

```text
src/
├── adapter.rs       trait 与输入输出 DTO
├── registry.rs      extension/MIME/shebang 到 adapter
├── parse_service.rs revision-aware task 与取消
├── symbols.rs       跨格式 SymbolIndex 合并
├── dependencies.rs  ResourceDependency graph
├── projection.rs    CST → IR orchestration
├── generate.rs      IR → target orchestration
├── reconcile.rs     generated source → SemanticPatch plan
├── report.rs        Exact/Equivalent/Degraded/Dropped
└── clipboard.rs     multi-MIME Copy As payload
```

## 公共接口

- 注册静态内建 adapter；首发不加载任意动态插件。
- `parse(resource, revision, text)` 与 `reparse(previous, edits)`。
- `workspace_index(changed_resources)` 增量更新符号和依赖图。
- `lower_semantic_edit(resource, revision, edit) -> TextPatch`。
- `project(selection) -> Projection { ir, coverage, diagnostics }`。
- `generate(target, ir, policy) -> artifact + ConversionReport`。
- `reconcile(generation, edited_source) -> ReconcilePlan`。
- `copy_as(targets, selection) -> ClipboardPayload + report`。

## 调度

同一资源新 revision 到达时取消未开始任务；运行中的解析可完成，但结果 revision 不匹配则丢弃。
依赖图变化只使下游入口失效。adapter panic 被隔离为内部错误，不能杀死 project session。

## 不变量

- 普通保存不调用 `generate`。
- adapter 不读取 ParseInput 未声明的资源。
- conversion report 覆盖每个输入节点及未投影原文/依赖，每项分别记录保真度、执行路径、编辑性与可移植性。
- Dropped 默认使导出失败；只有用户 policy 可降级为警告。
- Copy As 无副作用，临时资源在返回前清理或交由生命周期对象持有。

## 失败处理

- adapter panic 被隔离为内部错误，不终止 project session。
- 未投影节点、依赖或 Raw 未进入报告时视为门禁失败，不允许导出继续。
- 出现 `Dropped` 且用户 policy 未降级时导出失败；`Unresolved` 组件阻止正式输出。
- 资源访问超出 `ParseInput` 声明、registry 冲突或 revision 过期时拒绝并返回诊断。
- Copy As 的临时资源必须在返回前清理或由生命周期对象持有。
- 跨格式另存为失败时保留原项目与上次成功产物，不产生半成品目标项目。

## 测试门禁

fake adapters 验证取消、过期结果、registry 冲突、依赖失效、report 完整性、多 MIME 一致性。具体格式
golden 由各 adapter crate 提供，format 负责跨格式组合测试。

## 团队源码语言与混合项目合约

解析、生成和转换同时支持 ForeignSource 绑定与不可变组件快照。纯格式适配器输出组件/资源/符号桥接计划，
不调用编译器。conversion report 覆盖原始输入以及宏、模板、依赖，不能只统计成功进入 IR 的节点。
Raw 保留和 ForeignRendered 执行分开；另报目标编辑性、工具链依赖及 unresolved。普通保存不跨格式生成，
跨格式另存为和导出调用 generate，但不改变团队活动语言。新增双目标混合包和丢失覆盖率测试。

共同要求见 [混合源码与团队编辑](../MIXED_SOURCE_EDITING.md) 与 [ADR 0001](../adr/0001-mixed-source-team-editing.md)。
