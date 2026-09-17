# Spike 0007：阶段 0 第 4 项 — CRDT 赛马

> **有效性（2026-09-17 登记）**：自研最小基线的原始证据，**不用于关闭引擎选型**（其关联 ADR 已如此声明）；引擎选型由 [ADR 0009](../adr/ADR-0009-crdt-engine.md) 依 [报告 0014](0014-crdt-engines.md) 固定。阶段 0 当前判定见 [报告 0012](0012-exit-criteria.md)。

- 结论：**Pass（自研最小实现；判据 J1–J5 逐用例通过）**，缺口与能力范围外事项见"失败与不确定性"
- 对应验证项：[路线图阶段 0 第 4 项](../plan/ROADMAP.md)
- 日期：2026-09-16
- 执行者：ation_ciger
- 关联 ADR：[0001 三层历史模型](../adr/ADR-0001-mixed-source-team-editing.md)（CRDT / Action / checkpoint 三层与本地撤销语义）、
  [0009 CRDT 引擎选型](../adr/ADR-0009-crdt-engine.md)（Proposed，双向链接）。
  **本报告不能用于关闭 ADR 0009**：本 spike 不使用任何现成 CRDT 库，只提供一条自研基线、
  一套不依赖第三方语义的验收夹具（J1–J5 可直接移植），以及稳定节点标识模型的收敛性论证，
  见"对设计的影响"。
- 代码：[`spikes/crdt-race/`](../../spikes/crdt-race)
- 原始 stdout：[`spikes/crdt-race/artifacts/run-release.log`](../../spikes/crdt-race/artifacts/run-release.log)（136 行逐用例判定，本文引用均为其节选）

## 问题与判据

本节在动手实现**之前**写定。实现过程中发现缺陷后只**增加**用例（J2.5 与 J1 的两条语义意图断言），
没有放宽、删除或重新解释任何已写定的阈值。

要关闭的"待验证"：`docs/HISTORY_COLLABORATION.md` 的三层历史模型（CRDT update / Action / checkpoint）
是否能在自研最小实现上同时成立——两端离线编辑收敛、本地撤销不吞远端输入、任意时刻可快照恢复、
10 万次动作规模可承受。本 spike **不使用现成 CRDT 库**，先建立一条不依赖第三方语义的自研基线；
第三方引擎（Loro → Yrs → Automerge）的比较仍是阶段 0 该 ADR 的义务。

夹具为自研最小实现：文本用 Logoot 类序列 CRDT（每字符全局唯一 `CharId` + 位置标识 + LWW 存活位），
结构树用操作日志 + Lamport 时间戳（节点 `NodeId` 稳定，父子/兄弟位置与属性都是 LWW 寄存器）。

### 判据 J1：收敛

同一份文档的两个副本各自离线编辑（文本插入/删除 + 结构操作：包裹 / 解除包裹 / 属性变更），
交换同一份操作集后**收敛到同一状态**。判据是**序列化状态字节完全相同**（同时打印哈希），
且对两种交换顺序（先 A→B 再 B→A、反向）都成立。

逐用例：

- J1.1 两端在不同位置并发插入文本。
- J1.2 两端在**同一间隙**并发插入（位置标识竞争，考验全序确定性）。
- J1.3 一端删除区间、另一端在该区间内并发插入。
- J1.4 一端包裹节点、另一端对同一节点并发改属性。
- J1.5 一端解除包裹、另一端在容器内并发插入文本。
- J1.6 两端各自执行一段混合脚本（文本插入/删除 + 包裹/解除 + 属性变更）后交换。

### 判据 J2：本地 undo 不回滚远端输入

构造 A 本地插入 x、B 远端插入 y，A 收到 y 后 undo 自己的插入：结果中 **y 仍在、x 消失**；
undo 只撤销当前 actor 自己的最近一个本地动作。

逐用例：

- J2.1 A 本地插入 `x`，B 远端插入 `y`，A 合并后 undo：`x` 不出现、`y` 出现。
- J2.2 A 本地删除一段既有文本，B 在删除区间内并发插入 `y`，A undo 删除：`y` 仍在，被删文本恢复。
- J2.3 A 连续两次本地插入（`p` 后 `q`），只 undo 一次：只有 `q` 消失，`p` 仍在，远端 `y` 仍在。
- J2.4 A 的 undo 生成新操作并能同步回 B：B 收到后与 A 状态字节相同，且仍持有 `y`（共享历史只追加）。

