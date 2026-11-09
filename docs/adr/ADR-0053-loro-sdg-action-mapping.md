# ADR 0053：结构 SDG 的 Loro 映射、Action v1 与确定性修复

- 状态：Accepted（adapter 接入 Typst 会话，事务与迁移补充见 ADR 0059）
- 日期：2026-11-03
- 影响模块：scholium-sdg、后续协作/历史与会话持久化
- 关联：[ADR 0009](ADR-0009-crdt-engine.md)、[ADR 0033](ADR-0033-structured-local-session-migration.md)、
  [ADR 0042](ADR-0042-session-editing-core.md)、[ADR 0049](ADR-0049-formal-project-storage.md)

## 问题与决定

最新稳定 Loro 1.17.1 通过协调者预先规定的增量/离线门槛，版本与完整数据见 ADR 0009。
`scholium-sdg` 作为层 2 的独立权威 adapter，只依赖 model/document 与无 UI 的引擎/编码库。
Yrs 仅用于基准。未接入会话时，不另建第二份持续可写正文。

`Replica::edit` 消费既有 `StructuralEdit`，在临时、revision=0 的 `LocalSession` 上规划，
消费规划副本后将完整效果降成 Loro 操作。`apply_effects` 接受外部规划结果、预期 revision、
请求身份和 origin；拒绝过期结果、身份复用与无效结构。`document`/`projection` 返回只读值。
这不是把整个 JSON 文档写进一个 LWW 寄存器：只变更各节点属性/字符、树位置和存活状态。
当前效果比较与校验会遍历全文，并在 fork 上预演；引擎基准不证明此 adapter 的逐键延迟。

## 稳定身份与结构

- 文档只有一个 `sdg` movable tree。每个原有块、inline、MathNode 对应一个 TreeID；
  immutable `id` 元数据保存 NodeId 的 canonical JSON UUID 字符串；双向索引检查全局唯一，包含 tombstone。
  保留节点经移动、撤销、redo、合并和快照后不换 TreeID。`TreeIdentity` 是独立 DTO，不暴露引擎类型。
- 元数据的 `shape` 是显式 tag 的节点描述（块 kind、文字 style、数学种类与符号/定界符），
  `slot` 是 content/root/children 或 numerator/denominator/base/sub/sup/radicand/index/body。
  sibling 顺序由 movable tree 的 fractional index 承担。每节点独立的 LoroText 保存 literal text/RawMath。
  文本差量保留未改字符身份；Unicode 位置由 Loro scalar API 处理，结构规划仍使用 UTF-8 字素边界。
- `schema` map 保存 document UUID、version="1" 与全空文档的备用 block 身份。
  节点元数据另有 `active`、`reserved` 和可选 `fallback`。块备用 Text、数学 inline 备用 Hole，
  以及文档备用 paragraph/Text 都是在引擎中真实出生的节点；修复不会生成无 TreeID 的临时身份。
- 备用身份取 SHA-256 域分离字符串 `scholium-sdg-reserve-v1/{role}/{canonical-owner-JSON}` 的前 16 bytes。
  初始 role 为 document/fallback；补充备用节点和旧节点首次新增备用使用包含 Action RequestId 的 role；文档备用补充
  再加 document/ 前缀。身份碰撞/重复由索引拒绝，不猜测对应关系。model 仅增加 NodeId byte 往返方法。

新节点出生、其不可变身份/容器、备用补充使用 `system/identity` commit，排除在 undo 之外。
随后 activation、用户属性/字符、move/delete 使用用户 origin，一次命令一个 undo checkpoint。
同一 Action 的 delta 包含出生与用户 commit，不能把引擎 commit 数当 Action 数。
这样 undo 不销毁身份/文字容器，redo 不会复制 NodeId 到新的 TreeID。

