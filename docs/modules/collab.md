# scholium-collab

## 职责

封装选定 CRDT，实现共享语义树、共享文本、资源目录、Tree/Text 相对位置、state vector、update 编解码、
origin-aware transaction、语义/文本 delta 和 presence DTO。

## 非职责

不定义产品版本图，不保存 Action 文案，不访问服务器数据库，不解析格式，不管理账号。

## 内部结构

```text
src/
├── document.rs    SharedTree/SharedText/SharedDocument
├── transaction.rs actor + origin + intent group
├── position.rs    TreeAnchor/TextRange ↔ engine relative position
├── update.rs      versioned opaque update wrapper
├── state.rs       state vector、snapshot import/export
├── diff.rs        CRDT event → ordered TextEdit
├── awareness.rs   cursor/selection/presence
└── engine/        具体 CRDT adapter，禁止类型外泄
```

## 公共接口

- `create/load/snapshot` 共享文档。
- `apply_semantic(DocumentPatch, context)` 与 `apply_text(TextPatch, context)`；同一调用原子提交。
- `apply_remote(update) -> ApplyOutcome`；重复 update 返回 AlreadyKnown。
- `encode_diff(remote_state_vector)` 只生成对端缺失增量。
- `anchor_at` / `resolve_anchor` 明确 affinity 和 detached 结果。
- `observe()` 输出带 revision 的最小文本编辑，不暴露 engine event。

## 文档 schema

StructuredAuthority 使用共享节点 map、children sequence 和文本 leaves；SourceAuthority 每个文本 Resource
使用独立共享文本。项目目录使用 ResourceId 为 key 的共享 map；正文 update 不依赖路径。二进制 blob
只同步 hash 与元数据，内容走独立 blob 通道。

## 不变量

- 相同 update 任意顺序、任意次数应用后状态相同。
- 一个 actor id 不跨设备克隆；恢复设备身份时生成新 actor 并保留 user identity 关联。
- CRDT index 不越过 crate 边界；外部只见 EditorRange/RelativeAnchor。
- update decoder 有大小和递归限制，失败不改变当前文档。
- presence 不写入 document，不参与 checkpoint。

## 技术验证

按 Loro → Yrs → Automerge 的顺序验证，候选必须比较 Unicode 位置、批量编辑、origin undo、快照大小、10 万次 update、线程模型、
原生 Rust 线程/消息集成、可选 FFI 和长期维护；不以 JS/WASM 桥接作为桌面必要依赖。选择写 ADR；engine adapter 保留替换可能。

`scholium-sdg` 是层 2 的结构权威 adapter，锁定 Loro 1.17.1。`Replica::new/restore` 创建单一权威；
`edit(StructuralEdit, name)` 或 `apply_effects(request, origin, expected_revision, document)` 先规划/预演再写入。
`document/projection` 只返回值，后者携带确定性修复报告；`tree_identity` 提供稳定映射 DTO。
`receive(Action)` 校验 envelope、文档、peer 与结构后幂等导入，缺因果前驱的 Action 由 pending 保留；
`pending_actions` 表示同步缺口。`snapshot` 保留引擎身份、receipt hashes 与 pending，恢复可继续合并。

结构使用 movable tree + 属性 + 每节点独立 LoroText，存活 NodeId 不因包裹、移动或 redo 更换树身份。
新身份/备用补充排除于 undo；用户 activation、属性、文字及移动按一个命令分组。
`undo/redo` 使用本 actor 的 Loro UndoManager 并生成新 Action，system/remote origin 不进入本地栈。
设备会话最多保留 1000 步，快照恢复后栈为空；不冒充持久化产品历史。

并发修复依赖最终树状态：删除祖先隐藏后代；非法类型留在历史并报告；缺失固定槽退为同 ID Row/Hole；
多数学根按树顺序选择首个并报告其它根；空 inline/block/document 使用真实备用节点。
备用首次编辑保留原身份并补充下一备用，允许后续跨父移动；容量/深度修复明确报告且不删除原值。
修复投影不能作为没有冲突提示的发布成功。格式、限额及完整规则见
[ADR 0053](../adr/ADR-0053-loro-sdg-action-mapping.md)。

四版本基准及继续使用 Loro 的判据见 [ADR 0009](../adr/ADR-0009-crdt-engine.md)。
引擎增量接收预算不包含 adapter 全文规划、投影、UI 或存储。
benchmark 中的 parent/order 模型不作为 Yrs 并发移动正确性合约，也不是生产替代 adapter。

## 失败处理

- update decoder 遇到超限、递归过深或畸形输入时拒绝本次更新，且不改变当前文档。
- 重复 update 返回 `AlreadyKnown`；乱序、分片和重放按幂等应用。
- anchor 无法重定位时返回 `Detached`，由调用方决定提示、近似定位或取消。
- 未通过当前项目权限与 patch 前置条件校验的写入在进入共享 CRDT 前被拒绝。
- 远端未经协调者接受的更新不应用；engine 类型不得越过 crate 边界。

## 测试门禁

双端/三端随机收敛、乱序/重复/分片 update、删除区 anchor、emoji/组合字符、快照往返、恶意 decoder、
大文档延迟和内存。属性测试失败种子长期保留。

## 项目语言合约

文本与树 CRDT 属于共享底座，为创建/导入时语言固定的项目提供同语言多人协作。
Typst 结构项目同步语义树，源码项目同步权威文本；离线输入、幂等更新与权限撤销仍按现有规则处理。
跨语言只经转换建立新项目；混合组件与团队语言切换已推迟。
共同要求见 [项目语言、跨格式转换与文件操作](../MIXED_SOURCE_EDITING.md) 与 [ADR 0041](../adr/ADR-0041-separate-latex-typst-kernels.md)。

源码权威双副本实验的已接受文本投影、未提交草稿与追加撤销约束见
[ADR 0020](../adr/ADR-0020-native-source-replicas.md)；其中语言协调部分已按 ADR 0041 推迟。
脏草稿遇到远端变更不得整份覆盖，IME 输入仍由原编辑器完成。

## WASM 互通

浏览器与原生共享 schema、相对锚点语义和版本化更新协议，验证双端快照/增量互通、撤销与权限撤销后的重连；引擎 WASM 支持必须实际编译运行。具体候选见[全栈清单](../plan/TECH_STACK.md)。