### 判据 J3：快照

任意时刻可序列化/反序列化，恢复后继续合并操作仍收敛。

逐用例：

- J3.1 快照往返：`restore(snapshot(r))` 的状态字节与 `r` 相同。
- J3.2 部分操作后快照，恢复出的副本继续接收剩余操作，与未快照副本收敛。
- J3.3 在冲突中途（A 已收到 B 的一部分操作、尚未合并完）快照，恢复后完成合并，收敛。
- J3.4 恢复出的副本能继续产生新的本地动作并被原副本合并（快照不只是只读视图）。

### 判据 J4：规模

两个副本混合共 **100 000 次动作**（文本插入/删除、属性变更、包裹/解除）能完成、内存不爆，
给出总耗时与每动作成本。通过条件：跑完、两端收敛（字节相同）、进程堆峰值增量 **< 256 MiB**、
无 panic。

### 判据 J5：随机化收敛用例

固定随机种子，多轮随机操作 + 随机交错（操作按随机顺序投递，允许远端操作先于其因果前驱到达），
全部收敛。

- J5.1 100 轮、2 副本、每轮 200 个随机动作，随机投递顺序。
- J5.2 10 轮、3 副本、每轮 150 个随机动作，随机投递顺序。
- 种子、每轮动作数与每轮状态哈希写入本报告；任何一轮不收敛即整体 Fail 并保留该轮种子。

### 全局门禁

- `cargo build`、`cargo run --release` 均成功，编译警告为零。

## 环境

- 平台：Arch Linux Rolling Release，内核 `Linux 7.2.4-arch1-2`，`x86_64`
- CPU / 内存：13th Gen Intel Core i7-13650HX（20 线程），32 GB
- 工具链：`rustc 1.98.0-nightly (bd08c9e71 2026-06-25)`、`cargo 1.98.0-nightly (a595d0da2 2026-06-20)`、LLVM 22.1.7；Edition 2024
- 依赖（`spikes/crdt-race/Cargo.lock` 锁定）：`thiserror 2.0.20`（MIT OR Apache-2.0）；
  其传递依赖 `proc-macro2 1.0.107`、`quote 1.0.47`、`syn 3.0.5`、`unicode-ident 1.0.24`（均宽松许可）。
  无 CRDT 库、无 npm/Node.js、无 WebView
- crate 属性：`publish = false`、`license = "MIT OR Apache-2.0"`、空的 `[workspace]`（独立 workspace，
  不进正式 workspace 依赖图）
- 构建：`export CARGO_HOME=/home/ation_ciger/Projects/Mogan/scholium/spikes/native-ui/.cargo-home`，
  全部命令加 `--offline`

构建与运行命令：

```bash
cd spikes/crdt-race
export CARGO_HOME=/home/ation_ciger/Projects/Mogan/scholium/spikes/native-ui/.cargo-home
cargo build --offline
cargo build --release --offline
cargo run --release --offline
```

## 方法与夹具

### 实现结构（26 个文件，4 415 行，最长文件 478 行，全部 < 600）

| 模块 | 职责 |
|---|---|
| `ids.rs` / `model.rs` / `error.rs` | `(actor, seq)` 全局标识、Lamport 时间戳、节点种类、thiserror 错误 |
| `position.rs` | Logoot 位置标识：任意两个位置之间总能生成新位置的算法与不变量 |
| `seq.rs` | 分块向量：按 `(位置标识, 字符标识)` 全序保存字符，支持按可见偏移定位（跨墓碑） |
| `op.rs` / `codec.rs` | 6 种可交换操作、手写二进制编解码、FNV-1a 哈希 |
| `text.rs` | 文本 CRDT：全序集合 + 每字符 LWW 存活寄存器 |
| `tree.rs` / `doc.rs` | 结构树 LWW 寄存器、每文本叶子一个文本序列、规范状态与渲染 |
| `replica.rs` / `structure.rs` | 副本状态、本地文本编辑、建节点/包裹/解除包裹、同步、规范状态 |
| `undo.rs` | Action 记录、补偿操作生成、时间戳指纹校验 |
| `snapshot.rs` | 快照与恢复 |
| `workload.rs` | 随机用户动作发生器（规模与随机化判据共用） |
| `evidence/` | 判据 J1–J5 的场景与逐用例输出 |
| `alloc_counter.rs` / `report.rs` | 堆占用计数器（全局分配器包装）、证据收集 |

