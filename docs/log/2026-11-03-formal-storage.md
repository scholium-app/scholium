# 2026-11-03：正式项目存储与恢复骨架

## 问题与范围

T35 要求把阶段 0 的批量写入结论补成真实交互持久化负载证据，并裁决正式项目库的布局与恢复边界。
本批在 `scholium-wt1`、`ation_ciger/feat/formal-storage` 上从最新 origin/main 开始；没有修改
SessionStore 的 legacy/structured 实现，没有接入 app/session，没有新 crate、watcher、历史 refs 或浏览器存储。

## 实现

新增 `scholium_storage::project`：项目锁、SQLite 永久 schema v1、typed errors、Action/update 单事务、
不可变快照、独立源码草稿、SHA-256 blob 去重、revision/manifest 物化意图与显式恢复。
源码采用同目录独占临时文件 + fsync + rename + fsync 目录，继承现有权限；旧 revision 不读盘，
content hash 冲突不覆盖。源码访问使用 cap-std capability 目录，拒绝 symlink 逃逸。

新库先在相邻临时库构建/验证/关闭/fsync，不覆盖发布；异常临时库与未知/损坏 schema 拒绝。
完整 schema、Action 连续性/自增位置、引用及所有持久化 payload hash 在 writer 配置前验证。
open/recover 只产生计划，显式继续重新检查 old/new/外部 hash；异常临时文件保留并列出，不自动删除。
SQLite 连接先关闭、OS 锁后释放。新依赖 sha2/cap-std 均为 Rust，不新增 C/C++ 编译项；
系统 SQLite 的 ABI/所有权/线程/销毁规则沿用现有原生依赖登记，redb 只在 dev-dependencies。

规范更新位于 [storage](../modules/storage.md)，正式裁决与边界见 [ADR 0049](../adr/ADR-0049-formal-project-storage.md)。

## 基准方法、数据与裁决

复现：

```sh
cargo test --locked -p scholium-storage --test formal_benchmark compare_formal_durable_workloads -- --ignored --nocapture
```

测试代码在 `crates/scholium-storage/tests/formal_benchmark.rs`，常规 CI 不运行基准。
每项 1000 次独立提交；SQLite WAL/FULL 与 redb Immediate。Action/附件为 1 KiB；
草稿对同一资源覆盖 1 KiB / 100 KiB / 1 MiB；唯一附件与重复附件分别测量。
计时包含 SHA-256/envelope/事务提交，排除 payload 生成；两个引擎的 blob 都检查已存在内容以防碰撞。
redb 的 Action/草稿直接 insert，不人为增加读回/复制旧值。

冷打开定义为数据库引擎缓存冷，不清 OS 页缓存：每个负载先在子进程写入后 SIGKILL，
每轮计时前恢复并 fsync 原始崩溃主文件及 WAL，随后新句柄打开/恢复、完整扫描及 SHA-256 校验、关闭，100 个样本。
SQLite 额外执行 integrity_check，redb 使用引擎打开恢复；比较的是两种设计实际需要的恢复扫描成本。
两边数据模型 envelope 的物理布局不同；这不是相同文件字节数的纯 fsync 微基准。

环境及基准原始输出：

- [environment.txt](evidence/formal-storage/environment.txt)
- [benchmark.txt](evidence/formal-storage/benchmark.txt)

原始记录是最终 schema/基准代码的一次完整运行；未按结果挑选重复试次。
前期验证曾用 200 次提交探路，随后修正了恢复扫描（真正 SIGKILL 夹具、每轮恢复 WAL）与
redb 覆盖路径（去掉不必要的旧值复制），不把不同方法的早期数字混入最终表。
初次正式测量曾误用默认 `/tmp`（tmpfs）；审查发现后整组数字作废，夹具改到槽位
`target/t35-bench` 的 Btrfs/NVMe，主库/WAL 恢复与 fsync 均在计时外。只报告物理文件系统上的完整运行。
最终运行 monotonic 与 boottime 的 elapsed 差为 0.000001 秒，没有挂起污染。

| 写入负载 | SQLite p50/p99 µs | redb p50/p99 µs |
|---|---:|---:|
| Action | 665.785 / 1403.697 | 913.356 / 1848.156 |
| 草稿 1 KiB | 922.079 / 1538.449 | 686.656 / 1342.605 |
| 草稿 100 KiB | 756.422 / 1658.462 | 1005.284 / 1676.179 |
| 草稿 1 MiB | 1371.467 / 3302.800 | 2917.975 / 5779.243 |
| 唯一 blob | 738.068 / 1508.968 | 874.245 / 1677.844 |
| 重复 blob | 5.783 / 26.904 | 543.035 / 1038.437 |

| 崩溃重开与校验扫描 | SQLite p50/p99 µs | redb p50/p99 µs |
|---|---:|---:|
| Action | 5972.765 / 15207.226 | 8944.804 / 18396.671 |
| 草稿 1 KiB | 2277.162 / 3889.555 | 5283.548 / 7883.248 |
| 草稿 100 KiB | 2351.441 / 3466.144 | 5412.989 / 7510.440 |
| 草稿 1 MiB | 3466.741 / 5113.465 | 8167.018 / 10630.940 |
| 唯一 blob | 6259.405 / 9213.720 | 8864.976 / 12381.341 |
| 重复 blob | 2015.489 / 3368.924 | 5498.641 / 7930.073 |

