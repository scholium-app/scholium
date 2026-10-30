# scholium-format

## 职责

阶段 0 source-reconcile 在 Raw 转换前增加保守的片段分隔符检查；不完整片段保持草稿，
正文和历史不变。真实 LaTeX/Typst UI 证据及完整语法解析的边界见[报告 0021](../spikes/SPK-0021-native-edit-acceptance.md)。

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
├── report.rs        Exact/Equivalent/RawPreserved/Degraded/Dropped
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
- 出现 `Dropped` 且用户 policy 未降级时导出失败；无法处理的 Raw 跨方言输出必须降级报告或阻止导出。
- 资源访问超出 `ParseInput` 声明、registry 冲突或 revision 过期时拒绝并返回诊断。
- Copy As 的临时资源必须在返回前清理或由生命周期对象持有。
- 跨格式另存为失败时保留原项目与上次成功产物，不产生半成品目标项目。

## 测试门禁

fake adapters 验证取消、过期结果、registry 冲突、依赖失效、report 完整性、多 MIME 一致性。具体格式
golden 由各 adapter crate 提供，format 负责跨格式组合测试。

## 项目语言合约

格式适配与转换合约供两个内核使用，项目语言在创建/导入时固定；转换 IR 不作为共享文档模型。
跨语言只经一次性转换或导出，逐节点报告 Exact/Equivalent/RawPreserved/Degraded/Dropped。
Raw 同方言无损保留，跨方言降级或阻止导出；混合组件、跨引擎桥接与团队语言切换已推迟。
共同要求见 [项目语言、跨格式转换与文件操作](../MIXED_SOURCE_EDITING.md) 与 [ADR 0041](../adr/ADR-0041-separate-latex-typst-kernels.md)。
