# ADR 0049：正式项目存储、恢复意图与源码物化

- 状态：Accepted（阶段 1 项目存储骨架；尚未接入 app/session）
- 日期：2026-11-03
- 影响模块：scholium-storage；后续两种项目语言共享接入
- 关联：[ADR 0028](ADR-0028-storage-engine.md)、[ADR 0033](ADR-0033-structured-local-session-migration.md)、
  [存储规格](../modules/storage.md)、[源码权威与文件操作](../MIXED_SOURCE_EDITING.md)

## 问题与测量

ADR 0028 的单事务十万追加不代表交互编辑。本次用 rusqlite 0.37 / 系统 SQLite 3.54.0
与 redb 3.1.3 重测独立事务。SQLite 为 WAL + synchronous=FULL，redb 显式 Immediate。
各写入项 1000 个事务；Action/附件 1 KiB，草稿同一资源反复覆盖 1 KiB、100 KiB、1 MiB；
附件分唯一内容与重复内容。计时包括 SHA-256、envelope、事务建立及提交，数据生成不计时。
SQLite 使用正式六表 schema；redb 使用等价的资源键/序号/内容键和 version/hash/payload KV。
这是引擎持久化负载比较，不含 UI、语义计算、项目锁或源文件物化耗时。

每个引擎/负载由子进程写入，父进程在已提交状态 SIGKILL；保存此崩溃夹具。每次重开前在
计时外恢复并 fsync 同一主文件及 SQLite WAL，使用新连接/新数据库句柄，扫描并验证所有 payload 的
SHA-256；SQLite 同时做 integrity_check，redb 执行其打开恢复。各 100 次，包含关闭时间。
数据库自身缓存是冷的，**OS 页缓存是热的**，没有 root/drop_caches；不把它称为断电或物理冷盘基准。

环境：Arch Linux，Linux 7.2.11-arch1-1，i7-13650HX，本机 Btrfs/NVMe，夹具固定在槽位 `target/t35-bench`（不使用 `/tmp` tmpfs），Rust 1.99.0；
workspace test profile（依赖 opt-level=2）。nearest-rank p99；单次样本没有置信区间，
调频、OS 缓存和同机负载会影响绝对时间。monotonic/boottime 差小于 1 秒，无挂起污染。
复现与原始输出见[开发日志](../log/2026-11-03-formal-storage.md)及
[基准原始记录](../log/evidence/formal-storage/benchmark.txt)。

| 负载 | SQLite 写 p50/p99 µs | redb 写 p50/p99 µs | SQLite 崩溃重开扫描 p50/p99 µs | redb 崩溃重开扫描 p50/p99 µs |
|---|---:|---:|---:|---:|
| 独立 Action | 665.785 / 1403.697 | 913.356 / 1848.156 | 5972.765 / 15207.226 | 8944.804 / 18396.671 |
| 草稿 1 KiB | 922.079 / 1538.449 | 686.656 / 1342.605 | 2277.162 / 3889.555 | 5283.548 / 7883.248 |
| 草稿 100 KiB | 756.422 / 1658.462 | 1005.284 / 1676.179 | 2351.441 / 3466.144 | 5412.989 / 7510.440 |
| 草稿 1 MiB | 1371.467 / 3302.800 | 2917.975 / 5779.243 | 3466.741 / 5113.465 | 8167.018 / 10630.940 |
| 唯一 blob | 738.068 / 1508.968 | 874.245 / 1677.844 | 6259.405 / 9213.720 | 8864.976 / 12381.341 |
| 重复 blob | 5.783 / 26.904 | 543.035 / 1038.437 | 2015.489 / 3368.924 | 5498.641 / 7930.073 |

## 裁决：继续 SQLite，使用其唯一事务日志

SQLite 在 Action、100 KiB/1 MiB 草稿、唯一与重复 blob 的写入 p99 较低，六项恢复扫描 p99 均较低；
没有整体反转为 redb。redb 的 1 KiB 草稿写入 p50/p99 更快（687/1343 µs 对 922/1538 µs），
保留这个负面结果；100 KiB 草稿 p99 很接近，不声称 SQLite 全面领先。
原 40 倍批写结论不外推到本次负载。redb **仅为 dev-dependency**，产品不链接它。