### 收敛性论证（解释，不是新增判据）

下面的论证说明为什么本实现**必然**收敛，判据 J1/J5 只是它的实测对照。设 `O` 是某个副本已知的操作集合：

- **文本条目** `{ (位置标识, 字符标识, 字符) }` 等于 `O` 中插入操作的集合；可见顺序是全序
  `(位置标识, 字符标识)` 的排序结果。位置标识与字符标识都写在操作里、生成后不可变，
  所以条目集合、顺序都只是 `O` 的函数。
- **文本存活位**：每个字符一个 LWW 寄存器，取值是 `O` 中所有针对该字符的存活位写入按
  `(counter, actor)` 全序取的**最大值**。max 与遍历顺序无关。
- **结构树**：`kind` 由该节点唯一的创建操作确定；`parent`/`pos`/`place_ts` 是同一放置寄存器的
  max；`alive` 是存活寄存器的 max；每个属性键是独立寄存器的 max。全部与顺序无关。
- 同步只做集合并集，因此两个副本只要最终持有相同的 `O`，就必然得到同一个状态 `S(O)`，
  与投递顺序、重复投递无关。

论证成立的前提，也是实现必须守住的边界：（a）操作自带不可变的标识与时间戳，不依赖接收时的本地状态
重算；（b）`(actor, seq)` 全局唯一（同 actor 双设备写入会破坏这一条）；（c）`(counter, actor)` 是全序。
论证**只保证收敛**，不保证结果符合用户意图——并发互移可能成环，收敛但语义可疑（见"失败与不确定性"）。

关键设计取舍：

1. **操作全部可交换、可重复、可乱序**：结构操作写 LWW 寄存器，文本插入写全序集合，存活位写 LWW 寄存器。
   同步因此只需集合并集，不需要因果缓冲——这是 J5 乱序投递能通过的前提。
2. **本地编辑与远端合并走同一条 `apply` 路径**：本地编辑先按当前状态挑参数，再生成操作当普通操作应用。
   避免"本地路径与远端路径语义不一致"这类只在并发时暴露的 bug。
3. **删除是墓碑 + LWW 存活位**，不是物理移除：撤销删除与"远端在删除边界输入"都靠稳定标识重定位。
4. **节点标识稳定**：文本操作只引用节点标识，节点被包裹/移动后远端文本操作仍然有效（J1.5-intent）。
5. **每个 `pub` 项都有文档注释**；`cargo fmt --check`、`cargo clippy --all-targets`、6 个单元测试全过。

### 夹具

基线文档（`fixture.rs`，固定种子 `0x5EED_0001`）：

```text
root(document)
├── para(paragraph)
│   ├── body(text)      "hello world"
│   └── strong(strong)
│       └── bold(text)  "bold"
└── heading(heading)
    └── title(text)    "Title"
```

渲染为 `hello world**bold**\n# Title\n`。所有判据共用这一份夹具：一条判据只换动作脚本，不换数据。
副本 A 引导文档，副本 B 用 `fork` 从同一状态分叉（同一文档的两个设备），因此两侧共享基线操作与标识。

### 随机种子

| 用途 | 种子 |
|---|---|
| 夹具引导 / 结构位置 | `0x5EED_0001` |
| J1 各用例副本 B | `0xA1`–`0xA6` |
| J2 各用例副本 B | `0xB1`–`0xB5` |
| J3 各用例副本 B | `0xC1`–`0xC5` |
| J4 调度 / 负载 A / 负载 B | `0x5CA1_E001` / `0x5CA1_E002` / `0x5CA1_E003` |
| J5.1 两副本轮次基准（第 n 轮 + n） | `0x5EED_1234`（100 轮 → `0x5EED_1234`…`0x5EED_1297`） |
| J5.2 三副本轮次基准（第 n 轮 + n） | `0x5EED_9876`（10 轮 → `0x5EED_9876`…`0x5EED_987F`） |

