# 历史与协作设计

## 1. 三种记录

### CRDT update

机器层增量，负责并发收敛和幂等同步。它可以被压缩，不直接展示给用户。

### Action

用户层动作，例如连续输入一个词、粘贴一段内容、重命名 label、导入目录。Action 保存 actor、
origin、intent、作用域、update 引用、相对锚点和生成 inverse 所需信息，是撤销与时间线的基本单位。

### Checkpoint

版本层节点，保存一个或多个父节点、每个资源的 CRDT state/vector、名称、说明和作者。分支只是指向
checkpoint 的可移动引用；tag 是不可移动引用。

三者生命周期不同：update 可压缩，Action 可归档，checkpoint 必须长期可解析。

## 2. Action 分组

输入不能每个按键占一条时间线，也不能无边界合并。以下事件结束一个动作：

- 超过可配置空闲窗口。
- 光标或选区非连续移动。
- 文件、语法上下文或 intent 改变。
- 粘贴、格式化、补全接受、重构、导入等显式命令。
- 收到会改变当前锚点语义的远端操作。
- 用户手动建立 checkpoint。

IME 的 preedit 不进入 Action；commit 作为一个动作。自动格式化默认与触发它的动作同组，但保留
子步骤，方便在预览中解释。

## 3. 本地撤销

每个 actor、每个 branch 维护独立 undo scope。算法：

1. 从当前 head 反向查找最近一个由当前 actor 创建且尚未被本 actor 撤销的 Action。
2. 用 CRDT 相对锚点在当前状态重定位目标。
3. 验证上下文指纹；若可安全应用，生成 compensation Action。
4. 若目标已经被修改，展示三方预览：动作前、动作后、当前状态。
5. 用户确认冲突方案后仍以新 Action 提交。

redo 是撤销最近 compensation Action。远端 Action 永不自动进入本地 undo scope。

## 4. 任意 revert

任意 revert 比普通 undo 风险更高，必须依赖存档的语义 patch，而不只依赖 CRDT 库 UndoManager。

**引擎已选定（阶段 0）**：Loro 1.16.0（[ADR 0009](adr/ADR-0009-crdt-engine.md)、[报告 0014](spikes/SPK-0014-crdt-engines.md)）。依据是两个决定性能力只有它同时具备：**可移动树**（`LoroTree` 的 `mov`/`mov_to`/`mov_after`/`mov_before` 直接对应包裹/解除/重排）与**能过滤远端输入的内置 UndoManager**（实测撤销本地输入而保留远端输入）。Yrs 无 mov 类 API、Automerge 无 undo API。
两条约束：① 接口仍按操作语义（操作集、因果序、undo 作用域）定义，不把引擎类型暴露到三层历史之外；② 本项**未做性能对比**，阶段 1 首个迭代必须补（10 万动作、编码体积、内存），若 Loro 明显劣于 Yrs 则转 Yrs 并自建移动语义。

- 文本输入保存插入内容与边界 anchors。
- 删除保存被删内容、左右 anchors 与内容 hash。
- 语义重构保存 forward/inverse patch 和所有文件 precondition。
- 导入保存新增资源清单；revert 不删除后来被引用或编辑的资源，转为冲突项。

如果无法生成可靠 inverse，Action 标记为 `non_revertible` 并解释原因，不能伪装成功。

## 5. 分支

分支不是在同一个活动 CRDT 实例中切换状态。每个 branch head 指向 checkpoint：

1. 从最近快照加载 base document。
2. 重放到目标 checkpoint 所需的 update 集。
3. 为该 branch 创建独立活动 document/session。
4. 切换前先持久化当前 branch 的未物化动作。

未 checkpoint 的工作状态由隐藏 work head 保存，防止切换分支丢失内容。

## 6. merge 与 cherry-pick

CRDT update 的集合并集只保证数据结构收敛，不保证 LaTeX/Markdown 语法有效。因此 merge 分两层：

- 结构合并：合并两个 head 缺失的幂等 updates。
- 语义验证：重新解析入口和受影响资源，检查重复 label、断引用、环境破坏和资源路径冲突。

无语义冲突时创建双父 checkpoint。有冲突时建立临时 merge session，解决结果作为 Action 提交，
然后创建双父 checkpoint。

cherry-pick 优先重放 SemanticEdit；没有语义记录时才尝试 anchored text patch。precondition 失败必须预览。

## 7. 日志和快照

每条日志记录具有长度、版本、校验和、project、branch、actor、单调序号和 payload。启动时顺序扫描，
尾部半写记录可截断；中部损坏停止自动恢复并生成恢复报告。

快照触发条件由 update 数量、日志字节和空闲时间共同决定。压缩只能删除所有保留 checkpoint 均不再
引用、且服务器确认所有活跃设备已接收的 update。长期离线设备通过完整快照重新加入。

## 8. 同步协议

握手交换协议版本、project、branch、actor、权限、state vector 和能力集。随后双方发送缺失 update。

- update 以 `(actor, sequence)` 和内容 hash 去重。
- ACK 表示服务器已持久化，不表示其他客户端已应用。
- presence 使用独立有序通道，可丢弃旧消息。
- 客户端离线队列达到上限时继续本地工作，但提示尚未上传的数据量。
- 服务器拒绝未知 project、过期权限、序号回退、超限 payload 和无效签名。

## 9. 权限

- owner：管理成员、删除项目、变更所有权和写入。
- editor：读取、写入、建立 checkpoint/branch；不能管理成员。
- viewer：读取内容与 presence；不能上传 update。

权限变化带 epoch。客户端携带旧 epoch 的离线写入在重连时进入本地 fork，不可上传到已撤权分支，
以避免丢失用户离线工作。

## 10. 必测场景

- A 输入、B 插入其中、A undo：只消除 A 的内容。
- A 删除、B 在删除边界输入、A revert：B 的输入保留。
- 两端离线重命名同一 label：收敛后产生语义冲突。
- 从旧 checkpoint 分叉，两边修改后 merge：双父节点可复现。
- update 重复、乱序、断包、服务器重启：结果不重复、不丢失。
- actor 被撤权后离线编辑：本地内容进入 fork，服务器主分支不改变。

## 11. 团队源码语言控制

同一项目分支只有一个 ActiveSourceDialect；同语言可多人写入，不能让不同客户端独立开放 LaTeX 与 Typst 编辑。
控制记录和 edit_epoch 属于持久化协调平面，不是正文 CRDT，也不随 checkpoint、undo、revert 或 merge 回退。
切换经冻结、冲刷、隔离旧许可、建立新基线、递增 epoch，详细状态机见
[混合源码与团队编辑](MIXED_SOURCE_EDITING.md)。

Action 记录源码写集和接受时 epoch 以供审计。撤销、revert、cherry-pick、合并若改动 ForeignSource、宏、模板
或直接源码，执行时必须取得当前活动语言许可；历史许可不可复用。涉及两种语言的计划按可恢复步骤处理，
不能一次覆盖门禁。纯语义补偿不需要把两种派生 generator 更新算作两次源码写入。

共享分支断线后源码输入进入本地草稿/fork，重连先同步控制记录，再请求合入。草稿保留原 generation 和源文本，
拒绝旧 epoch 不等于删除用户输入。独立分支可选择不同语言；合并正文不合并活动语言/许可，按当前控制状态校验。
首发项目分支级范围覆盖共享模板和资源，避免同文档多入口绕过规则。

新增测试：同语言多人并发、两人竞争切换、IME/草稿 drain、掉线和服务重启隔离、迟到旧包、重复已确认包、
外部文件写入与语义命令绕过、旧历史重放、跨语言分支合并和双向引用冲突。