正式数据库位于 `.scholium/history/project.sqlite3`，旁边的 `-wal`/`-shm` 由 SQLite 管理。
不另建应用权威 WAL。数据库不是缓存，不能单删 WAL；运行中备份须使用 SQLite 一致备份机制。
`.scholium/history/project.lock` 是 OS advisory lock 文件，持锁至 SQLite 关闭以后；不以文件存在
判定占用、不删除旧 lock 文件。SIGKILL 自动释放 OS 锁，同机第二个进程收到明确 Locked。
协作仍由上层协议处理，项目锁不提供网络文件系统/不同机器的互斥保证。

只提供 `OpenPolicy::Full`；数据库提交使用 WAL/FULL，物化文件/目录使用 fsync。
返回错误可能已有持久化意图或新源文件，不能将错误解释为「完全没有发生」，也不能显示保存成功。

## 永久 schema v1

`application_id=1396919378`、`user_version=1`。这与 SessionStore structured v1 是不同的格式。
SQL 定义是 `src/project/schema.sql`；STRICT 表、外键、CHECK 和 immutable triggers 都参与精确校验。

| 表 | 主键 | 内容/约束 |
|---|---|---|
| resources | 16-byte 随机资源身份 | 唯一 UTF-8 项目相对 path；路径不是身份，不能隐式重新绑定 |
| actions | 从 1 连续增长的 durable sequence | version=1、payload、SHA-256；只追加、禁止 UPDATE/DELETE |
| blobs | 32-byte SHA-256 | 不可变 bytes；写入 dedup 后比较 bytes，读取验证 hash；禁止 UPDATE/DELETE |
| drafts | resource FK | version=1、可空 base_hash、bytes、hash；独立覆盖/丢弃，不改源文件或 Action |
| manifests | resource FK | version=1、正 revision、可空 old_hash、new_hash blob FK、pending 0/1 |
| snapshots | Action sequence FK | version=1、bytes、hash；不可覆盖或删除，边界必须是已提交 Action |

SQLite envelope 的版本列与校验列是永久编码，不直接序列化内部 Rust 结构。Action payload
必须由内核编码完整的 Action 身份/actor/origin 和权威 update，领域格式在 payload 内自行版本化。
`append_action` 将其作为一个事务提交；序号在 commit 后才对调用者可见，不另写相互独立的状态/WAL。
本骨架不解释 CRDT，不生成产品历史，也不声称已实现 history DAG。

快照由内核在某个已提交 Action 边界构造，`save_snapshot` 校验外键并单独原子提交不可变副本；
加载最高边界后重放其后的 Actions。两边的领域一致性由内核保证。接入策略为累计 1024 个动作或
8 MiB 更新后请求 checkpoint；本骨架没有内核 reducer，因此不自动制造快照。
首版不压缩、不删除 Actions/旧快照/blob；GC、压缩 codec 与 refs 须另行定义保留策略及版本。
SQLite 自身默认 WAL auto-checkpoint 与领域 checkpoint 是两件事。

SHA-256 使用 `sha2 0.10`（MIT OR Apache-2.0，未启用 asm，未增加 C/C++ 构建依赖）；
cap-std 3.4.6（Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT）提供 capability-relative 源码 I/O。
SQLite 沿用[现有原生依赖登记](../NATIVE_DEPENDENCIES.md)，连接不跨线程共享。

未知 application/version、额外/缺失/改变的 schema、损坏 checksum/引用、非连续 Action 或异常初始化文件
都拒绝打开；校验发生在启用 writer/WAL 前，不补表、不重解释 legacy、不降级临时库。
Unix 新库以 0600 创建（SQLite WAL/SHM 继承私有权限），不扩大旧库权限。
新库在相邻 `.initializing` 临时库完成事务、验证、关闭并 fsync，再 hard_link 不覆盖发布、fsync 目录。
中断的临时库原样保留并报错。SQLite 自身会在读打开时识别已提交 WAL；这不等于应用执行数据修复。

v1 没有自动迁移。增加永久表/列/envelope 或改变约束必须提升版本并写迁移器：持项目锁，一致备份
包含 WAL 中的已提交数据，在独立目标库迁移/验证/关闭/fsync，以不覆盖方式发布，保留旧库与失败目标。
不得原地猜测未知格式，迁移失败不得覆盖正文；旧 writer 遇新版本必须拒绝。

## 源码草稿与物化协议