备用节点初次进入本地效果时，保留同一 NodeId/TreeID、转为普通节点，并给原 owner 补充新的备用节点；
补充和引用更新也属于 undo 排除的出生阶段。它可以继续被包裹或跨父节点移动，原容器以后再次为空时
不会与已移动节点共用同一身份。未被使用的备用节点在底层 root 隐藏；投影修复将其放到 owner 的必填位置，
第一次真实编辑再物化 parent。备用节点本身不作为用户删除目标物理删除。

## Action 与 snapshot 的永久 envelope v1

编码均为：8-byte magic + 4-byte big-endian JSON-header-length + 4-byte big-endian payload-length +
精确 UTF-8 JSON header + 原始 Loro binary payload。拒绝长度溢出、未知版本、未知 header 字段、
未知 origin、无效 actor 与多余尾字节。v1 绑定本 ADR 的 Loro 1.17.1 编码；引擎 wire 不兼容升级须新版本与迁移。

| 内容 | Action | snapshot |
|---|---|---|
| magic/version | `SCSDGA01` | `SCSDGS01` |
| header | document、id（RequestId）、actor（非零 u64 peer）、origin | document、base_revision、receipts、pending |
| origin tag | user/system（name，1–256 UTF-8 bytes，无控制字符）、undo、redo | 不适用 |
| payload | 此命令前 version vector 之后、仅当前 actor 产生的 Loro updates | 完整 Loro Snapshot |
| receipts | 外部 Action 身份，不能由引擎 op 数代替 | canonical Action ID → SHA-256(完整编码 Action) |
| pending | 缺前驱时由接收端保留 | canonical ID → 完整编码 Action bytes |

Action 身份与字节一同去重：相同 ID/字节重复接收无效果，不同字节重用 ID 拒绝。
接收检查文档、peer 元数据/非空变更、codec 和投影；在 fork 中验证后才导入 live authority。
原始 Loro Snapshot 不保留尚缺依赖的输入，故 adapter 必须单独保存 pending，restore 时重新喂给引擎。
新依赖到达后移除已完成项。`pending_actions` 暴露缺口；不能仅根据 semantic revision 判断同步完成。

revision = 初始 base_revision + 已接纳的唯一 Action 数（包括 pending、undo、redo），与到达顺序无关。
不宣称它是 CRDT clock 或产品 checkpoint；Action ID 也不是数据库 DurableSequence。
`Action::encode()` bytes 可以直接交给 ADR 0049 的 ActionRecord payload；snapshot 必须关联已提交的存储边界，
恢复时加载该 snapshot 并通过 `receive` 重放后续 Action。

边界：JSON header 32 MiB、engine payload 64 MiB、pending 编码 Action 合计 1 MiB、最多 100000 个 Action receipts、
最多 400000 个引擎节点（含出生、备用与 tombstone）。超限拒绝且保留已接受数据；无自动截断/压缩/GC。
引擎 payload 的大小检查不是恶意解码器的隔离沙箱；权限、可信 peer 分配和不可信输入隔离属于后续接入边界。

## 并发与投影修复

修复只依赖合并后的树、属性、顺序和备用身份；不按本地到达顺序发补偿写入。
修复报告按 NodeId bytes/reason 排序；原始内容留在引擎历史，未报告的静默结构丢弃不构成成功。

