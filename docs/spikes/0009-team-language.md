# Spike 0009：阶段 0 第 6 项 — 团队语言协调

> **有效性（2026-09-17 登记）**：阶段 0 第 6 项的原始证据，真实网络／多进程／磁盘／解析器未验证；契约记在 [ADR 0001](../adr/ADR-0001-mixed-source-team-editing.md)（仍为 Proposed）。阶段 0 当前判定见 [报告 0012](0012-exit-criteria.md)。

- 结论：**Pass（内存模型 + 确定性时序；真实网络 / 多进程 / 磁盘 / 解析器均未验证）**
- 对应验证项：[路线图阶段 0 第 6 项](../plan/ROADMAP.md)
- 日期：2026-09-16
- 执行者：ation_ciger
- 关联 ADR：[ADR 0001 团队单一源码语言与 LaTeX/Typst 混合项目](../adr/ADR-0001-mixed-source-team-editing.md)
  （该 ADR 状态为 Proposed，尚未加入指回本报告的反向链接；反向链接应由整合者随提交补上）
- 代码：[`spikes/language-coordination/`](../../spikes/language-coordination)

## 问题与判据

**判据在动手前写定，实现过程中未调整。** 本 spike 要关闭的"待验证"是：`docs/MIXED_SOURCE_EDITING.md`
第 3 节的状态机与第 4 节的门禁是否足以保证"同一共享分支同一时刻只有一种源码语言可写"，
以及在切换、重启、分区、旧包与恶意客户端下是否仍成立。判据如下（`C1`–`C7`）。

通过条件：每条判据至少 1 个**成功夹具**（期望行为成立）与 1 个**失败夹具**（异常/攻击被拒绝或被检测）；
每个用例单独断言、单独打印，任何一条不通过即整体 `Fail`。

| 编号 | 判据 | 通过条件（成功夹具） | 失败条件（失败夹具） |
|---|---|---|---|
| C1 | 同语言多人编辑 | 活动语言为 LaTeX 时 3 个客户端交错写入全部接受，两种投递顺序渲染结果一致 | 过期许可 / 伪造许可被拒，且不进入共享内容 |
| C2 | 切换屏障 | 屏障期间旧语言在途写入可提交完成；目标语言写入被拒；屏障原子提交后新语言可写 | 目标语言在屏障期间被拒绝；旧 epoch / 旧许可在切换后被拒；扫描时间线 0 跨语言可写窗口 |
| C3 | epoch 隔离 | 每次切换 epoch 单调递增，新 epoch 写入可接受；控制记录回放不倒退 | 旧 epoch 写入被拒且**不自动回灌**（两种语言正文都没有它），同时保留为本地草稿 |
| C4 | 服务重启 | 重启后 active / epoch / phase / 待冲刷队列 / 许可 / 成员 / 已应用全部一致，队列可在恢复后冲刷 | 尾部半写被截断并报告；中部损坏拒绝自动恢复；空日志拒绝自动恢复 |
| C5 | 网络分区 | 连接侧可继续写入；隔离侧重连后重新授权可重放，标记恰好出现 1 次 | 隔离期间写入 `NetworkUnreachable` 且无接受记录；重连旧草稿被拒且无回执 |
| C6 | 旧包 | 已确认重复包返回**原回执**且不重复应用 | 旧 epoch / 旧协议 / 序号回退 / 同序号异内容分别被拒并给出明确原因 |
| C7 | 恶意写集 | 合法基线包被接受（校验不等于一律拒绝） | 19 类攻击逐条拒绝且原因各不相同，攻击内容 0 条进入正文 |

补充要求：报告必须区分成功夹具与失败夹具、给出真实 stdout、并说明未验证范围。

## 环境

- 工具链：`rustc 1.98.0-nightly (bd08c9e71 2026-06-25)`、`cargo 1.98.0-nightly (a595d0da2 2026-06-20)`，Edition 2024。
- 平台：Linux x86_64；纯逻辑 crate，无 GUI、无网络、无文件系统依赖。
- 依赖：`thiserror = 2.0.20`（MIT OR Apache-2.0），精确锁定、离线缓存可用；没有引入 npm / Node.js / WebView。
  除标准库外无其他依赖，因此 `cargo deny` 类别风险为零。
