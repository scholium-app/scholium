# 协议设计

## 1. 目标

协议把 UI、core 和服务器从实现细节中隔离。协议只传 DTO，不传 Rust enum 内存布局、CRDT engine 类型、
CST 节点或平台对象。所有请求可关联、可拒绝、可重试；所有异步结果可判断是否过期。

## 2. 通用信封

```text
Envelope {
  protocol_version,
  message_id,
  correlation_id?,
  project_id?,
  actor_id?,
  sent_at?,
  payload_type,
  payload
}
```

`message_id` 用于幂等和调试，不决定顺序。只有 actor sequence、resource revision 或 branch ref CAS 决定
对应领域中的先后。未知可选字段忽略，未知 payload type 返回 UnsupportedMessage。

## 3. UI → core 命令

读命令：OpenProject、OpenResource、ReadRange、QuerySymbols、QueryTimeline、GetBuildStatus、GetPresence。

写命令：

```text
ApplySourceEdits {
  resource_id, base_revision,
  editor_edits[], intent, action_boundary,
  scope, dialect, edit_epoch, permit_id, source_write_set
}
ApplySemanticEdit { document_id, base_revision, tree_selection, edit }
OpenSourceLens { document_id, node_id, dialect }
PlanSourceReconcile { generation_id, edited_source }
ConfirmSourceReconcile { plan_id, expected_revision, resolutions }
ApplyWorkspacePatch { revisions_by_resource, patch }
Undo { branch_id, scope }
Redo { branch_id, scope }
CreateCheckpoint { expected_head, name, description }
ForkBranch { checkpoint_id, new_name }
SwitchBranch { expected_work_head, target_branch }
PlanRevert / ConfirmRevert
PlanCherryPick / ConfirmCherryPick
PlanMerge / ApplyMergeResolution
StartBuild / CancelBuild
RequestSourceDialectSwitch { scope, expected_epoch, target_dialect }
DrainSourceWrites { scope, edit_epoch, accepted_sequences, durable_drafts }
ResolveSourceSwitch { switch_id, expected_epoch, resolution }
SaveSourceDraft / PlanDraftIntegration / ConfirmDraftIntegration
SaveProject / PlanSaveAs / ConfirmSaveAs
PlanExport / ConfirmExport { plan_id, input_snapshot, target, packaging, draft_policy }
ConnectSync / DisconnectSync
```

所有 plan/confirm 两阶段命令的 confirm 必须携带 plan id、基线 heads/revisions 和用户选定 resolution；状态变化
后返回 PlanStale，不在旧计划上盲目执行。

ApplySemanticEdit/ApplyWorkspacePatch 若写入源码、ForeignSource、宏或模板，同样携带实际写集及许可。
不同语言的组合写集不能一次绕过语言门禁；按可恢复步骤执行并经过团队切换。普通纯语义写入无需源码许可。

## 4. core → UI 事件

```text
ResourceOpened { resource, revision, text, capabilities }
SourceChanged { resource_id, from_revision, to_revision, editor_edits, action }
DocumentChanged { document_id, from_revision, to_revision, semantic_delta, action }
FocusChanged { document_id, revision, tree_cursor, ancestors, capabilities }
GenerationChanged { binding_id, generation_id, dialect, state, source_map }
SourceEditControlChanged { scope, active_dialect, edit_epoch, phase, pending_members }
SourcePermitChanged { scope, actor_id, dialect, edit_epoch, permit_id, expires_at }
DraftDurabilityChanged { draft_id, state, base_generation, integration_required }
ExportChanged { export_id, input_snapshot, target, state, report, artifacts? }
DurabilityChanged { action_id, state }
DiagnosticsChanged { resource_id, revision, diagnostics }
SymbolsChanged { index_revision, affected_resources }
BuildChanged { build_id, input_revisions, state, diagnostics, artifact? }
HistoryChanged { branch_id, head, actions/checkpoints delta }
BranchChanged { branch_id, work_head, resources }
PresenceChanged { actor_id, resource_id, anchor/selection?, ttl }
ConflictRaised { conflict_id, plan }
SyncChanged { connection, queued_bytes, acked_sequence }
```

