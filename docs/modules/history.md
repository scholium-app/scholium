# scholium-history

## 职责

建立用户可理解的 Action、生成补偿动作、维护 checkpoint DAG 与 branch/tag refs、计算时间线、
规划 revert/cherry-pick/merge，并输出冲突计划。

## 非职责

不直接操作 CRDT 库，不写磁盘，不解析格式，不同步网络，不决定 UI 如何展示 diff。

## 内部结构

```text
src/
├── action.rs       Action、group、origin、intent、inverse recipe
├── undo.rs         actor/branch scoped undo/redo planner
├── graph.rs        Change/Checkpoint DAG 与可达性
├── refs.rs         branch/tag 规则
├── revert.rs       任意动作补偿规划
├── cherry_pick.rs  语义重放与 anchored fallback
├── merge.rs        head 集合、共同祖先、merge plan
├── timeline.rs     查询、过滤、分页
└── conflict.rs     conflict kind 与 resolution DTO
```

## 公共接口

- `record_action(ActionDraft, AppliedUpdate) -> Action`。
- `plan_undo(actor, branch, state) -> UndoPlan`，只规划不应用。
- `plan_revert(action, state) -> RevertPlan`，返回 clean/conflicted/non-revertible。
- `create_checkpoint(heads, metadata)`；验证 parent 可达且不存在环。
- `fork(checkpoint, branch_name)`、`move_ref(expected_old, new)` 使用 compare-and-swap。
- `plan_merge(left, right)` 返回 update 集和必须由格式层复核的资源集合。

## Inverse recipe

记录 TextInserted、TextDeleted、SemanticForwardInverse、ResourcesAdded、ResourcesMoved 等 recipe。
recipe 使用 RelativeAnchor 和上下文 hash，不保存可执行闭包或第三方库对象。大删除内容可存 blob hash。

## 不变量

- 历史 append-only；只允许移动 branch ref，不能改写 Action/Checkpoint。
- undo 不能选择其他 actor 的 Action；显式 revert 可以，但需权限和预览。
- compensation 也有普通 ActionId，可再次撤销。
- checkpoint 内容由 parent、资源 heads 和 metadata hash 决定。
- merge checkpoint 至少两个不同父节点；fast-forward 不伪造 merge。

## 失败处理

- 无法生成可靠 inverse 的动作标记为 `non_revertible` 并说明原因，不伪装成功。
- 上下文指纹或 precondition 失败时展示三方预览；用户确认后仍以新 Action 提交。
- branch ref CAS 失败返回冲突，由客户端重新获取 head，不静默覆盖。
- merge 出现语义冲突时建立临时 merge session；只有无冲突才创建双父 checkpoint。
- 中部日志损坏时停止自动恢复并生成恢复报告，保留损坏副本；只允许截断尾部半写记录。
- 重复 revert 与重复 update 幂等；涉及源码的历史操作无当前许可时拒绝或转为可恢复步骤。

## 测试门禁

DAG 属性、无环、共同祖先、多分支 ref CAS、远端穿插 undo、删除后重定位、不可撤动作、重复 revert、
cherry-pick precondition、merge 重启复现。用模型测试与简化参考实现对照随机动作序列。

## 团队源码语言与混合项目合约

记录 ForeignSource/模板/宏写集、原文恢复信息和接受时 epoch，但不把活动语言控制状态放入可回退正文历史。
undo/revert/cherry-pick/merge 若修改源码，按当前活动语言取得新许可；不能复用旧 Action 许可。
两语言组合计划经可恢复步骤和团队切换执行；独立分支可各用一种语言，合并不得覆盖目标分支控制状态。
未合入草稿独立恢复，不冒充共享 Action。新增 epoch 不回退、双语言合并、源码 inverse 与混合引用冲突测试。

共同要求见 [混合源码与团队编辑](../MIXED_SOURCE_EDITING.md) 与 [ADR 0001](../adr/0001-mixed-source-team-editing.md)。