标准 `.typ`/`.tex` 仍为源码项目的权威正文。draft 只是带原始磁盘 base_hash 的私人未提交输入，
解析错误不阻止其落盘，默认不进共享/发布包。草稿可先于源文件及其目录存在，物化不自动丢弃草稿。
项目语言/authority 由项目创建或导入时的声明配置与内核决定，本存储骨架不推断/切换语言。

单资源物化顺序：

1. 校验身份/path、revision 范围；旧 revision 直接 Discarded，已完成的相等 revision 也丢弃；当前或更高 revision 遇 pending 先显式恢复。
2. 读取源文件 exact-byte hash，与 expected_disk_hash 比较；None 明确表示预期不存在。
3. 一个 SQLite FULL 事务绑定资源、去重写入目标 blob、写入 revision/old_hash/new_hash/pending=1。
4. 同目录独占创建临时文件，继承现有权限，写完整 bytes，fsync 临时文件。
5. 再次验证磁盘 hash，然后 atomic rename，fsync 同目录。
6. 第二个 SQLite FULL 事务将 pending=0。保留 revision 与 hash，后续旧请求不能覆盖。

数据库意图与源码是协调的两阶段发布，不是两个独立可写权威。SourceAuthority 中 blob 仅用作
未完成物化的恢复材料；外部内容不因有数据库副本而被无条件覆盖。
原生权威项目将来使用同一 Action/update 边界，生成源码是派生物。

`recover` 和 `open` 只返回计划，不改源文件：

| pending 对应的磁盘状态 | RecoveryState | 显式 resume_materialization |
|---|---|---|
| hash=old_hash | CompleteFromOld | 从已提交 blob 重新做临时文件发布 |
| hash=new_hash | FinalizeNew | 同步现有文件与目录，再提交完成标记 |
| hash 与两者均不同 | Conflict | 返回 Conflict，保留磁盘、意图、blob、草稿与临时文件 |

恢复时重新读 hash，不能信任旧计划；重复恢复幂等。任何失败后源文件只有 old 或 new 的完整字节，
不会暴露半个文件；磁盘满、rename 或最后提交失败留下 pending，可供再次显式恢复。
异常临时文件通过 RecoveryPlan 列出，不自动删除。当前不做破坏性日志截断/自动 salvage；
若以后引入，必须先保全损坏主库/WAL/SHM 的副本，不能截断后才备份。

## 路径与宿主边界

路径拒绝绝对路径、父目录跳转、重复分隔符/`.`/尾部分隔符别名和 `.scholium` 资源访问，源文件/目录 symlink 被拒绝；
源码操作经 capability directory，相对同一父目录句柄读取/rename/fsync，防止 symlink 逃逸。
元数据目录/数据库/锁及 SQLite sidecar 在打开前也拒绝 symlink。项目锁的协议参与者不得移动或
替换 `.scholium`、锁文件、数据库和源文件父目录；本骨架不是抵御同 UID 恶意进程篡改元数据的沙箱。

content hash 在提交意图前及 rename 前各校验一次，不依赖 mtime 或时间窗口。普通文件系统没有
「比较 content hash 后条件 rename」原语：不遵守项目锁的外部 writer 在最后校验与 rename 间仍有
不可原子化的竞态，不能声称获得跨任意外部编辑器的线性化 CAS。后续 watcher/reconcile 接入须保留
这个边界；本批不实现 watcher。当前故障测试覆盖意图提交后的外部改写与恢复阶段的冲突拒绝。

文件/目录 fsync 和不覆盖 hard_link 发布在当前 Linux 本地文件系统验证；不支持这些原语的宿主
返回错误，不静默复制/降低持久性。Windows/macOS、真实断电与浏览器 IndexedDB/OPFS 另行验收。
跨文件 write group、历史 refs/branch 与 GC 不在本次 API 内，不能宣称已有多文件全 old/全 new 能力。

## SessionStore 关系与后续迁移

legacy 与 structured v1 的代码、API 和默认 app 保存路径均不变；正式项目库使用独立 application_id
及命名空间，不自动把会话文件当项目库，也不退役现有库。
后续接入两种内核时，持旧库只读一致事务验证 head/请求日志/结构 envelope，生成带兼容报告的
新项目与资源身份；历史原始数据保留为只读档案，不能编造旧 Action 的 actor/update。
在独立目标验证重开/恢复及两边窗口保存闭环后，显式切换唯一权威；旧库保留可回退副本，再另批退役。