| 冲突 | 投影规则 |
|---|---|
| 同段/同叶插字 | 由 Loro Text 合并；空 MathText 变同 ID Hole，Hole 留有并发文字时投影为同 ID MathText |
| 同节点移动 | Loro 决定唯一 parent 和 sibling 顺序，应用层不另写父指针 LWW |
| wrap 与 delete | inactive tombstone（兼容旧 tree tombstone）隐藏被删除子树；缺失固定槽的 wrapper 退化为保留现有合法数学子节点的 Row，空则 Hole |
| delete 与远端插子节点 | 删除祖先使该子树不可见；远端操作仍留在历史，不将孩子提升到其它非法 parent |
| 固定槽缺失/重复/未知 | 容器退化为同 ID Row/Hole，保留合法数学孩子的 tree 顺序 |
| 一个 inline 多个数学 root | 按 tree 顺序选择第一个 root，报告其余活跃节点；不拼装含重复身份的虚构根 |
| 空数学 inline / 空 block / 全空文档 | 使用 owner 已出生的备用 Hole / Text / paragraph，报告 EmptyContainer |
| 非法 parent/节点种类 | 不投影到错误的层级，报告活跃但未使用的节点；保留原始状态 |
| Equation 不再只有一个 Math inline | 降为同 ID Paragraph，保留合法 inline 并报告 |
| 并发造成深度/容量超限 | 深度边界变同 ID Hole；按树顺序限额投影节点/块，文字按 UTF-8 边界保留前缀；报告 Capacity/Depth，原值不删除 |

本地效果预演必须等于规划后的 blocks；不会把不匹配的效果冒充成功。
下一次本地命令可将有效投影的修复结构一同物化；undo 后原始冲突可能重新显露，但投影仍满足模型验证。
多 root 的可见选择不承诺保留两边所有结构意图，报告必须交给将来的冲突 UI，不能直接用于无提示发布。

## Actor undo 与恢复

每个设备 writer 使用不同非零 peer；不能复制 actor 到另一个并发 writer。UndoManager 只跟踪本 peer 的
本地输入，排除 system/ 和 remote/ origin；逐命令 Action 不合并；会话 UI 可显式组成 undo group，最多保留 1000 个设备 undo item。
undo/redo 生成新操作和新 Action，不回退共享快照；远端文字、原有子树身份得以保留。
过期/隐藏目标的本地请求拒绝，不修改权威。引擎消费但未生成操作的 undo item 返回 None，不误撤更早动作。

snapshot 保留 engine 操作身份、备用映射、receipt 和 pending；恢复可继续编辑/合并。
UndoManager 栈是设备会话状态，恢复后为空；本 ADR 不把其栈伪装为持久化 Action DAG 或跨重启产品撤销历史。

## 验证与后续接口

测试覆盖所有现存节点种类、Unicode、并发结构冲突、同叶插字、远端穿插 undo、wrap undo、redo 身份、
备用补充后跨父移动、容量修复、snapshot+pending/乱序/重复 Action 重放，以及固定种子
`0x5eed0041..0x5eed004a` 的 10 轮三 actor、每轮 120 步随机交错（含 undo/redo/中途恢复）。
实测与证据索引见 [T41 日志](../log/2026-11-03-loro-sdg-adapter.md)，门禁保留
[最终命令/退出码](../log/evidence/2026-11-03-loro-sdg-adapter/final-gates.json)和
[最终测试输出](../log/evidence/2026-11-03-loro-sdg-adapter/final-tests.log)。

后续 session 在 `EditorCore::run`/成功的 `apply_joined` 原子命令边界用 adapter 替换本地权威，
不能让 LocalSession 与 Replica 持续双向可写。先完成规划与 Action，调用
`ProjectStore::append_action` 取得 durable sequence 后才发布成功；存储失败回滚必须保持 actor/undo/receipt 一致，
不能以恢复旧快照假装协作 undo。`save_snapshot(sequence, bytes)` 保存已提交 Action 边界，启动时重放后续 Action。
本批不修改 session/app/storage，也不定义网络、DAG、权限或冲突 UI。

## 会话接入补充

[ADR 0059](ADR-0059-sdg-session-durability.md) 定义可恢复初始化 Action、命令/外层 Input 的原子边界、
设备 journal 回滚、UI undo group、保留 TreeID 的 inactive 删除与旧 native/session 一次迁移。
Typst 会话只持久化 adapter authority，恢复由 snapshot + 后续 Actions 重建 LocalSession 投影；
设备 undo 栈仍不跨重启持久化。本 ADR 的 Loro 1.17.1 裁决、wire v1 与限额保持不变。
