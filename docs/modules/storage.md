# scholium-storage

## 职责与边界

正式项目存储位于 `project/`，提供项目锁、版本化 SQLite schema、追加 Action、内核快照、独立源码草稿、
内容寻址 blob、单资源原子物化及非破坏性恢复计划。它不解释 LaTeX/Typst，不决定撤销，不合并 CRDT，
不切换项目语言，不上传网络，也不展示冲突 UI。

[ADR 0049](../adr/ADR-0049-formal-project-storage.md) 是正式布局、永久 schema、版本迁移和持久性裁决。
`SessionStore` 是保留作只读迁移和兼容工具的 legacy/structured 会话格式；app/session 使用正式项目存储。
项目声明、默认路径与退役策略见 [ADR 0051](../adr/ADR-0051-project-files-and-session-retirement.md)。
两种项目语言共用该存储边界，标准 `.tex`/`.typ` 保持源码权威；项目语言与 authority 的声明配置不由数据库推断。
项目设置的声明 v2、入口资源身份与原子发布规则见 [ADR 0054](../adr/ADR-0054-project-settings.md)。
`SourceProject::update_settings` 在现有锁下保存主文件、显式引擎和参考文献偏好，不改语言、项目身份或源字节；
脏入口及未完成物化意图阻止主文件切换。入口资源映射保留私人草稿身份，恢复判定与源码打开一致。
新建声明仍为 v1，显式保存设置才发布 v2；两版均可读，旧读取器拒绝 v2，SQLite schema 不变。


## 内部结构

```text
src/project/
├── mod.rs          ProjectStore/OpenReport、项目打开与 typed error
├── types.rs        版本信封、稳定资源身份、revision/hash 与恢复 DTO
├── schema.sql      永久 v1 STRICT schema 与不可变触发器
├── schema.rs       不覆盖创建、先校验后 writer、完整性验证
├── data.rs         Action、快照、草稿、内容寻址 blob
├── handle.rs       不可变项目/资源身份、语言与入口声明、同格式副本
├── native.rs       临时版本信封、原生 Action 重放与快照
├── source.rs       私人草稿、磁盘基线、保存/重新载入
├── migration.rs    只读会话导入、新项目验证与保留原库报告
├── files.rs        capability-relative 文件操作、同目录临时发布
├── materialize.rs  revision/manifest、恢复检查与显式继续
└── io.rs           可注入 I/O/事务边界
```

## 持久化布局与 schema

```text
.scholium/history/
├── project.sqlite3                 正式项目库；application_id=1396919378，user_version=1
├── project.sqlite3-wal             SQLite 自有 WAL，存在时不可单独删除
├── project.sqlite3-shm             SQLite 管理的共享索引
├── project.lock                    OS 锁文件，文件存在不表示持锁
└── project.sqlite3.initializing     仅新建时的相邻临时库；异常遗留拒绝自动修复
```

不另建权威 WAL 或 WAL 目录。Action envelope 和权威 update 在一个 SQLite 事务里提交。
`resources(id,path)` 保存 16-byte 稳定身份和唯一相对路径；`actions(sequence,version,payload,hash)` 连续只追加；
`blobs(hash,bytes)` 使用 SHA-256 不可变内容键；`drafts(resource,version,base_hash,bytes,hash)` 是独立私人草稿；
`manifests(resource,version,revision,old_hash,new_hash,pending)` 协调数据库与文件；
`snapshots(sequence,version,bytes,hash)` 保存不可变内核快照，sequence 外键指向已提交 Action。
所有 envelope version=1，hash 是 32-byte SHA-256；资源、blob 与 Action 引用由 FK 约束。

未知版本、schema 漂移、内容校验失败、断裂 Action 序列/引用均拒绝，不补表、不自动迁移。
新增永久 schema/envelope 必须提升版本，经一致备份和独立目标迁移验证后显式发布，保留失败目标及旧库。
Unix 新库以 0600 创建，SQLite WAL/SHM 继承私有权限，不扩大已有库权限。
SQLite 连接不跨线程共享。浏览器存储另行适配，不将宿主持久性回调冒充本机 fsync。

## 公共接口

- `ProjectStore::open(root, OpenPolicy::Full) -> Result<OpenReport, StoreError>`：现有项目根内创建或校验库；
  持锁，未知格式在 writer/WAL 配置前拒绝；`OpenReport { store, recovery }` 不自动改源文件。