### 与 `scholium-spike-core` 的关系

**没有复用其代码**，只借鉴了节点种类与"文本叶子 + 行内包裹"的思路。原因是它的身份类型不能承担 CRDT 责任：
`scholium-spike-core::NodeId` 是 arena 下标（本地、随加载变化），`CharId` 是无 actor 的 `u64` 序号，
两者都不满足"全局唯一、永不复用、跨副本可比"。本 spike 需要的是 `(actor, seq)` 全局标识，
所以另写了最小模型，并在报告里明确这是**思路复用而非代码复用**。

## 结果

逐条对应判据。以下 stdout 片段逐字摘自 `artifacts/run-release.log`；过长的渲染文本字段用 `…` 截断，
其余数字未改动。

| 判据 | 结果 | 证据 |
|---|---|---|
| J1 收敛 | **Pass**（6 用例 × 2 合并顺序 + 2 条语义意图断言 = 14/14） | `J1.1a`–`J1.6b`、`J1.5-intent`、`J1.3-intent` |
| J2 本地 undo 不回滚远端 | **Pass**（5/5，含"远端删除冲突时跳过"路径） | `J2.1`–`J2.5` |
| J3 快照 | **Pass**（5/5） | `J3.1`–`J3.5` |
| J4 规模 | **Pass**（100 000 动作，2.94 s，堆峰值增量 87.5 MiB，两端收敛） | `J4.1`、`J4.2` |
| J5 随机化收敛 | **Pass**（110 轮全部收敛：2 副本 ×100、3 副本 ×10） | `J5.1-round000`–`round099`、`J5.2-round000`–`round009` |
| 全局门禁 | **Pass**：`cargo build`／`cargo build --release` 零警告，`cargo run --release` 退出码 0，`cargo fmt --check` 与 `cargo clippy --all-targets` 无输出，6/6 单元测试通过 | 见"复现步骤" |

汇总行（日志末尾）：

```text
用例总数 136，通过 136，失败 0
全部场景墙钟时间 3.882325828s
```

### J1 收敛（节选，`a` = A→B→A，`b` = B→A→B，两组都通过）

```text
[PASS] J1.1a :: 离线操作 A=29 B=29；合并顺序 A→B→A（应用 2/2）；字节 906 == 906：true；hash 0x0393c9805614dfcf / 0x0393c9805614dfcf；文本 "XYhello worldZW**bold**\n# Title\n"
[PASS] J1.2a :: 离线操作 A=28 B=28；合并顺序 A→B→A（应用 1/1）；字节 846 == 846：true；hash 0xf4d67159b7e3a77d / 0xf4d67159b7e3a77d；文本 "helloXY world**bold**\n# Title\n"
[PASS] J1.3a :: 离线操作 A=32 B=28；合并顺序 A→B→A（应用 1/5）；字节 827 == 827：true；hash 0xf0d46fa3f61c4eb6 / 0xf0d46fa3f61c4eb6；文本 "Y world**bold**\n# Title\n"
[PASS] J1.4a :: 离线操作 A=29 B=28；合并顺序 A→B→A（应用 1/2）；字节 892 == 892：true；hash 0x6da5d5688f27a117 / 0x6da5d5688f27a117；文本 "**hello world**bold**\n**# Title\n"
[PASS] J1.5a :: 离线操作 A=29 B=29；合并顺序 A→B→A（应用 2/2）；字节 846 == 846：true；hash 0xdc2ba7a0d72036f5 / 0xdc2ba7a0d72036f5；文本 "hello worldbo!!ld\n# Title\n"
[PASS] J1.6a :: 离线操作 A=34 B=36；合并顺序 A→B→A（应用 9/7）；字节 1066 == 1066：true；hash 0xff5f9b52be0df8c1 / 0xff5f9b52be0df8c1；文本 "_lloaa worldbbold\n_$# TTitle\n$"
[PASS] J1.5-intent :: bold 父级=para(1 0)（期望 para）、bold 文本="bo!!ld"（期望 "bo!!ld"）；strong 存活=false
[PASS] J1.3-intent :: "hello" 被删、区间内的并发插入保留：文本="Y world"（期望 "Y world"）
```