- 构建与运行命令（可直接复制执行）：

```bash
export CARGO_HOME=/home/ation_ciger/Projects/Mogan/scholium/spikes/native-ui/.cargo-home
cd /home/ation_ciger/Projects/Mogan/scholium
cargo build --manifest-path spikes/language-coordination/Cargo.toml
cargo run --release --manifest-path spikes/language-coordination/Cargo.toml
```

- 静态检查（本 spike 自查）：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 均通过；
  `cargo build`、`cargo build --release`、`cargo run --release` 编译警告为零。

## 方法与夹具

实现是**单进程、内存、确定性逻辑时钟**的协调者模拟：没有真实 socket、没有真实多进程、没有真实磁盘。
所有事件按脚本顺序发生，每个事件推进一个逻辑 tick，因此任何断言都可精确复现。

`spikes/language-coordination/src/` 按职责拆分（每个文件均 < 600 行）：

| 模块 | 职责 |
|---|---|
| `model.rs` | 方言、epoch、`Phase`、许可、写入包、`Decision`、常量 |
| `error.rs` | `RejectReason`（每个变体对应一条独立断言）与 `RecoveryError` |
| `validate.rs` | 结构级写集校验：协议、空写集、条数、负载、路径越界、扩展名、混语言、内容方言抽检 |
| `coordinator.rs` + `coordinator/{control,gate,recovery}.rs` | 状态机、控制面、写入门禁、WAL 重放 |
| `wal.rs` | 简化追加日志：`u32 len | payload | u64 fnv1a`，尾部截断 / 中部损坏语义 |
| `crdt.rs` | 确定性操作日志（收敛断言的最小模型，不是 CRDT 选型） |
| `timeline.rs` | 许可可写区间、接受序列、屏障、拒绝记录与重叠扫描 |
| `harness.rs` + `fixtures.rs` + `scenarios/` | 逐用例证据与 7 条判据的夹具 |

**关键的"防自欺"设计**：门禁策略可切到故意写坏的对照实现，用来证明断言有牙齿，否则"扫描到 0 重叠"
可能只是扫描器永远返回 0。

- `GatePolicy::Strict`：设计要求；校验 epoch → 许可 → 撤销/有效期 → 阶段/语言 → 序号。
- `GatePolicy::TrustPermitToken`：只信任客户端缓存的许可 token（`MIXED_SOURCE_EDITING.md` 第 3 节明令禁止的写法）。
- `GatePolicy::SkipStructuralValidation`：跳过结构级写集校验，保留 epoch / 许可门禁。

判据 2 的"不存在两种语言同时可写的窗口"被形式化为两条可扫描检查：

1. 不同方言的许可可写区间 `[issued, revoked)` 是否重叠 → `cross_dialect_permit_overlaps()`；
2. 接受序列中相邻两次接受语言不同时，两次之间是否**存在匹配的屏障**（epoch 递增）→ `dialect_changes_without_barrier()`。

## 结果

总体：`spikes/language-coordination` 运行 **60 个用例，60 pass / 0 fail**，每条例均含成功夹具与失败夹具。

| 判据 | 结果 | 证据 |
|---|---|---|
| C1 同语言多人编辑 | Pass | 4 用例（成功 1 / 失败 2 / 对照 1），见下 |
| C2 切换屏障 | Pass | 11 用例（成功 6 / 失败 4 / 对照 1），见下 |
| C3 epoch 隔离 | Pass | 5 用例（成功 2 / 失败 2 / 对照 1），见下 |
| C4 服务重启 | Pass | 6 用例（成功 3 / 失败 3），见下 |
| C5 网络分区 | Pass | 6 用例（成功 3 / 失败 2 / 对照 1），见下 |
| C6 旧包 | Pass | 6 用例（成功 2 / 失败 4），见下 |
| C7 恶意写集 | Pass | 22 用例（成功 2 / 失败 19 / 对照 1），见下 |