- `append_action(&ActionRecord) -> DurableSequence`：version=1 的 Action/update 在 SQLite WAL/FULL 事务提交后返回；
  序号与语义 revision 不混用。`actions_after(sequence)` 供内核重放，零表示完整日志。
- `save_snapshot(sequence, bytes)` / `latest_snapshot()`：由内核保证领域一致性的版本化 checkpoint。
  正式增量 adapter 接入时在累计 1024 动作或 8 MiB 更新后请求快照；临时 native 信封由显式保存触发快照，
  加载后重放其后的 Actions，不自动删日志/快照。
- `save_draft(resource, base_hash, bytes)` / `load_draft` / `discard_draft`：接受无效语法，保留原始基线，
  不触碰源文件、不生成 Action；草稿可先于文件目录存在。默认不进发布包，物化不自动清草稿。
- `put_blob(bytes)` / `get_blob(hash)`：SHA-256 去重，读写验证内容，不按路径或 mtime 判等。
- `materialize(resource, revision, bytes, expected_disk_hash)`：返回 Written 或 Discarded；
  旧 revision 与已完成的相等 revision 不读盘；当前或更高 revision 遇 pending 返回 RecoveryRequired，不覆盖上一个意图。
- `recover() -> RecoveryPlan`：只读检查 pending 和遗留临时文件。
  `resume_materialization(resource)` 显式继续，重新检查磁盘而不信任旧计划。
- `open_with_io(root, policy, Arc<dyn IoHooks>)`：注入打开、源读取、临时文件创建/写入/fsync、rename、
  目录同步、事务提交及意图提交后/rename 后的故障边界。

`Resource::new(id,path)` 校验唯一词法形式的 UTF-8 相对路径，拒绝重复分隔符、`.` 和尾部分隔符等别名；身份不得隐式绑定新 path，path 不得绑定另一身份。
重命名需要后续显式 catalog 操作。本骨架没有 watcher、跨文件 write group、history refs/branch、自动 GC 或浏览器存储。

## 项目生命周期

`ProjectHandle::create` 新建固定语言项目；Typst 为原生权威，LaTeX 为源码权威。
`open(path, entry)` 读取版本 1 声明或按明确的 `.typ` / `.tex` 入口导入普通目录；多候选入口必须显式选择。
声明与资源路径拒绝 symlink、不安全路径和语言/authority 不匹配。声明仅在首次创建/导入时不覆盖发布。
`copy_source` / `copy_native` 复制普通资源并生成新身份，排除私人元数据；已有/嵌套目标和特殊文件拒绝。

`NativeProject::open(handle, initial)` 持锁，验证并恢复快照及后续全部 Actions；空项目先提交初始化事件和快照。
`append_edit` 提交版本 1 临时绝对状态信封，含 structured 文档、当前请求身份日志和事件身份；
`save` 在最新 Action 边界写不可变快照，重复保存不覆盖。当前信封由共享 SDG adapter Action 替换，
不代表 CRDT、协作 actor 或产品历史。`saved_document` / `saved_requests` 保留最后保存基线供显式丢弃追加。

`SourceProject::open` 持锁并读取 UTF-8 源码或私人 draft，不执行语法或改源文件。
`edit` 保留私人输入并提交 durable draft，失败仍保留新输入与未保存状态；`save` 显式恢复该资源的 pending 意图、执行 materialize，随后清 draft。
外部 hash 变化返回 Conflict；没有未完成意图时 `reload` 显式丢弃 draft 并重读磁盘。
物化完成但草稿清理前崩溃，磁盘/draft/已完成 manifest hash 三者一致时恢复新基线，避免假外部冲突。
源文件缺失和空文件基线不同；损坏编码、锁冲突或失败提交不标记保存成功。

`migrate_session(source,target)` 只读一致验证 schema 0 或 structured v1 会话，生成独立项目及兼容报告；
当前 head 作为一次导入事件，旧历史仍在保留的原会话。RawMath 保留并报告数量。
迁移目标带 `.scholium-importing` 标记，保存、重开比对及报告同步完成前拒绝作为项目打开；
已有目标永不覆盖，失败目标保留。返回值含新 handle、原库规范路径与 Raw 计数。

## 物化与恢复不变量