- J1.2 的文本 `"helloXY world"` 说明同一间隙的并发插入由位置标识（数字随机）与字符标识共同定序，
  两端得到**同一个**交错结果；J1.2b 的哈希与 J1.2a 相同，说明与合并方向无关。
- J1.5-intent 是收敛之外的**语义意图**断言：A 解除包裹把 `bold` 搬到 `para` 下，B 同时往 `bold`
  节点里插入 `"!!"`；收敛后 `bold` 的父级是 `para`、文本是 `"bo!!ld"`。这直接验证"节点标识稳定，
  文本操作不因父级变化而失效"。
- J1.6 的渲染文本结构标记看起来错位（`"_lloaa worldbbold\n_…"`），这是**简化渲染器**把块级换行
  也包进行内标记所致，不是 CRDT 状态问题：判据比较的是规范状态字节，两端完全一致。

### J2 本地 undo 只撤销本地动作（全部用例）

```text
[PASS] J2.1 :: A 收到远端 1 个操作后 A 文本="helloxy world"；undo(skipped=0) 后 A 文本="helloy world"；未同步的 B 文本="helloy world"（B 未受 A 的 undo 影响）
[PASS] J2.2 :: A 收到远端 1 个操作后文本="y world"（"hello" 已被 A 删除、B 的 y 仍在）；undo(skipped=0) 后文本="helylo world"（恢复 5 个字符且 y 未被动过）
[PASS] J2.3 :: 两个独立本地动作 p、q；A 收到远端 1 个操作后文本="hellopyq world"；undo 一次(skipped=0) 后文本="hellopy world"（只撤 q，p 与远端 y 保留）
[PASS] J2.4 :: undo 后 A 操作总数=31；B 合并到补偿操作 3 个后文本="hellopy world"；字节 867 == 867：true；hash 0xcc5d4bde96e34dfa / 0xcc5d4bde96e34dfa
[PASS] J2.5 :: A 收到远端删除 2 个操作后文本=" world"；undo(skipped=2) 后 A 文本="heo world"（h/e/o 恢复、被 B 删掉的 l/l 跳过）；B 合并补偿后两端字节 808 == 808：true
```

- J2.1 同时验证"undo 只影响本地"：B 的副本没有收到 A 的补偿操作前，文本仍是 `"helloy world"`。
- J2.5 是新补的用例，覆盖**冲突跳过**路径：A 删除 `"hello"`，B 并发删除其中 `"ll"`；A 撤销时
  `l`、`l` 的存活寄存器已被 B 的新时间戳覆盖，指纹不匹配 → 跳过（`skipped=2`），只有 `h`、`e`、`o`
  恢复（`"heo world"`），即**远端删除的字符不会被本地 undo 复活**。这是
  `docs/HISTORY_COLLABORATION.md` §3 第 3 步"验证上下文指纹"的正面用例。

### J3 快照（全部用例）

```text
[PASS] J3.1 :: 规范状态 897 字节（hash 0xdd27870e185b4360）== 恢复后 897 字节（hash 0xdd27870e185b4360）：true；快照 1926 字节，恢复副本二次序列化字节相同：true
[PASS] J3.2 :: B 新增 5 个操作，快照前只投递 2 个（半程文本="AAhello worldBB**bold**\n# Title\n"）；恢复后补投 3 个、B 反向收到 2 个；字节 988 == 988：true；hash 0x9a8cd0e431b445d8 / 0x9a8cd0e431b445d8
[PASS] J3.3 :: B 新增 5 个操作，快照时只到了 2 个（半程文本="helloabba world**bold**\n# Title\n"）；恢复副本补 3 个、原副本补 3 个；字节 888 == 888：true；hash 0xfeae4cada4b5580e / 0xfeae4cada4b5580e；文本 "loabba world…"
[PASS] J3.4 :: 恢复副本新产生 3 个操作、B 收到 4 个；字节 897 == 897：true；B 文本="ZZQhello world"（hash 0x423f683d52811507 / 0x423f683d52811507）
[PASS] J3.5 :: 快照前文本="XYhello world"；恢复后 undo(acted=true, skipped=0) 得到 "hello world"（期望 "hello world"）
```

- J3.1 比判据更严：不只规范状态字节相同，`restore(s).snapshot() == s` 也逐字节成立（含时钟、序号、
  随机状态、操作日志与撤销栈）。