汇总行（`cargo run --release` 的真实输出）：

```text
=== 判据汇总（逐用例） ===
C1 同语言多人编辑：多人并发写入并收敛: cases=4 pass=4 fail=0 [成功夹具=1 失败夹具=2 对照=1] -> PASS
C2 切换屏障：不存在两种语言同时可写的窗口: cases=11 pass=11 fail=0 [成功夹具=6 失败夹具=4 对照=1] -> PASS
C3 epoch 隔离：切换递增 epoch，旧 epoch 拒绝且不自动回灌: cases=5 pass=5 fail=0 [成功夹具=2 失败夹具=2 对照=1] -> PASS
C4 服务重启：活动语言 / epoch / 待冲刷队列可安全重建: cases=6 pass=6 fail=0 [成功夹具=3 失败夹具=3 对照=0] -> PASS
C5 网络分区：隔离写入必须被拒绝或重放，恢复后不双写: cases=6 pass=6 fail=0 [成功夹具=3 失败夹具=2 对照=1] -> PASS
C6 旧包处理：重复包返回原回执，旧包被拒并给出原因: cases=6 pass=6 fail=0 [成功夹具=2 失败夹具=4 对照=0] -> PASS
C7 恶意写集：逐条拒绝并给出原因: cases=22 pass=22 fail=0 [成功夹具=2 失败夹具=19 对照=1] -> PASS
总计: cases=60 pass=60 fail=0 -> PASS
```

### C1 同语言多人编辑

```text
[C1 成功夹具] c1.success.three_writers_converge -> PASS
    实际: accepted=9/9 timeline=9 identical=true markers=9/9
    说明: 文档 93 字节；三个 LaTeX 客户端交错提交
[C1 对照] c1.control.arrival_order_diverges -> PASS
    实际: identical=false
[C1 失败夹具] c1.failure.expired_permit_rejected -> PASS
    实际: reject(PermitExpired(id=4, expires_at=19, tick=24))
[C1 失败夹具] c1.failure.unknown_permit_rejected -> PASS
    实际: reject(PermitUnknown(id=4242))
```

三人各 3 条、交错提交，9/9 接受；把同一操作集按正序与逆序投递到两个副本，确定性渲染结果一致
（`identical=true`），9 个标记全部存在。对照夹具故意换成"按到达顺序拼接"，两种投递顺序立即不一致，
说明收敛断言不是恒真。失败夹具：`dave` 的许可 TTL=1、推进 5 tick 后写入被 `PermitExpired` 拒绝；
`mallory` 用伪造许可 ID 4242 被 `PermitUnknown` 拒绝，且两者都没有进入正文（`applied_len` 仍为 9）。

### C2 切换屏障

```text
[C2 成功夹具] c2.drain.in_flight_flush_accepted -> PASS
    实际: accept(receipt=2, epoch=1, ops=1)
    说明: Draining 下仍允许 from 方言冲刷
[C2 失败夹具] c2.drain.target_language_rejected -> PASS
    实际: reject(TargetDialectNotYetActive(target=typst))
[C2 失败夹具] c2.drain.incomplete_barrier_blocked -> PASS
    实际: Err(DrainIncomplete { missing: ["alice", "bob"] })
[C2 成功夹具] c2.barrier.committed_atomically -> PASS
    实际: Ok(SwitchOutcome { from: Latex, to: Typst, epoch: 2, revoked_permits: 2, flushed_ops: 1, tick: 11 })
[C2 失败夹具] c2.after.old_epoch_rejected -> PASS
    实际: reject(SourceEpochStale(got=1, current=2))
[C2 失败夹具] c2.after.revoked_permit_rejected -> PASS
    实际: reject(PermitRevoked(id=1))
[C2 成功夹具] c2.timeline.no_cross_dialect_write_window -> PASS
    实际: overlaps=0 unbridged_changes=0 min_gap=Some(3)
[C2 对照] c2.control.trust_token_opens_two_languages -> PASS
    实际: switch=true typst_accepted=true latex_accepted=true overlaps=2 unbridged=1
```

