# ADR 0059：SDG 会话事务、初始化与旧权威退役

- 状态：Accepted
- 日期：2026-11-08
- 影响模块：session、scholium-sdg、storage 的只读迁移接口
- 关联：[ADR 0053](ADR-0053-loro-sdg-action-mapping.md)、[ADR 0033](ADR-0033-structured-local-session-migration.md)、
  [ADR 0049](ADR-0049-formal-project-storage.md)、[ADR 0051](ADR-0051-project-files-and-session-retirement.md)

## 决定

`EditorCore` 持有唯一可提交的 `Replica`。`LocalSession` 只规划一次命令和提供只读投影，不能独立保存。
`run` 在全部结构规划成功后，把最终效果、当前共享 revision、新 RequestId 和 user origin 交给
`apply_effects`；多次结构规划只生成一个 `SCSDGA01` Action。发布前重建 planner，debug/tests 比对完整
identified snapshot 与 Replica projection。投影修复通过 `shared_repairs` 明确暴露，不把修复当无损导出。

普通命令、Undo、Redo 和临时槽清理都走同一追加通道。复合命令内部的临时槽撤销与后续编辑合并为一个
Action/update；中间 receipts 不对外发布。UI 分组控制 adapter 的 UndoManager group，不合并 durable Actions。
历史条目中的快照仅作端点/兼容测试记录，正文撤销使用 actor UndoManager，保留远端输入。
Undo/Redo 推进共享 revision，不能再通过 revision 回退识别历史位置。

## 原子性与恢复

命令或外层抽象 Input 拒绝/恐慌时，同时恢复 planner、端点、UI 分组和 Replica 的 engine/receipts/undo/redo。
普通 wire snapshot 不含设备 UndoManager，不能单独用于内存回滚。设备内存 journal 保存最初 snapshot 和
本会话接受的本地、远端、分组及补偿步骤；seed 在 actor attach 时建立，首次 checkpoint 之前的编辑也保留。
成功路径只记录步骤，失败时按原 actor/Action 身份确定性重建。
该 journal 不是产品历史、不是第二份正文 authority、不落盘；正常 restart 的撤销栈为空。

持久化在内存事务成功后调用 `ProjectStore::append_action(ActionRecord::new(Action::encode()))`。
Input 包含的命令先收集队列，直到外层 Input 成功才写库。提交失败保留已接纳内存输入和队列，明确显示
persistence error、维持 dirty；显式 save 重试。只有 durable sequence 返回才移除队首，save 成功才更新保存基线。
`SCSDGS01` checkpoint 只覆盖已提交的 sequence，恢复先 import snapshot，再 receive 后续 Actions。

首个 `system/initialize-v1` Action 包含从空 version vector 导出的完整识别树，而非 JSON 状态；
它本身可以恢复初始 Replica。初始化 Action 已提交而首个 snapshot 未提交时，重开从这个 Action 恢复，
不创建不同身份的空白文档。每次重开分配新的随机非零 peer，避免恢复后误用另一个 writer 的 actor。
无首个 snapshot 时，初始化 Action 的投影仍是保存基线，其后未显式保存的 Actions 保持 dirty。

## 稳定身份的删除

本地删除把原 TreeID 的 `active` 置 false，保留 metadata/text 容器和树身份；inactive ancestor 隐藏后代。
物理删除容器后让引擎 Undo 重建，会产生重复 NodeId，不能用于这一映射。重新显现历史节点复用原 TreeID，
不对同一 NodeId 创建第二个 metadata 容器；用户变化和 tombstone 都属于可补偿操作。
已有 wire v1、底层旧 tree tombstone 的读取、Action magic 和 SQLite 外层 schema 不变。
不提供 GC，也不以重新引入一个隐藏身份静默覆盖活跃节点。

## 一次迁移与保留

旧 `scholium.temporary.native` version 1 payload 先严格验证文档与请求日志，恢复其最后完整状态。
随后追加 `initialize-v1` Action 和 SDG snapshot；旧 Actions/snapshots 永不删除或覆盖。死在任何提交边界时，
仍能从旧 head 或新初始化 Action 继续。未知旧 envelope/新 magic 版本拒绝，不猜测版本。

`EditorCore::open(path)` 将实验 SQLite 会话作为只读导入源，新 authority 位于相邻的
`path.with_extension("sdg-project")` 正式项目。源文件永不写入；已发布但没有 head 的目标可恢复迁移，
已有 durable head 后只读新项目。原来的 `SessionStore` 写 API 仅供兼容工具与旧格式 fixture，不能成为编辑 authority。
导入保留当前 NodeId/文字/Raw，不从旧快照栈伪造 CRDT 操作历史。

显式 `restore_document` 验证旧请求日志；同文档替换追加 system Action，不能作为 undo。
更换 DocumentId 只允许刚初始化的空私有会话，追加可恢复初始化；非空项目拒绝身份替换。
另存为创建独立项目与 Replica，原项目及资源保留。

## 限制

journal 和全文 planner/projection 的成本随会话/文档增长；冷路径回滚需要重放设备 journal。
16 ms 输入预算按 [测试规范](../TESTING.md#6-性能预算) 沿用，不把小文档测量外推到大型项目。
网络、协作 UI、权限协议、持久化 undo 栈、产品 checkpoint DAG 和 LaTeX source authority 不在本决定范围。