- J3.5 说明撤销栈随快照保留，恢复出的设备仍能撤销快照之前的本地动作（本 spike 的选择，正式设计未定，
  见"失败与不确定性"）。

### J4 规模

```text
[PASS] J4.1 :: 用户动作 100000（A=50228 / B=49772：插入 60204 删除 9960 属性 8030 包裹 9123 解除 7676 撤销 5007）；操作总数 153499（夹具引导 27）；节点 9130；文本条目 60224（可见 15425、墓碑 44799），最大分块数 89；最长位置标识 26 位；位置复用回退 1 次；总耗时 3.727423578s（37274 ns/动作）；两端收敛 true（hash 0x840de2e2a0d791d5）
[PASS] J4.2 :: 堆峰值增量 87.5 MiB < 上限 256 MiB；结束时堆占用增量 86.4 MiB；规范状态 2328862 字节
```

判据要求的是"用户动作"而不是 CRDT 内部操作：100 000 次用户动作展开为 153 499 个操作
（每次删除多个字符、每次包裹两个操作）。每 500 个动作同步一次（双向各 200 次），最后全量合并。
耗时与内存都来自单次运行。**同一台机器上重复运行观察到 J4 约 2.8–6.0 s 的波动**（其它构建任务并行时更慢），
下面引用的是 `artifacts/run-release.log` 中那一次的数字：

| 指标 | 值 |
|---|---|
| 用户动作 | 100 000（A 50 228 / B 49 772） |
| 展开的 CRDT 操作 | 153 499 |
| 总耗时 | 3.727 s（37 274 ns/动作，约 37.3 µs） |
| 堆峰值增量 | 87.5 MiB（结束时 86.4 MiB，上限 256 MiB） |
| 规范状态 | 2 328 862 字节 |
| 文本条目 / 可见 / 墓碑 | 60 224 / 15 425 / 44 799（墓碑占 74%） |
| 最长位置标识 | 26 位（26 个 `u16`，约 52 字节/字符） |
| 节点数 | 9 130 |
| 位置复用回退 | 1 次（并发包裹导致上下界相同的罕见情形） |

### J5 随机化收敛（节选）

```text
   J5.1 种子基准 0x5eed1234：100 轮 × 2 副本 × 每副本 200 个随机动作
[PASS] J5.1-round000 :: seed=0x000000005eed1234；新操作 A=141 B=111（乱序投递）；字节 5318 == 5318：true；hash 0x45b9eb4c774319f6 / 0x45b9eb4c774319f6；文本 "ymnvnhxenbuylxulocyjxyrtkdxxvlfhlbtspvavevyqmanxepgeondicg…"
[PASS] J5.1-round099 :: seed=0x000000005eed1297；新操作 A=127 B=124（乱序投递）；字节 5098 == 5098：true；hash 0x66b434ebc9d3e300 / 0x66b434ebc9d3e300；文本 "ibexcavahguuhdriejvxuyxtaukvwtlwzetmlaswuoxtllawczaeeiotnvciqa…"
   J5.2 种子基准 0x5eed9876：10 轮 × 3 副本 × 每副本 150 个随机动作
[PASS] J5.2-round000 :: seed=0x000000005eed9876；新操作 A=64 B=60 C=53（乱序投递）；三方字节 4180/4180/4180 相等：true；hash 0xa46e40860e21c815 / 0xa46e40860e21c815 / 0xa46e40860e21c815；文本 "rwvymogdihlsepgutkpirwrllyxwqlvkueivptlqlpwsobhf…"
[PASS] J5.2-round009 :: seed=0x000000005eed987f；新操作 A=45 B=65 C=61（乱序投递）；三方字节 4147/4147/4147 相等：true；hash 0x1d8e27066cc5102d / 0x1d8e27066cc5102d / 0x1d8e27066cc5102d；文本 "lsubehofkrkemhxwaalfwsphbmltjaidgblmlmvyunoyyjccplbqxoff…"
```

每个随机动作按固定比例取自：插入 60%、删除 10%、属性 8%、撤销 5%、包裹/解除 17%。每个副本的
"本轮新操作"各自洗牌后投递给其它副本，因此允许远端操作先于其因果前驱到达（删除先于被删字符的插入、
放置先于节点创建、存活位写入先于字符插入）。110 轮全部逐字节收敛；失败轮会打印种子与哈希，
本报告没有失败轮。