继续选择 SQLite。redb 的 1 KiB 草稿写入 p50/p99 更好；SQLite 其余五类独立写入和六类恢复扫描 p99 较低，
不是各指标全面领先，也不能沿用 ADR 0028 的 40 倍批量差距。单次本机数据没有置信区间，
调频/同机负载可能改变绝对时间；不宣称所有磁盘、OS 或断电场景都得到相同比率。

## 测试先行与故障覆盖

最初的行为测试在 `project` 模块不存在时以 unresolved import 失败，随后加入实现。
测试揭示 cap-std 的 O_PATH 目录句柄不能 fsync；已改成通过同一 capability 目录打开 `.` 后同步。
另一个先失败测试揭示替换源文件会扩大原 0600 权限；已保留现有权限，回归测试通过。
最终审查的先失败测试还复现了新库受 umask 影响产生 0644 权限；已将 Unix 新库显式创建为 0600，
并检查 SQLite WAL/SHM 继承私有权限，避免源文件保护好而草稿/恢复 blob 暴露。

| 测试文件 | 覆盖 |
|---|---|
| project.rs | Action/快照/草稿/blob 重开、资源绑定、未知/漂移 schema、旧 revision、hash 冲突、symlink/元数据逃逸、锁、0600 源文件及新库/WAL/SHM 权限 |
| project_faults.rs | 9 个 I/O 边界注入 ENOSPC、rename EACCES、草稿事务失败、完成事务失败、损坏 payload/初始化保留、意图之后的外部修改与恢复冲突 |
| project_kill.rs | Action 8 MiB 事务提交前 SIGKILL；物化 5 个屏障 SIGKILL；跨进程锁冲突与 SIGKILL 释放 |

故障测试不需要 root、真实满盘或特殊文件系统。SIGKILL 后检查完整 old/new 字节、未变的私人草稿、
恢复计划不擅自修改源文件；显式恢复后新文件和完成标记一致。大 Action 未提交事务不可见，已确认的 5 条精确保留。
旧 revision 即使遇到较新 pending 意图也直接丢弃，不触碰源文件；该边界另经先失败回归测试修复。
资源路径另以先失败测试覆盖重复 `/`、中间 `.` 和尾 `/` 的词法别名，避免两种数据库路径拼写指向同一源文件。
rename 失败的完整临时文件保持可读；外部 hash 不等于 old/new 时不覆盖，后续 revision 不能跳过 pending。

## 验证记录

最终 Rust 门禁通过：workspace tests 556 passed / 0 failed / 6 ignored（显式基准另跑），
项目新增 21 项测试全部通过；fmt、clippy -D warnings、rustdoc -D warnings、cargo deny、app/storage bins 构建全部通过。
文件规模、文档链接、界面字符串和分层检查通过。首次双语真实窗口 44/44 通过；所有修复后重新构建的最终二进制再次完成完整双语窗口，44/44 通过。
执行记录位于 `target/t35-final-gates/`，真实窗口证据位于 `target/t35-final-window/`。
窗口使用脚本的机器级互斥锁与 private Xvfb，运行中文和英文，不触碰用户桌面。
锁文件变动所触发的 Typst fork 源码重建通过；系统字体与 bundled 字体分别 31/31 个 stock 对照像素一致。
精简可审查的门禁与像素记录见 [gates.json](evidence/formal-storage/gates.json) 和
[typst-parity.json](evidence/formal-storage/typst-parity.json)，
最终 44 项窗口摘要见 [window-summary.txt](evidence/formal-storage/window-summary.txt)。

## 后续接入与明确边界

- SessionStore 迁移/退役与两种内核的保存命令接入另批实施；迁移需独立目标、兼容报告、旧库保留和唯一权威切换。
- 快照由内核生成，本批没有自动 reducer、Action pruning、GC、历史 refs/branch、跨文件原子组或 watcher。
- 只承诺最后 hash 检查捕获的冲突；普通文件系统没有 hash+rename 原子 CAS，非合作外部编辑器在最后检查后的竞态仍存在。
- 同 UID 恶意替换 metadata/父目录不属于 advisory 项目锁的沙箱；不支持的 fsync/hard_link 宿主返回错误，不静默降级。
- Linux 本机故障注入与 SIGKILL 通过不等于真实断电、网络文件系统、Windows/macOS 或浏览器验收。

## 给协调者的 PR-body 材料

### 问题

既有会话库不是正式项目存储；ADR 0028 单事务批量负载不能代表逐动作提交、草稿覆盖或崩溃恢复。
缺少永久 schema、项目互斥锁以及数据库提交与源码物化之间可审查的恢复协议。

### 改动

ADR 0049 根据本机 Btrfs/NVMe 上的 SQLite/redb 对比继续选择 SQLite WAL/FULL，
明确 schema v1、SHA-256、快照、迁移和 SessionStore 退役路线。新增独立 ProjectStore，
实现 Action/update 事务、私人草稿、内容寻址 blob、项目锁、manifest 原子物化及显式恢复，
并加入可注入 I/O、真实 SIGKILL 与权限/路径/冲突测试。

### 验证

所有共享门禁逐项结果见本文件验证记录及 evidence/formal-storage；
引擎六类负载各 1000 次提交/100 次崩溃重开，保留 redb 在 1 KiB 草稿写入更快的结果。
Typst fork 源码可重建，系统/bundled 两组各 31/31 stock 像素一致。

### 待办

另批接入两种内核和保存命令、显式会话迁移/退役、内核自动 checkpoint、watcher/reconcile、
跨文件写入组、历史 refs/GC；真实断电与其他宿主验收仍独立进行。
末次 hash 校验与 rename 不能替代针对非合作外部 writer 的原子 CAS。