这正是判据核心。屏障开始后，旧语言在途写入仍可提交完成（`flushed_ops=1`），但目标语言写入在屏障期间
被 `TargetDialectNotYetActive` 拒绝；两个成员未确认时 `complete_switch(false)` 返回 `DrainIncomplete`；
确认后一次性原子提交 `epoch=2`、撤销 2 个旧许可。切换后旧 epoch 与旧许可分别被拒。

时间线扫描：**跨方言许可窗口重叠 0，无屏障的语言变化 0**，撤销到下一次发放的最小白障间隔 3 tick（≥1）。
对照夹具 `TrustPermitToken` 下同一时间窗内 latex 与 typst 写入同时被接受，扫描器报出 **2 处区间重叠、1 处
无屏障语言变化**——证明扫描器确实能抓到"两种语言同时可写"。

### C3 epoch 隔离

```text
[C3 成功夹具] c3.success.epoch_monotonic -> PASS
    实际: (Some(1), Some(2), Some(2), Some(3), 3, Some(3), Some(2))
[C3 失败夹具] c3.failure.old_epoch_rejected -> PASS
    实际: reject(SourceEpochStale(got=1, current=3))
[C3 失败夹具] c3.failure.no_auto_backfill -> PASS
    实际: backfilled=false drafted=true
[C3 成功夹具] c3.success.control_record_survives_replay -> PASS
    实际: recovered_epoch=Ok(3)
[C3 对照] c3.control.trust_token_accepts_stale_epoch -> PASS
    实际: switch_epoch=Some(2) decision=accept(receipt=2, epoch=2, ops=1) accepted_epoch=Some(2)
```

`LaTeX(1) → Typst(2) → LaTeX(3)`，epoch 不复用且单调。epoch=1 的写入在 epoch=3 被 `SourceEpochStale` 拒绝；
关键断言是 `backfilled=false drafted=true`：被拒文本在 LaTeX 与 Typst 两种正文里都不存在（**没有悄悄转换
成新语言的写入**），同时保留在本地草稿队列（拒绝不等于删除输入）。把控制记录全量回放后仍是 `epoch=3 /
latex`，控制记录不随历史回放倒退。对照夹具下同一旧包被接受并登记到 epoch=2，证明 epoch 检查才是隔离来源。

### C4 服务重启

```text
[C4 成功夹具] c4.success.state_restored_exactly -> PASS
    实际: before=active=latex epoch=1 phase=Draining { from: Latex, to: Typst, target_epoch: 2 } queue=1 permits_probe=2 members=2 applied=3 drafts=0
          after=Some("active=latex epoch=1 phase=Draining { ... } queue=1 permits_probe=2 members=2 applied=3 drafts=0")
    说明: records=Ok(9) truncated_tail=None
[C4 成功夹具] c4.success.pending_queue_flushed_after_restart -> PASS
    实际: completed=Ok((2, Typst, 1)) queued_present=true
[C4 失败夹具] c4.failure.torn_tail_truncated -> PASS
    实际: truncated_tail=Some(69) drain_queue=Some(0)
[C4 失败夹具] c4.failure.mid_log_corruption_refuses_autostart -> PASS
    实际: Err(MidLogCorruption { offset: 59, detail: "checksum mismatch" })
[C4 失败夹具] c4.failure.empty_log_refuses_autostart -> PASS
    实际: Err(NoControlRecord)
```

在"屏障进行中、队列里有 1 条待冲刷写入"的状态下序列化控制记录并重建：active / epoch / phase /
队列 / 许可 / 成员 / 已应用全部逐字段一致；恢复后的协调者继续完成屏障并把队写入冲刷到旧语言正文。
失败夹具覆盖三种损坏：尾部截断 5 字节 → `truncated_tail=Some(69)` 且队列退回 0（半写的 `DrainQueued`
记录要么全在要么全不在）；中部篡改 1 字节 → `MidLogCorruption{offset:59}`，**拒绝自动恢复**；
空日志 → `NoControlRecord`，不允许用"默认 LaTeX epoch=1"顶替，也不允许因 presence 清空重新开放写入。