1. 返回 durable 前达到 Full 策略；失败不转用临时库冒充成功。SQLite 关闭后才释放 OS 项目锁。
2. 物化先检查 expected content hash（None 表示预期不存在），同一 FULL 事务存 blob + revision/manifest 意图；
   同目录独占临时文件完整写入并 fsync、保持现有权限、再次比对 hash，atomic rename 后 fsync 目录；最后提交完成标记。
3. open/recover 不物化、不清理异常文件。磁盘 hash=old 时计划 CompleteFromOld；hash=new 时 FinalizeNew；
   两者均不等则 Conflict。显式恢复前再验证，冲突保留磁盘、草稿、blob、manifest 与临时证据。
4. ENOSPC、rename、fsync 或末次提交失败都不宣称成功；源文件保持完整 old/new，pending 可显式继续。
   不提供破坏性截断/修复；将来引入 salvage 时须先保全损坏数据库/WAL/SHM 的副本。
5. 源文件访问经 capability-relative 目录，拒绝 symlink/绝对路径/父目录跳转和 `.scholium` 资源；
   元数据及 SQLite sidecar 打开前也拒绝 symlink。锁的参与者不得移动/替换项目元数据或源文件父目录。
6. 外部修改按 exact-byte hash 判冲突，不按 mtime。最后 hash 校验与 rename 不构成跨不合作外部 writer 的
   原子 CAS；同 UID 恶意篡改 metadata 不在该锁协议的沙箱保证内。watcher/reconcile 接入不能掩盖此边界。
7. 私人草稿、已提交源码、内核权威 update 各有明确状态；数据库的恢复 blob 不成为另一个可独立提交的源码权威。
   固定项目语言/转换约束见 [项目语言与文件操作](../MIXED_SOURCE_EDITING.md)。

## 测试门禁

常规 CI 运行项目 round-trip、独立草稿、blob 去重/校验、快照 Action 边界、schema/未知版本拒绝、
同进程和跨进程锁、旧 revision、symlink 逃逸、外部 hash 冲突、ENOSPC/rename/fsync/commit 注入。
真实 SIGKILL 在大 Action 提交前以及意图提交后、rename 前后、目录同步前、完成事务提交前验证 old/new 与显式恢复。
引擎基准单独 opt-in，复现命令与 p50/p99 见 [ADR 0049](../adr/ADR-0049-formal-project-storage.md)。
真实断电、其他 OS/文件系统、多文件原子组、watcher 风暴在相应接入批次验证，当前不宣称通过。

## 本地候选 v1 与 legacy 独立迁移

[ADR 0033](../adr/ADR-0033-structured-local-session-migration.md) 约束现有 SessionStore：
legacy `user_version=0` 保持默认 save/load/snapshot_at；结构候选 v1 使用独立
create_structured/open_structured/save_structured/load_structured API 与 typed StructuredStoreError。
未知数据库版本在 DDL/日志模式调整前拒绝；v1 snapshot 需要明确 format/version envelope
及 identified 校验。SQL CHECK 拒绝旧裸 JSON，legacy API 不能读写 v1。请求日志必须连续、
唯一且与语义 revision 一致。没有读写失败后的临时库回退。

`migrate_legacy_file(source, target)` 以只读一致事务读取源 head、请求身份与旧快照；
只将 head 转为 identified 当前内容，未知 legacy 字段/表/列/元数据拒绝迁移，避免宽松旧 DTO 静默遗漏。原历史 JSON 进入没有写 API 的 legacy_snapshots
档案，不猜过去的叶身份或请求日志。所有旧公式 Raw 保留，能力限制随报告返回。
JSON 单条上限 128 MiB，旧档案合计 256 MiB/10000 行，请求 100000；超限拒绝、不截断。

候选在目标同目录独占临时文件构建，DELETE 日志、FULL 同步，关闭并 fsync 主文件后以
hard_link 不覆盖发布，再 fsync 目录；正常后清理自有临时文件。目标存在（含 dangling
symlink）或不可发布时不覆盖；发布后目录同步失败可能留完整目标，错误不宣称 durable。
原库始终保留，普通 app 不自动选择目标。重开候选恢复 WAL/FULL，不改变结构身份。
此格式是本地会话候选，不替代共享 WAL/历史、协作恢复或跨平台断电验证。

`open_structured` 只以 READ_WRITE 打开已有文件，不含 CREATE；校验 v1 schema 后才调整
WAL/FULL。legacy、缺失或坏库拒绝，不自动生成/迁移表或改变旧库日志模式。
开发主程序据此选择已知 v1；创建新库使用独立原子 create API。