### 实现过程中被判据抓到的缺陷（保留记录）

J2.2 首次运行时 `skipped=5`、被删文本没有恢复：`TextCrdt::apply_alive` 比较了存活寄存器的时间戳，
但**没有把新时间戳写回寄存器**，于是指纹永远不匹配，且乱序到达的旧写入会覆盖新状态（LWW 失效）。
修好之后 J2.2 通过、J2.5 的冲突跳过路径也得到正确计数。这正是"判据逐用例断言"要抓的东西：
只看"undo 后 y 还在"的总数会漏掉"删除撤销整体失效"。

## 失败与不确定性

**结论标 Pass 的范围**：本报告只覆盖上面写定的 J1–J5 与全局门禁。以下全部属于**未测或已简化**，
不构成任何通过结论：

1. **第三方 CRDT 引擎没有比较。** 本 spike 按要求自研实现，没有引入 Loro / Yrs / Automerge，
   也没有实测它们的收敛、撤销、快照、规模与 `wasm32` 行为。其版本、许可证与代码依赖已由
   [报告 0013](0013-crdt-prescreen.md) 预筛，选型判据见 [ADR 0009](../adr/ADR-0009-crdt-engine.md)
   （Proposed）；本报告提供可移植到三者的**验收夹具**（J1–J5 与 136 条逐用例断言）
   与"自研基线至少需要什么"的参照，不能替代选型比较。
2. **结构树语义极简。** 只有"收敛"没有"语义正确"：并发地互相移动两个节点可能让父指针成环
   （渲染有 64 层深度上限，不会 panic，但没有冲突解决）；`wrap`/`unwrap` 由多个操作组成，
   **没有原子性**，只投递一半会得到结构性中间态；删除父节点不处理子节点归属；属性只有
   LWW 单值，没有列表/集合/引用类型；没有 `docs/DATA_MODEL.md` 的固定槽位、Raw/ForeignSource。
3. **撤销只做到设计中段。** 没有 Action 分组规则（空闲窗口、光标不连续、显式命令边界、
   远端操作改变锚点）；没有三方预览与用户确认（`docs/HISTORY_COLLABORATION.md` §3 第 4–5 步）；
   上下文指纹是"时间戳是否仍等于自己写入的那个"这一粗粒度判据，被跳过目标的**具体冲突内容
   没有呈现给用户**，只在计数里；撤销范围是设备本地（`fork` 不继承撤销栈），
   "换设备后继续撤销"未验证。
4. **文本 CRDT 的成本没有治理。** Logoot 的错位交错（interleaving anomaly）未处理（未引入
   LSEQ 类作保护）；墓碑只增不减（J4：44 799 / 60 224 = 74%），没有压缩、GC 或 checkpoint；
   反复在同一间隙插入会让位置标识变长（J4 最长 26 位），也没有 renumber 机制；因此"长期编辑
   同一文档"的存储与内存曲线**没有测**，只看了一次 10 万次动作的截面。
5. **同步协议没有实现。** 只有"全量操作日志求并集"：没有 state vector / 增量协商 / ACK /
   权限 epoch / 离线队列 / 断包与重复处理；`merge_from` 每次都要线性扫描对方的全部日志
   （10 万级操作时每次同步 O(15 万) 次哈希查询），服务端、持久化与网络故障不在本 spike 范围
   （属第 5、6 项）。
6. **快照没有治理。** 快照包含完整操作日志与撤销栈，体积随历史线性增长（J4 规范状态 2.3 MB，
   快照明显更大）；没有跨版本迁移测试，也没有损坏/截断输入恢复路径的正面用例
   （只有解码错误的分支代码，未构造损坏样本）。
7. **性能没有剖析。** 37.3 µs/动作是端到端数字：结构操作里的 `children()` 与 `next_sibling_pos()`
   每次都要遍历全部 9 130 个节点（`tree.rs`），没有为它建索引；没有逐阶段剖析，也没有与第三方
   库的对照基准。10 万次动作能完成不等于可交互。
8. **同一 actor 双设备写冲突未处理。** 恢复副本会从快照继续使用原 actor 的序号，因此
   J3.2–J3.4 必须冻结原副本；两台设备同时用同一 actor 写会撞序号。没有 actor 版本/epoch 或
   设备标识。