### C5 网络分区

```text
[C5 成功夹具] c5.success.connected_side_writes -> PASS
    实际: a1=accept(receipt=1, epoch=1, ops=1) switch=Ok(2) commit=Ok((2, Typst)) a2=accept(receipt=2, epoch=2, ops=1)
[C5 失败夹具] c5.partition.isolated_write_not_accepted -> PASS
    实际: reject(NetworkUnreachable(actor=bob))
[C5 失败夹具] c5.reconnect.stale_draft_rejected -> PASS
    实际: reject(SourceEpochStale(got=1, current=2))
[C5 成功夹具] c5.reconnect.authorized_replay_accepted -> PASS
    实际: refreshed=true replay=accept(receipt=3, epoch=2, ops=1)
[C5 成功夹具] c5.success.no_double_write -> PASS
    实际: occurrences=1 receipts_ok=true
[C5 对照] c5.control.trust_token_accepts_isolated_draft -> PASS
    实际: decision=accept(receipt=2, epoch=2, ops=1)
```

分区期间两侧各自尝试写入：连接侧（alice）继续提交并在隔离 bob 后完成切换；被隔离侧（bob）的写入
`NetworkUnreachable`，重放前没有任何接受记录包含它。恢复后 bob 先用旧 epoch 草稿重连 → 被拒且无回执；
经显式重新授权后在新 epoch 重放 → 接受一次。最终标记在正文中**恰好出现 1 次**（`occurrences=1`），
双写为 0。对照夹具下同一个旧草稿被静默接受——证明 epoch 与撤销检查是隔离的来源。

### C6 旧包

```text
[C6 成功夹具] c6.success.confirmed_duplicate_returns_receipt -> PASS
    实际: first=accept(receipt=1, epoch=1, ops=1) duplicate=accept(receipt=1, epoch=1, ops=1) receipt=Some(1)/Some(1)
    说明: switch=Some(2)
[C6 失败夹具] c6.failure.stale_epoch_rejected -> PASS         实际: reject(SourceEpochStale(got=1, current=2))
[C6 失败夹具] c6.failure.protocol_version_rejected -> PASS    实际: reject(ProtocolVersionUnsupported(got=1, supported=3))
[C6 失败夹具] c6.failure.sequence_rollback_rejected -> PASS   实际: reject(SequenceRollback(seq=4, last_accepted=5))
[C6 失败夹具] c6.failure.duplicate_seq_conflict_rejected -> PASS 实际: reject(DuplicateSeqConflict(seq=5))
[C6 成功夹具] c6.success.no_old_packet_leaked_into_content -> PASS 实际: applied=2 p1=1 p5=1
```

已确认的重复包在 epoch 已推进之后重发，仍返回**原回执**（`receipt=Some(1)/Some(1)`）且不重复应用
（`applied` 不增加、正文中 `[p1]` 恰好 1 次）。四类旧包分别命中不同原因：旧 epoch、旧协议版本、
序号回退、同序号异内容。这条顺序（重复包判定先于 epoch 判定）来自设计第 3 节第 6 步，是必须固化的实现要点。

### C7 恶意写集

19 条攻击全部逐条拒绝，每条单独打印期望与实际（此处节选，完整输出见运行）：

