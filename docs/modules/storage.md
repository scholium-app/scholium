# scholium-storage

## 职责

打开两类权威项目、资源目录、追加 WAL、共享树/文本快照、原生正文或源码原子物化、外部文件观察、恢复、内容寻址 blob、
本地索引缓存和 schema migration。

## 非职责

不解释 LaTeX/Typst/Markdown，不决定撤销目标，不合并 CRDT，不上传网络，不展示冲突 UI。

## 内部结构

```text
src/
├── project_store.rs   项目打开/关闭与锁
├── catalog.rs         ResourceId ↔ path
├── wal.rs             长度前缀记录、校验和、fsync 策略
├── snapshot.rs        快照写入、验证、压缩协调
├── materialize.rs     temp + fsync + atomic rename
├── transaction.rs     多文件 write-ahead manifest
├── watcher.rs         去抖、稳定读取、self-write 识别
├── blob.rs            附件 hash 存储
├── recovery.rs        启动扫描与恢复报告
└── migration.rs       元数据版本迁移
```

## 公共接口

- `ProjectStore::open(root, policy) -> OpenReport`，不在发现异常时擅自修复。
- `append_action(record) -> DurableSequence`；返回前记录已达到配置的持久性级别。
- `materialize(resource, revision, bytes)`；旧 revision 的请求自动丢弃。
- `begin_workspace_write(group)` / `commit_workspace_write` 实现跨文件恢复语义。
- 以事件报告 ExternalCreated/Modified/Renamed/Removed，携带观察到的 content hash。
- `recover() -> RecoveryPlan` 先给计划，破坏性截断需保留损坏副本。

## 持久化布局

```text
.scholium/history/
├── schema.json
├── wal/00000001.log
├── snapshots/<hash>.snap
├── refs/branches.json
├── manifests/
└── blobs/<prefix>/<hash>
```

日志和快照格式独立版本。索引可以删除重建；WAL、快照和 refs 不可被当缓存清理。

候选窗口的可丢弃恢复 DTO 见 [ADR 0015](../adr/ADR-0015-ui-session-recovery-spike.md)，
只验证单文件内容/草稿闭环，不是上述持久化布局、历史实现或正式格式依赖。

## 不变量

- 先 WAL 后对 UI 宣称 durable，先完整临时文件后替换源文件。
- 目标数据库打不开或已有数据恢复失败时，保存不得退到临时库并宣称成功；已有会话无法确认完整时阻止覆盖。
- watcher 通过写入 token/hash 识别自身物化，不能只靠时间窗口；生成源码外部变更进入 reconcile 草稿。
- 多文件操作恢复后只能全部处于 old 或 new 语义状态；中间物理状态必须可继续完成。
- 不覆盖未知的外部修改；content hash 不匹配时返回 conflict。
- 项目锁只防同机进程踩踏，不代替协作协议。

## 失败处理

- content hash 不匹配时返回 conflict，不覆盖未知的外部修改。
- WAL 尾部半写可截断；中部损坏停止自动恢复并生成恢复报告，保留损坏数据副本。
- 磁盘满、权限变化或 rename 失败时，多文件操作必须停在 old 或 new 语义状态，并可继续完成。
- 旧 revision 的物化请求直接丢弃，不覆盖较新内容。
- watcher 通过写入 token/hash 识别自身物化；无法确认归属时按外部变更处理。
- schema migration 失败时保留备份并保持幂等；项目锁冲突明确报错，不代替协作协议。

## 测试门禁

对每个 I/O 边界注入失败；断尾日志、校验和损坏、磁盘满、权限变化、rename 失败、symlink 逃逸、
watcher 风暴、外部编辑与内存编辑竞态。恢复测试必须保留原损坏数据副本。

## 团队源码语言与混合项目合约

保存组件原文、宏/模板、版本化绑定 manifest、资源依赖和未提交源码草稿；具体长期编码另立持久化 ADR。
语法错误不阻止原文/草稿落盘。SourceDraft 保存 actor、原 generation、revision 和可见性，崩溃不被重新生成覆盖。
普通 .tex/.typ 无元数据也可打开；混合包打开先校验路径与依赖。另存为创建独立项目身份，不复制活跃许可或协作权限。
export 临时目录完整验证后发布；原项目和前次成功输出在失败时保留。私人草稿默认不进入共享/发布包。
共享分支外部文件变更先交 session 检查语言许可，非活动语言变更保留为草稿；不能静默覆盖磁盘或共享正文。
单机语言控制与共享控制缓存分开，不能把离线缓存当有效共享写许可。新增混合包重开和草稿/发布失败注入测试。

共同要求见 [混合源码与团队编辑](../MIXED_SOURCE_EDITING.md) 与 [ADR 0001](../adr/ADR-0001-mixed-source-team-editing.md)。

## 后端候选与宿主边界

按 SQLite/rusqlite → redb 验证；上文日志目录与内部文件列表是逻辑草案，物理编码待恢复试验和持久化 ADR 决定。若采用数据库事务日志，不再另设独立权威 WAL；应用 Action 记录纳入同一提交边界。标准源文件依旧保持源码权威，通过带 revision 的恢复 manifest 协调数据库与多文件物化。

浏览器 IndexedDB/OPFS 适配单独验证，不假定本机数据库可直接运行；导入/下载导出为文件能力基线，配额、驱逐及异常关闭见[WASM 计划](../plan/WASM.md)。宿主持久性确认不能冒充本机 fsync 语义。