事件 delta 必须携带 from/to revision。前端缺少 from revision 时请求 snapshot，不猜测补丁仍可应用。

## 5. 乐观编辑与 rebase

视觉编辑器与源码编辑器都立即显示本地输入并发送 base revision。core 有三种响应：

- Accepted：返回相同逻辑编辑对应的新 revision/action。
- Rebased：远端变更已先到，返回先应用的 semantic/source delta 与已重定位后的 local edits。
- Rejected：锚点分离、权限或 precondition 失败；返回 authoritative range/snapshot 和原因。

前端维护 pending edit 队列，按 request id 确认；不能收到任一 ResourceChanged 就清空全部 pending。

## 6. client ↔ sync-server 帧

```text
ClientHello { versions, project, branch, actor, permission_epoch, state_vector, capabilities }
ServerHello { version, role, epoch, limits, snapshot_offer?, missing_ranges }
UpdateBatch { first_sequence, updates[], batch_hash }
UpdateAck { through_sequence, durable_at }
RequestRange { actor, sequence_range }
SnapshotOffer/Upload/Accepted
MoveBranchRef { ref, expected_old, new_head }
Presence { counter, resource, anchors, display }
GetSourceEditControl / SourceEditControl
RequestDialectSwitch / SourceSwitchDraining / SourceSwitchCommitted
AcquireSourceWritePermit / SourceWritePermit / SourceWriteFenced
SourceWriteBatch { scope, actor, dialect, edit_epoch, permit_id, source_write_set, updates }
SourceDrainAck { switch_id, actor, accepted_sequences, durable_drafts }
Error { code, retry, details }
```

内容帧可靠且可重试；presence 可丢弃。压缩在协商后启用，解压后大小仍受限。服务器对同一 actor 的序号
空洞请求补包，序号回退且 hash 不同视为身份冲突。

涉及源码的更新必须经 SourceWriteBatch 或等价受门禁通道。服务器按资源/绑定与结构级写集验证真实写入范围，
不能允许普通 UpdateBatch 夹带 ForeignSource 改写。先查已接受包的幂等回执，再对新包验证当前 epoch、许可、
phase 与权限；校验和内容持久化原子完成。切换控制消息可靠且持久化，不能放到 presence 通道。
离线/旧 epoch 的新源码包返回 SourceEpochStale 或 SourcePermitRequired，客户端保留草稿/fork，不能直接重放。
协议方案和最小可信校验由阶段 0 验证；未支持此能力的旧客户端不能写入受保护共享分支。

## 7. 错误模型

错误由稳定 code、category、retryability、用户安全消息和内部 correlation id 构成。正文片段、token、绝对路径
不进入跨网络错误。主要类别：InvalidInput、StaleRevision、Conflict、PermissionDenied、QuotaExceeded、
Unavailable、CorruptData、Unsupported、Internal。

新增稳定错误：SourceEpochStale、SourceDialectMismatch、SourcePermitRequired、SourceSwitchInProgress、
SourceWriteSetMismatch、DraftIntegrationRequired、ForeignDependencyMissing、MixedLayoutConflict、
ReferenceBridgeNonConvergent、ExportUnresolved。返回可重试/需合入/需修复信息，不丢弃原输入。

## 8. 版本兼容

- UI/core 随应用一起发布，但仍保留协议测试，防止开发期前后端漂移。
- sync-server 同时支持当前和前一公开协议版本。
- 新客户端不得上传服务器无法安全保留的新 CRDT/schema 能力。
- 协议降级必须显式返回禁用 capability，UI 不显示不可用命令。
- 每种消息都有 golden encode/decode，敏感字段有日志脱敏测试。