```text
[C7 失败夹具] protocol_unsupported    实际: reject(ProtocolVersionUnsupported(got=1, supported=3))
[C7 失败夹具] scope_mismatch          实际: reject(ScopeMismatch(expected=proj-scholium/branch-main, got=proj-other/branch-x))
[C7 失败夹具] forged_epoch            实际: reject(EpochForged(got=1001, current=1))
[C7 失败夹具] forged_epoch_max        实际: reject(EpochForged(got=18446744073709551615, current=1))
[C7 失败夹具] stale_epoch             实际: reject(SourceEpochStale(got=0, current=1))
[C7 失败夹具] path_traversal          实际: reject(PathOutOfScope(path="../other/secret.tex"))
[C7 失败夹具] path_absolute           实际: reject(PathOutOfScope(path="/etc/passwd.tex"))
[C7 失败夹具] path_wrong_extension    实际: reject(DialectExtensionMismatch(path="notes.txt", expected_ext=.tex))
[C7 失败夹具] path_control_byte       实际: reject(MalformedPath(path="bad\0.tex"))
[C7 失败夹具] declared_dialect_mismatch 实际: reject(DeclaredDialectMismatch(declared=latex, op=typst))
[C7 失败夹具] mixed_dialect_write_set   实际: reject(MixedDialectWriteSet(dialects=["latex", "typst"]))
[C7 失败夹具] content_dialect_mismatch  实际: reject(ContentDialectMismatch(declared=latex, foreign_marker="#let "))
[C7 失败夹具] malformed_payload        实际: reject(MalformedPayload(detail=control byte in payload))
[C7 失败夹具] oversized_payload        实际: reject(PayloadTooLarge(got=70034, max=65536))
[C7 失败夹具] too_many_ops             实际: reject(TooManyOps(got=65, max=64))
[C7 失败夹具] empty_write_set          实际: reject(EmptyWriteSet)
[C7 失败夹具] permit_unknown           实际: reject(PermitUnknown(id=4242))
[C7 失败夹具] permit_not_owner         实际: reject(PermitNotOwner(id=1, owner=alice, actor=bob))
[C7 失败夹具] dialect_not_active       实际: reject(DialectNotActive(active=latex, requested=typst))
[C7 成功夹具] c7.success.no_attack_content_in_shared_content -> PASS
    实际: attacks=19 leaked=[] applied=1 baseline=1
[C7 对照] c7.control.skip_structural_validation_accepts_attacks -> PASS
    实际: content=accept(receipt=1, epoch=1, ops=1) mixed=accept(receipt=2, epoch=1, ops=2)
```

19 类攻击覆盖：伪造 epoch（含 `u64::MAX`）、旧 epoch、协议版本、范围不符、路径穿越、绝对路径、
扩展名不符、路径控制字节、声明语言与操作语言不符、混语言写集、内容与声明方言矛盾、畸形负载、
超长负载、条数超限、空写集、未知许可、许可不属于提交者、写入非活动语言。每条都有独立断言，
且 0 条攻击内容进入正文（`leaked=[]`）。对照夹具在 `SkipStructuralValidation` 下让"内容方言矛盾"和
"混语言写集"通过，证明结构级校验确实是拦截来源，而不是恒真拒绝。

## 失败与不确定性

诚实记录如下（结论为 Pass 的部分，其边界在此列明）。

1. **首次运行 60 用例中有 2 条不通过，均为夹具/期望错误，不是实现缺陷。**
   - `c2.timeline...` 把撤销→发放的最小白障间隔写死为 1，实测为 3（撤销与下一次发放之间还夹了两次被拒提交）；
     判据本身不变，改为断言 `gap >= 1`（即"不存在同时可写瞬间"）。
   - `c5.partition.isolated_write_not_accepted` 的标记检查放在了**合法重放之后**，因此命中了重放本身的接受记录；
     改为在重放前记录"是否曾泄漏"。两条修正后逐用例重跑全部通过。
   记录这一点是因为它恰好说明"只看总数"会掩盖问题：修正前汇总仍显示 58/60，但看具体用例只有 2 条。
2. **真实环境全部未验证**：没有真实网络、真实多进程、真实 socket、真实时钟、真实磁盘 fsync/掉电。
   "分区"是协调者内部的一个 `BTreeSet` 标记，不是内核级网络隔离；"重启"是内存字节流的重新解析。
3. **持久化格式是简化帧**（`u32 len | payload | u64 fnv1a`），不是正式格式，也没有 fsync 语义。
   尾部截断是人工 `truncate`，中部损坏是人工翻 1 bit；真实掉电下的半写形态未测。
