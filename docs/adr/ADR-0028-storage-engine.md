# ADR 0028：存储引擎选型（SQLite/rusqlite vs redb）

- 状态：Accepted（2026-09-23 按预设失败判据改裁 SQLite；见"裁决"节）
- 日期：2026-09-23
- 影响模块：scholium-storage（未建立）、document、app
- 关联：[ADR 0023](ADR-0023-stage0-feasibility-boundary.md)（阶段 1 门禁 1）、[报告 0008](../spikes/SPK-0008-build-recovery.md)（恢复机制已验证）

## 背景

阶段 1 门禁要求：正式存储开工前比较 SQLite（rusqlite）与 redb，并裁决 WAL/快照/内容寻址
与持久化 schema；实验会话文件不得转正。当前 `LocalSession` 纯内存，关闭即失。

## 候选对比

| 维度 | rusqlite（SQLite） | redb |
|---|---|---|
| 模型 | 关系表 + SQL；schema 演进靠迁移 | 嵌入式 KV（表 → key/value 范围） |
| 事务/WAL | 成熟 WAL、跨进程并发、断电语义有长期实证 | 单写者 MVCC，崩溃安全由 B-tree + 意图 journal 保证 |
| 内容寻址 | 需自建（blob 表 + hash 列 + 索引） | 天然契合：key=hash 的 blob 表 + 前缀扫描 |
| Action 日志 | append 表 + 触发器/应用层保证只追加 | 单表 `push`，天然 append |
| 快照/恢复 | 需要自写快照表 + 版本切换 | 多表 + 版本 key 即快照；恢复=丢弃未提交 key |
| 查询 | SQL：诊断、历史时间线按 actor/时间过滤方便 | 范围扫描够用；复杂过滤在内存投影层做 |
| 依赖 | C 库（bundled sqlite3，~1MB），NATIVE_DEPENDENCIES 需登记 ABI | 纯 Rust，无 C 依赖 |
| WASM 前景 | sqlcipher/wasm 路径复杂 | 纯 Rust 但文件系统假设需适配（见 WASM 计划） |
| 成熟度 | 极高 | 1.x 稳定，社区规模小 |

## 拟议决策

**选 redb**，理由：
1. 存储负载形态简单——Action 日志（append）、内容寻址 blob、命名快照、少量元数据；
   不需要关系查询，schema 演进 = 新表 + 迁移函数，风险低于 SQL 迁移链。
2. 纯 Rust 满足 AGENT.md 原生栈与 WASM 边界（C/C++ 依赖须单独登记验证）。
3. 报告 0008 已验证的恢复语义（WAL 撕裂恢复、外部改动拒绝）在 KV + 只追加日志上
   复现直接；SQLite 的等价实现要多一层胶水。

拟议 schema（首个正式版）：
`actions`（表=ActionLog，key=单调序号，value=JSON 序列化的 DocumentRequest+结果）、
`blobs`（key=内容 hash）、`snapshots`（key=revision，value=DocumentSnapshot JSON）、
`meta`（schema 版本、文档身份）。会话写入路径：动作先落 ActionLog 再投影快照，
崩溃后重放 > 快照 revision 的动作恢复。

## 证据与裁决（2026-09-23，同机 dev profile 最小实现）

两引擎各按拟议 schema 实现最小版并同机对比（Arch Linux / sqlite 3.53.4 / redb 3.1.3）：

| 指标 | SQLite (rusqlite 0.37, WAL+FULL) | redb 3.1.3 |
|---|---|---|
| 10 万动作追加（单事务） | **59–72 ms** | 2 595–2 627 ms |
| 整库读回（快照+日志） | **<1–28 ms** | 244–246 ms |
| SIGKILL 写入中途击杀 | **恢复到某个完整提交**（探针测试） | 未测（已改裁） |

写入差距约 **40×**，远超预设失败判据（p99 写入 >2× 即重裁）。
按本 ADR 预先声明的判据，**裁决改为 SQLite（rusqlite 链接系统 libsqlite3）**：
拟议决策作废，`crates/scholium-storage` 后端已改写为 SQLite（schema 与公共 API 不变：
snapshots/action_log/meta 三表 + `head_revision` 指针；WAL + synchronous=FULL，
保存为低频显式动作，换崩溃/断电安全）。

redb 保留为 WASM 路线的候选记录（纯 Rust、无 C 依赖），浏览器存储适配另行验证；
`head_revision` 指针的教训（低 revision 重存不能按最大 key 取最新）对两引擎同样适用。

证据复现：`cargo test -p scholium-storage`（round-trip/日志收缩/历史读/10 万基准）
与 `cargo test -p scholium-storage --test recovery`（hammer 子进程 SIGKILL 注入）。
libsqlite3 依赖登记见 [NATIVE_DEPENDENCIES](../NATIVE_DEPENDENCIES.md)。

## 后果

建立 `crates/scholium-storage`（首个正式存储）；保存/另存为/强杀恢复随后接入；
SQLite 路径保留本 ADR 作为回退依据。

## 修订（2026-09-24）：保存结果与恢复错误

本地会话的“已保存”只表示目标 SQLite 数据库事务提交成功。数据库打不开时不再写入临时库冒充保存；
启动时已有会话恢复失败会阻止覆盖原文件，并向界面报告错误。读取时缺失 head 快照、损坏的请求身份、
快照 revision 与日志长度不一致均作为恢复错误处理，而非空会话或默认身份。
