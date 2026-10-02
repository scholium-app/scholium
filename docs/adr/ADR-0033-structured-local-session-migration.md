# ADR 0033：本地结构身份、会话代际与候选格式迁移

- 状态：Accepted（限定本地候选 v1；不默认替换主程序）
- 日期：2026-10-01；负责人：Codex，按用户授权实施接入计划 A。
- 影响模块：model、document、storage；方向见 [ADR 0032](ADR-0032-typst-edit-kernel.md)。
- 对应任务：[主程序接入计划 A](../plan/TYPST_MAIN_APP_INTEGRATION.md)。

## 背景

页面正文的块 ID 稳定，但 inline 与公式内部位置仍由索引/源串描述。同一个语义 revision
可能在撤销后再次出现。若直接搬入 spike Editor，既有 LocalSession/保存会话与它将形成
两个可写正文；若原地改旧 SQLite JSON，旧写入器可能清除新身份或结构空槽。

## 验证方法

在正式 model/document/storage crate 中验证候选数据与现有兼容接口，不引入 Typst 或 UI
依赖。用同一 LocalSession 的不同 snapshot 类型消费式迁移，检查失败保留原会话、成功
只留下一个权威。分数/文字/拆合操作、Unicode、身份重复、保存重开和请求日志逐项对照。

存储先验证源库 head、日志与历史快照，再在相邻临时文件完成新格式；目标以不覆盖方式
发布。验证原文件不写入、目标存在/损坏源/未知版本/非法节点时停止、旧 writer 的裸 JSON
不能写入新库，以及历史原始 JSON 无损留存。

## 候选方案

1. 给旧字符串旁加一棵可写数学树：产生两个权威，拒绝。
2. 原地升级匿名 inline JSON：失败/旧 writer 风险且旧文件无法独立回退，拒绝首轮采用。
3. 同一 LocalSession 泛型承载 legacy 或 identified 结构；显式迁移到独立候选库：本轮选择。

## 决策

新增无 Typst 类型的 identified 结构：块、inline、数学 Row/Text/Symbol/Fraction/Hole 的
NodeId 全文唯一。分数两个固定槽以拥有者身份和槽语义区分，文字使用叶内 UTF-8 字素边界。
RawMath 仅保留原始 Typst 数学源串，不能同时编辑原串与数学树。首轮不执行/猜测旧公式：
全部旧 Math 原文保留为 RawMath，转换数量和能力限制在迁移报告明确列出。

`LocalSession<Snapshot>` 仍是唯一可写权威；legacy 默认参数维持既有程序接口。迁移验证
后消费旧 session，转成结构 snapshot 类型并保留请求身份日志；失败返回原 session。
结构文字、分数、拆合操作由 document 验证后一次提交。暂用事务规划副本保证错误原子性，
不宣称该核心路径已有全文规模解耦或协作历史。

渲染会话 epoch 是随机身份，恢复/迁移启动新 epoch；布局请求、profile/resource generation
独立于语义 revision。epoch 不写入文档或请求日志。SceneStamp 全字段相等才允许当前采纳，
不能只比较 revision；主程序 worker/撤销接线属于后续接入批次。

候选 SQLite `user_version=1`；snapshot JSON 用显式 format/version envelope 包装 identified
文档。旧裸快照为 legacy v0，默认读写格式不改变。新库的 SQL CHECK 拒绝裸 JSON，即使
旧 writer 忽略 user_version 也不能覆盖结构快照；现代 legacy API 拒绝操作 v1 内容。
未知版本在建表/改变日志模式前拒绝。legacy 的额外字段/表/列/元数据也拒绝迁移，不能丢弃未知数据。候选不作为完整共享 SDG 或永久历史 schema。

迁移不覆盖源库或已有目标。源库在只读事务中取得一致 head、现有请求日志和历史 JSON。
head 只迁移一次并赋 inline/数学身份；匿名旧历史不猜跨版本身份，原始快照归入只读档案，
不生成伪造的过去请求。目标先在同目录临时 SQLite 完成/验证，关闭连接并 fsync 文件，再
以不覆盖的硬链接发布、fsync 目录。发布后目录同步失败可留完整目标但不宣称 durable，
源库始终保留。普通主程序不得自动选择候选库或把迁移档案当当前可写内容。

## 后果

新候选可保存稳定身份与必填 Hole；旧程序默认行为不变，后续主程序候选复用这些类型。
legacy head/request 日志的原子保存条件保留。旧公式只有 Raw 能力，未提供自动数学解析或
结构编辑；类型新增不意味着主程序已经移除回显。树深/节点/文本/JSON 有明确容量上限，
超限拒绝迁移，不能截断正文。

本轮只验证本地动作，单机撤销仍不得冒充 actor/CRDT undo；正式历史与协作格式另立裁决。
硬链接发布需同文件系统，首轮 Linux 验证；不支持的宿主应拒绝而非改用覆盖式复制。
不增加核心依赖许可类别，不修改默认 Typst 依赖或处理任意用户代码。

限定兼容证据见[报告 0053](../spikes/SPK-0053-structured-session-migration.md)：
正式 crate 回归及跨模块 fixture 验证身份/空槽重开和不覆盖发布，不替代主程序窗口验收。

## 替换条件

主程序候选闭环、完整选区/输入法、分页、严格输出、迁移能力与依赖安全通过后，按接入
计划选择默认路径。升级候选格式必须新版本、兼容报告和独立迁移目标，不重解释 v1。
若迁移原子性/旧 writer 门禁无法证明，保留 legacy 默认，候选 API 不进入发行路径。
