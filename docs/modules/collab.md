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

## 失败处理

- update decoder 遇到超限、递归过深或畸形输入时拒绝本次更新，且不改变当前文档。
- 重复 update 返回 `AlreadyKnown`；乱序、分片和重放按幂等应用。
- anchor 无法重定位时返回 `Detached`，由调用方决定提示、近似定位或取消。
- 未取得当前语言许可的 `apply_text` 或含源码写集的 `apply_semantic` 在进入共享 CRDT 前被拒绝。
- 远端未经协调者接受的更新不应用；engine 类型不得越过 crate 边界。

## 测试门禁

双端/三端随机收敛、乱序/重复/分片 update、删除区 anchor、emoji/组合字符、快照往返、恶意 decoder、
大文档延迟和内存。属性测试失败种子长期保留。

## 团队源码语言与混合项目合约

正文 CRDT 外增加协调接口的适配：scope = ProjectId + BranchId，持久 active_dialect/epoch 与许可隔离。
同语言多 actor 可写，不能使用单人排他锁代替；语言选择不能用 CRDT LWW 或 presence 解决。
apply_text、包含源码写集的 apply_semantic 和共享生成草稿都先经许可门禁；远端只应用协调者已接受的更新。
服务端使用受限结构校验接口核对实际写集与资源/绑定，避免 opaque updates 夹带异语言源码；无需解析语言语义。
该接口的最小可信实现须通过阶段 0 验证后才确定 CRDT 选型。离线无许可的源码变化停留草稿/fork。
新增 drain 屏障、旧 epoch、同语言并发、恶意结构写集、重连和已确认重复包的模型测试。

共同要求见 [混合源码与团队编辑](../MIXED_SOURCE_EDITING.md) 与 [ADR 0001](../adr/0001-mixed-source-team-editing.md)。

## WASM 互通

浏览器与原生共享 schema、相对锚点语义和版本化更新协议，验证双端快照/增量互通、撤销与重连 epoch；引擎 WASM 支持必须实际编译运行。具体候选见[全栈清单](../TECH_STACK.md)。