4. **没有真实 CRDT**：C1 的"收敛"只是确定性操作日志渲染。CRDT 合并、undo、乱序与幂等本身是阶段 0 第 4 项。
5. **写集校验远不足以支撑"能阻止恶意客户端"**：
   - "声明语言 vs 实际内容"只用 8 个 Typst 标记与 7 个 LaTeX 标记做字符串抽检，**不是解析器**；
     注释或字符串里出现异语言标记会误报（代码已注明），把标记藏在转义/注释里也能绕过。
   - 没有鉴权、没有签名、没有 viewer/editor 权限模型，没有按资源通道限定写入。
   - 因此本 spike 只证明"门禁结构存在且可逐条拒绝已知攻击形态"，不能据此宣称已经能阻止绕过。
6. **协调面是单副本串行状态机**：没有验证多副本协调者/共识下的 epoch 唯一性与网络重排。
7. **规模与压力未验证**：没有多客户端长时间运行、没有大负载下的队列/日志增长行为。
8. **客户端交互未验证**：没有真实 IME commit、drain 交互、超时隔离的用户流程；隔离与强制完成是脚本直接调用。
9. **恶意写集只覆盖 19 类**，未覆盖重放签名伪造、跨 scope 资源引用、编码/转义绕过、许可重放等。
10. **报告未回链**：`docs/plan/ROADMAP.md` 与 `docs/spikes/README.md` 的对应条目/状态表需由整合者更新
    （本次按约束只新增 spike 目录与本报告）。

## 对设计的影响

1. **设计状态机与门禁可落地**：`Active → Draining → Active`、epoch 单调、撤销与提交在同一原子提交内，
   在内存模型下全部成立。C2 的时间线扫描给出了"无同时可写窗口"的可复现判据，建议把它固化为实现期回归测试。
2. **需要写进设计的实现顺序约束（本 spike 发现）**：
   - 已确认重复包必须在 epoch / 许可检查**之前**判定并返回原回执，否则重连重发会被误判为 stale。
   - 屏障期间"接受旧语言在途写入"必须与"入队待冲刷"写成**同一条原子控制记录**，否则尾部截断会产生
     半提交状态（本 spike 初版即为两条记录，已拆分修正）。
   - 恢复时不得缺省回退到"某种语言可写"：空日志/缺 bootstrap/中部损坏都必须拒绝启动并给出原因。
3. **最小可信校验需要一个独立 spike 或明确降级**：`MIXED_SOURCE_EDITING.md` 第 4 节要求"服务端不能仅信任
   客户端声明的 dialect"，但本 spike 的标记抽检不足以作为证据。建议在阶段 0 出口条件中把"写集校验的可信度"
   单列为待验证项，或明确首发的攻击模型边界。
4. **建议补充 ADR 0001 的验收条款**：把本报告的 C1–C7 用例作为阶段 5 的回归脚本来源；
   并把"控制记录必须持久化、且不得因 presence 清空而重新开放写入"写成不变量。
5. **不通过时的下一步**：本项无 Fail 项；若后续真实网络/多进程验证出现 Fail，应先修协调面（单写者控制记录 +
   epoch 单调），而不是在客户端加语言锁。

## 复现步骤

```bash
# 1. 构建（离线缓存来自 native-ui spike 的 CARGO_HOME）
export CARGO_HOME=/home/ation_ciger/Projects/Mogan/scholium/spikes/native-ui/.cargo-home
cd /home/ation_ciger/Projects/Mogan/scholium
cargo build --manifest-path spikes/language-coordination/Cargo.toml

# 2. 运行：打印 60 个用例的成功夹具、失败夹具与对照，以及判据汇总
cargo run --release --manifest-path spikes/language-coordination/Cargo.toml
# 期望末尾：总计: cases=60 pass=60 fail=0 -> PASS

# 3. 自查（可选）
cargo fmt --manifest-path spikes/language-coordination/Cargo.toml -- --check
cargo clippy --manifest-path spikes/language-coordination/Cargo.toml --all-targets -- -D warnings
```

运行输出中的每条 `PASS`/`FAIL` 即判据级证据；进程在任一条不通过时以非零码退出，便于 CI 直接使用。
