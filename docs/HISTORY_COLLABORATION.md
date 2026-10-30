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

## 11. 项目语言与协作历史

项目语言在创建/导入时固定为 Typst 或 LaTeX，同语言多人通过树/文本 CRDT 协作。
Typst 原生项目记录语义动作，源码项目记录权威文本动作；undo/revert/cherry-pick/merge 按当前权限、
相对锚点与 precondition 验证。分支继承项目语言，跨语言只经转换建立新项目及兼容报告。

离线输入保留同步状态与原基线；草稿冲突先预览，不以重新生成的内容覆盖用户输入。
权限撤销仍按第 9 节处理，权限 epoch 不随历史回退。混合组件与团队语言切换已按
[ADR 0041](adr/ADR-0041-separate-latex-typst-kernels.md) 推迟，文件操作见
[项目语言、跨格式转换与文件操作](MIXED_SOURCE_EDITING.md)。

测试覆盖同语言多人并发、离线重连、重复已确认包、源码 inverse、草稿恢复与转换后项目历史独立性。