9. **字符模型是 `char` 不是字素簇。** 偏移单位是字符数；中文/emoji/组合字符的插入与删除边界、
   以及编辑器协议的 UTF-16 偏移换算没有夹具（`docs/DATA_MODEL.md` 要求区分这些单位）。
10. **J3 是点覆盖而不是全称覆盖。** "任意时刻可快照"只在 5 个构造点验证（含合并中途与恢复后
    继续编辑），没有做"每一步都快照一次"的穷举；J5 的随机交错是每轮一次置换，
    不是逐操作任意交错 + 部分合并的更强模型。
11. **内存阈值 256 MiB 是本 spike 自定**，不是产品要求；测的是主分配器计数器的堆增量，
    不含栈、内核页与（本 spike 不存在的）第三方运行时。

## 对设计的影响

1. **三层模型成立，可以继续。** 操作（可交换、可乱序、幂等）/ 动作（按 actor 撤销、生成补偿操作）/
   快照（可序列化、恢复后继续参与合并）三层在本 spike 上都能独立实现并各自通过判据，
   `docs/HISTORY_COLLABORATION.md` §1–§3 的核心主张不需要修改。
2. **"共享历史只追加"在实现上可行。** J2.4 证明补偿操作是普通操作，能同步给其他副本并保持收敛，
   撤销不需要回退共享状态。
3. **稳定节点标识的收益被实测。** J1.5-intent 说明结构移动与并发文本插入可以互不干扰，
   这支持 `docs/DATA_MODEL.md` §1 的稳定身份要求；它也意味着**文本操作不必携带父级路径**，
   协议负载可以更小。
4. **必须在 ADR 里写死三件治理机制**，否则自研或第三方引擎都会撞上：
   （a）墓碑压缩与 checkpoint 触发条件（本次 74% 墓碑）；
   （b）位置标识的 renumber/压缩策略（本次最长 26 位）；
   （c）撤销栈的持久化范围与跨设备语义（本次选择随快照保留、`fork` 不继承，属未验证决定）。
5. **快照不能只存文档状态。** 恢复出的设备必须能回答"我有哪些操作"，否则无法与别人求差集；
   本次把日志一起装进快照，代价是体积线性增长。ADR 需要决定日志截断与 state vector 的组合。
6. **CRDT engine ADR 的验收夹具已就绪。** `spikes/crdt-race/` 的 J1–J5 场景与逐用例输出格式
   可以直接改写成对 Loro / Yrs / Automerge 的同一套测试；ADR 仍需补：许可证与依赖树、
   Rust/WASM 绑定与二进制体积、离线打包、文档格式稳定性、与 Action 层的边界（谁能提供
   "只撤销自己"的原语）。**本 spike 不改变该 ADR 的"缺文件"状态。**
7. **阶段 0 第 4 项的出口条件"两端收敛且本地撤销保留远端输入"已有可复现证据**；
   但"10 万次动作"只能作为"机制可承受"的结论，性能预算（p95 延迟、交互期内存上限）
   需要在选定引擎后单独测，不能引用本次的 37.3 µs 作为产品指标。

## 复现步骤

```bash
# 1. 进入 spike
cd spikes/crdt-race

# 2. 使用仓库内已缓存的 cargo 目录，离线构建
export CARGO_HOME=/home/ation_ciger/Projects/Mogan/scholium/spikes/native-ui/.cargo-home
cargo build --offline
cargo build --release --offline

# 3. 跑证据（判据 J1–J5 的 136 条逐用例判定；退出码 0 表示全部通过）
cargo run --release --offline | tee artifacts/run-release.log

# 4. 可选门禁
cargo fmt --check
cargo clippy --offline --all-targets
cargo test --offline
```

预期结果：stdout 以 `=== Scholium 阶段 0 第 4 项验证：CRDT 赛马 ===` 开始，
以 `用例总数 136，通过 136，失败 0` 结束；`cargo build` 与 `cargo run --release` 无警告；
命令 `echo $?` 为 0。机器不同则 J4 的耗时与内存数字会变，收敛性与用例通过数不变
（所有随机性都来自报告里列出的固定种子）。
