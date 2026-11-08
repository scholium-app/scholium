# 2026-11-08：Typst 编辑会话接入 SDG Action 权威

本次在 wt2 从 `origin/main` 的 `38544dd` 创建 `ation_ciger/feat/sdg-session`，保留原来的
`ation_ciger/test/latex-exit` 与 PR #108。工作对应 T52；没有网络同步、协作 UI 或 LaTeX 内核修改。
持久化边界裁决见 [ADR 0059](../adr/ADR-0059-sdg-session-durability.md)，模块规范同步更新了
[session](../modules/session.md)、[collab](../modules/collab.md)、[history](../modules/history.md)、
[storage](../modules/storage.md)，并更新了 ADR 0053 的接入状态；没有修改共享索引或阶段出口判定。

## 实现与恢复

`EditorCore::run` 在结构规划成功后用当前 revision、新 RequestId 和 user origin 调用
`Replica::apply_effects`，再从 Replica 投影重建只读 planner。debug 断言与测试比较完整 identified snapshot。
每个改变共享正文的命令发布一个完整 `SCSDGA01` Action；临时槽补偿与后续编辑的复合命令合并中间 delta，
不发布中间 receipts。设备 undo/redo 使用 actor UndoManager，UI 词/删除分组保留，远端输入保留。

Action 在内存事务成功后通过 `ProjectStore::append_action` 提交；外层 Input 也成功后才写库。
提交失败保留内存输入、待提交队列、dirty 与可见错误，save 重试成功后才推进保存基线。
显式 save 在 durable sequence 上写 `SCSDGS01`；重开 import snapshot、重放其后 Actions。
初始化 Action 包含完整树增量，首个 snapshot 未提交时也可重开；初始化投影是保存基线，后续 tail 保持 dirty。
每次重开使用新 actor，**设备 undo 栈不跨重启持久化**。

失败回滚使用设备内存 journal 重建 engine、receipts、分组、undo/redo；普通 wire snapshot 不携带 UndoManager。
seed 在 actor attach 时建立，避免第一次 checkpoint 前的历史丢失。成功输入只追加设备步骤，不重放历史。
这不是产品 checkpoint DAG 或第二份可保存正文。

旧 native v1 envelope 严格校验后，把最后完整 head 作为初始化 Action 加 checkpoint 迁移，旧字节全部保留。
旧保存点之后的 durable tail 被纳入迁移 head；迁移后首次打开与再次重开使用同一保存基线。
实验 `EditorCore::open(path)` 只读导入原 SQLite 到相邻 `path.with_extension("sdg-project")`，
声明已发布但没有 durable head 时可重试；有新 head 后不再导入源文件。未知版本明确拒绝。

## 先失败后修复的证据

- [旧会话未持久化 Action](evidence/2026-11-08-sdg-session/session-authority-red.log)：
  新测试首先证实 payload 不是 `SCSDGA01`，随后接入新权威。
- [删除后 undo 身份错误](evidence/2026-11-08-sdg-session/deletion-undo-red.log)：
  display 插入撤销触发 `Sdg(Identity)`；物理 tree delete 的 undo 重建 metadata，复制 NodeId。
  改为在原 TreeID 上设置 inactive tombstone，保留原 metadata/text；专门测试验证反复 undo/redo 身份不变。
- [首次 checkpoint 丢 undo](evidence/2026-11-08-sdg-session/checkpoint-history-red.log)：
  checkpoint 前编辑在回滚后不能撤销；把 journal seed 前移到 attach 后修复。
- [无首个 snapshot 的保存基线](evidence/2026-11-08-sdg-session/initial-baseline-red.log)：
  初始化 Action 后的未保存 tail 重开被误标 clean；恢复时明确记录初始化投影后修复。
- [旧迁移保存基线](evidence/2026-11-08-sdg-session/migration-baseline-red.log)：
  带已保存 checkpoint 和后续 durable tail 的旧项目首次迁移与重开 dirty 不一致；迁移 checkpoint 统一基线。
- [继承描述符延长锁占用](evidence/2026-11-08-sdg-session/inherited-lock-red.log)：
  并行测试中出现 `Project(Locked)`。用复制同一 open file description 的确定性测试复现了 fork→exec 窗口：
  owner 关闭文件后，子进程描述符仍持 flock。storage 在 SQLite 关闭后显式 unlock，保留活跃 owner 的排他性。

## 测试与门禁

最终 [workspace 测试日志](evidence/2026-11-08-sdg-session/tests.log.gz) 记录了 **758 passed、0 failed、18 ignored**。
新增覆盖包括：四类结构冲突（wrap/delete、竞争 parent move、删除祖先/远端 child、并发 wrap）、
模拟远端 actor 的 undo/redo、三个固定种子的乱序/重复/随机穿插重放、含 pending 的 snapshot 恢复、
复合命令单 Action、拒绝输入回滚、ENOSPC 提交失败重试、严格旧格式迁移与未知版本拒绝。
真实子进程 SIGKILL 覆盖初始化 Action 提交前、首个 snapshot 提交前、编辑提交前、显式 checkpoint 前、
命令已确认、保存已确认六个边界，恢复最后 durable prefix，且重启 undo 为空。ignored 中包括手动性能测试和
由父测试显式启动的 SIGKILL helper；不是跳过恢复验收。

