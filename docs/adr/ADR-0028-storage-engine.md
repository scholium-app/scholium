# ADR 0028：存储引擎选型（SQLite/rusqlite vs redb）

- 状态：Proposed（拟议决策；实现轮以真实基准与恢复测试裁决后改 Accepted）
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

## 裁决前必须补的证据（实现轮）

- 基准：10 万 Action 追加、按 revision 读快照、崩溃恢复（SIGKILL 撕裂注入）三项，
  redb 与 rusqlite 各实现最小版并同机对比；不满足则回退本决策。
- `verify-stage0.sh` 式门禁：redb 依赖许可证、平台构建（Linux 先行）。
- 失败判据：redb 任一项显著劣于 rusqlite（恢复不完整或 p99 写入 >2×）即重新裁决。

## 进展（2026-09-23，首个实现）

`crates/scholium-storage` 已按拟议 schema 实现并接入 app（Ctrl+S 保存、启动恢复、
脏状态标题/状态栏）。redb 3.1.3 实测（本机，dev profile，含测试断言）：
**10 万动作追加 2.6 s、整库读回 246 ms**；未提交事务整体不可见（redb MVCC），
保存为单事务。修复点：重存低 revision 需以 `head_revision` 元数据指向最新保存，
不能按最大 key 取快照。rusqlite 对照与 SIGKILL 撕裂注入仍未跑，ADR 维持 Proposed。

## 后果

建立 `crates/scholium-storage`（首个正式存储）；保存/另存为/强杀恢复随后接入；
SQLite 路径保留本 ADR 作为回退依据。