| 命令 | 结果 | 证据 |
|---|---|---|
| `cargo fmt --all -- --check` | PASS | [fmt](evidence/2026-11-08-sdg-session/fmt.log) |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS | [clippy](evidence/2026-11-08-sdg-session/clippy.log) |
| `cargo test --locked --workspace` | PASS | [tests](evidence/2026-11-08-sdg-session/tests.log.gz) |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | PASS | [rustdoc](evidence/2026-11-08-sdg-session/doc.log) |
| `python3 scripts/check-rust-size.py` | PASS | [size](evidence/2026-11-08-sdg-session/size.log) |
| `python3 scripts/check-doc-links.py` | PASS | [links](evidence/2026-11-08-sdg-session/links.log) |
| `python3 scripts/check-ui-strings.py` | PASS | [strings](evidence/2026-11-08-sdg-session/strings.log) |
| `python3 scripts/check-layers.py` | PASS | [layers](evidence/2026-11-08-sdg-session/layers.log) |
| `cargo deny --locked check licenses bans sources` | PASS | [deny](evidence/2026-11-08-sdg-session/deny.log) |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | PASS | [build](evidence/2026-11-08-sdg-session/build.log) |

退出码及完整参数另存为 [gates.json](evidence/2026-11-08-sdg-session/gates.json)。
独立 stock Typst 对照按 `typst-fork.yml` 的 fork example、reference manifest 与 `compare.py` 三步执行，
system 与 `SCHOLIUM_TEST_FONTS=bundled` 两个 profile 均 **31/31 pixels equal**：
[system](evidence/2026-11-08-sdg-session/stock-system.json)、[bundled](evidence/2026-11-08-sdg-session/stock-bundled.json)。
没有修改独立 reference 或像素阈值。

首轮窗口运行 `python3 scripts/window-checks.py --locale zh-Hans en --out target/t52-windows`
完成 **62/64**。失败是中文 native 的旧 revision 回退断言，以及中文 source-preview-layout 使用错误
工具链 override；修正后英文两项通过，但首轮使用过不同构建，不能冒充最终完整验收。
[首轮汇总](evidence/2026-11-08-sdg-session/windows-first.log.gz)、
[native 失败](evidence/2026-11-08-sdg-session/windows-first-zh-Hans-native.log.gz)、
[source-preview-layout 失败](evidence/2026-11-08-sdg-session/windows-first-zh-Hans-source-preview-layout.log.gz)
保留了证据。最终二进制的 [SHA-256](evidence/2026-11-08-sdg-session/window-binaries.json) 在完整重跑前记录。
所有窗口运行均通过 runner 的机器级锁和私有 Xvfb，没有接触用户桌面。

最终完整运行 `python3 scripts/window-checks.py --locale zh-Hans en --out target/t52-windows-final2`
退出码 0，**64/64 PASS（中文 32/32、英文 32/32）**，没有跳过或重试单项。
[最终完整日志](evidence/2026-11-08-sdg-session/windows-final.log)、
[逐项结果与耗时](evidence/2026-11-08-sdg-session/windows-final.json) 保留全部门禁结果。
`undo-group`、`backspace-block-merge`、全部 `math-*` 两个语言配置均通过，且脚本保持原样。
最终构建哈希在运行前后完全一致。大量原始 AUDIT/截图留在 ignored `target/t52-windows-final2`，
提交证据仅保留引用的最终/失败记录，合计小于 1.5 MB。

## 延迟测量

沿用 [TESTING 的性能预算](../TESTING.md#6-性能预算)，没有新增阈值。相同手动测试通过真实 native project
连续输入 140 个字面 `a`，丢弃 20 次 warmup，保留 120 个命令样本，计时包含同步 durable append；
before 使用接入前构建的测试二进制，after 使用接入实现。每轮比较 CLOCK_MONOTONIC 与 CLOCK_BOOTTIME，
差值均小于 1 微秒，没有检测到 suspend；[原始命令和结果](evidence/2026-11-08-sdg-session/latency.json)、
[before](evidence/2026-11-08-sdg-session/latency-before.log.gz)、[after](evidence/2026-11-08-sdg-session/latency-after.log.gz)。

| 每键 command | before | after |
|---|---:|---:|
| p50 | 0.289 ms | 0.603 ms |
| p95 | 0.531 ms | 0.642 ms |

p50 约翻倍、p95 增加约 21%，属于可见回归；新增 CRDT preflight、投影和 Action 编码有成本。
小文档测量仍低于既有 input-to-local-ack p95 16 ms 预算；不把它冒充单独 append p95、
端到端渲染延迟或大型项目验收。最后补充的初始化/回滚/解锁修复不改变这段稳态输入计时路径。

## 偏离、边界与未做项

保持了 `undo-group`、`backspace-block-merge`、`math-*` 窗口脚本原样。
`native-check.py` 的旧断言要求 undo 回退共享 revision，与追加式 Action 冲突；改为逐项精确比较 document/blocks
（含全部 NodeId），同时验证 revision 递增，保存/重开仍比较完整 snapshot，没有放松像素或时延阈值。
`source-preview-layout-check.py` 使用失效的 `SCHOLIUM_RTEX_BINARY`，没有触发缺失工具链场景；
依据实际 detector 改为 `SCHOLIUM_RTEX_BIN`，保留原失败场景断言。
项目锁 teardown 是为修复本次并行恢复测试稳定复现的所有权问题所作的最小 storage 补充。

没有触碰 T49/T51 保留的路径；原 PR #108 保留。按启动指令只推送分支，不创建 PR、不轮询 CI。
设备 journal 随会话历史增长，失败回滚重放其历史；大型文档/长会话性能仍需单独验证。
没有自动阈值 checkpoint 调度、持久化 undo、网络协议、协作 UI、历史 DAG、GC、跨平台断电或真实断电验收。
这次完成初始化/迁移/显式保存的 durable checkpoints 与真实 SIGKILL 恢复，不据此宣称上述未做项通过。
