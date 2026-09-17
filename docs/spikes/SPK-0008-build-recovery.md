# Spike 0008：阶段 0 第 5 项 — 构建/恢复

> **有效性（2026-09-17 登记）**：阶段 0 第 5 项的原始证据；本报告不固化选型（见其关联 ADR 说明）。阶段 0 当前判定见 [报告 0012](SPK-0012-exit-criteria.md)。

- 结论：**Pass（机制；四条判据全部逐用例通过，但有一项实现缺口见下）**
- 对应验证项：[路线图阶段 0 第 5 项](../plan/ROADMAP.md)
- 日期：2026-09-16
- 执行者：ation_ciger
- 关联 ADR：尚无（本报告不固化选型；若把 WAL 记录格式与"拒绝静默覆盖"写成正式契约，需新 ADR）；[0008 源码 reconcile 策略与构建工具链隔离](../adr/ADR-0008-reconcile-and-toolchain-isolation.md)（隔离构建部分）
- 代码：[`spikes/recovery/`](../../spikes/recovery)
- 证据日志：本报告引用的全部输出来自一次连续运行，命令见"复现步骤"

## 问题与判据

要关闭的"待验证"是：**在两种引擎（LaTeX / Typst）并存、且崩溃与外部改动都可能发生的前提下，
构建隔离、草稿恢复与冲突拒绝这三条机制能不能真的成立。** 判据在动手前写定如下，事后未调整。

### 判据 1：两种引擎隔离构建（4 条）

1.1 同一份文档分别用 LaTeX 与 Typst 构建互不干扰：中间文件与产物分目录隔离，
源码目录里除源码与**被命名的隔离子目录**外不得出现任何引擎产物。
1.2 能并发跑而不互相覆盖：两条路径同时在各自目录构建，产物内容签名必须与各自串行基线一致；
同一输出目录的第二个写者必须被拒绝。
1.3 构建 A 不会读到 B 的中间文件：在 B 的目录放入**同名且语义相反**的 `main.aux`
（声明 999 页），A 的产物内容签名必须与干净基线逐字节一致。
1.4 负向对照：必须证明"签名一致"不是因为签名恒定——改动 A 自己的源码后签名必须变化。

### 判据 2：WAL 与未提交草稿恢复（3 种情况）

记录格式为 append-only，每条带长度与 CRC-32。写入过程用**子进程 + SIGKILL** 制造崩溃。

2.1 正常关闭：全部记录可恢复，无尾部缺陷，无字节被丢弃。
2.2 尾部截断：只恢复到最后一个**完整**记录，尾部部分写入的记录被丢弃并给出可读分类；
再细分为"撕裂落在负载中间"与"撕裂落在头中间"两个子用例。
2.3 尾部字节损坏：CRC 必须拦住，恢复停在损坏记录之前，被丢弃的正好是那一条。
2.4 能力上界（负向用例）：把某条记录的负载与 CRC **一起**改成自洽的组合后，
恢复器**应当**接受改写并照常恢复——用于把"CRC 防随机损坏、不防蓄意改写"这条边界
变成可复现证据，而不是一句免责声明。

### 判据 3：外部源码修改冲突

源码文件被外部改动（内容哈希变化）后，应用必须检测到冲突并**拒绝静默覆盖**，
给出可读报告（含加载哈希、当前哈希、差异行）。同时必须有反向判据：无人改动时保存必须成功，
否则"拒绝一切写入"也能骗过这条判据。

### 判据 4：随机截断点

对日志做 24 轮随机截断/随机字节损坏（固定种子，种子写进报告），每一轮独立断言：
恢复条数等于截断点之前完整的记录条数、`good_bytes` 落在真实记录边界、
丢弃字节数 = 文件长度 − 边界、恢复内容与基线**逐字节**前缀一致、
只有切在记录之间时才允许没有缺陷。

### 判定口径

结论按**每个用例**判定（15 个用例，每个用例的断言必须全部通过），不看断言总数——
总数会掩盖个别用例静默失效。任何断言失败即该用例 Fail；任何用例 Fail 即判据 Fail。

## 环境

- 平台：Arch Linux，Linux 7.2.4-arch1-2，x86_64
- 工具链：`cargo 1.98.0-nightly (a595d0da7 2026-06-20)`、`rustc 1.98.0-nightly (bd08c9e69 2026-06-25)`，edition 2024
- LaTeX：TeX Live 2026（Arch 打包）；证据行由引擎自己打印：
`Latexmk, John Collins, 15 June 2025. Version 4.87`；`XeTeX 3.141592653-2.6-0.999998 (TeX Live 2026/Arch Linux)`
- 字体：`Noto Sans CJK SC`（源码里显式 `\setCJKmainfont` 钉死，避免 ctex 自动探测导致分页漂移）
- Typst：`typst 0.15.1` / `typst-layout 0.15.1` / `typst-kit 0.15.1`
（features `embedded-fonts`、`scan-fonts`），与阶段 0 第 2 项同版本
- 其它锁定依赖（`Cargo.lock`）：`thiserror 2.0.20`、`crc32fast 1.5.2`、`sha2 0.10.9`、
`rand 0.9.5`、`libc 0.2.189`、`scholium-spike-core 0.0.0`（path，复用第 1 项的文档模型）
- 许可证：全部依赖落在 `AGENT.md` 允许清单内（MIT / Apache-2.0 / ISC）；
无 npm / Node.js / WebView 依赖
- 构建与运行命令：

```bash
export CARGO_HOME=/home/ation_ciger/Projects/Mogan/scholium/spikes/native-ui/.cargo-home
cargo build --release --offline --manifest-path spikes/recovery/Cargo.toml
cargo run  --release --offline --manifest-path spikes/recovery/Cargo.toml
```

工作目录默认 `/tmp/scholium-recovery-workspace-r0`；`SCHOLIUM_SPIKE_WORKSPACE` 可改根目录，
`SCHOLIUM_SPIKE_KEEP=1` 保留现场，`SCHOLIUM_SPIKE_REPEAT=2` 跑两轮并逐用例比对。

## 方法与夹具

- **文档模型**：复用 `scholium-spike-core` 的标准夹具（中英混排段落 + 分数 + 上下标 + 矩阵 + 根式），
两条构建路径吃**同一份** SDG，否则"同一份文档、两种引擎"无从谈起。
- **渲染器**：`render.rs` 只覆盖上述最小子集，不含完整转义；把渲染器做小，
判据失败时才不会和"转义没做全"混在一起。
- **隔离模型**：源码在 `project/<engine>/main.<ext>`，中间文件与产物在 `project/<engine>/.build/`；
构建前后对源码目录**枚举文件名**，多出任何非预期项即算泄漏。
- **WAL 格式**（小端）：`"SWAL" | total_len:u32 | seq:u64 | epoch:u64 | payload_len:u32 | crc32:u32 | payload`，
固定头 32 字节，CRC 覆盖 `[4,28)` 加负载（即除魔数外的整条记录）。
每条记录写完立即 `fsync`。
- **恢复语义**：从文件头顺序扫描，遇到第一条不完整或校验失败的记录即停止，其后字节一律丢弃。
不做"跳过坏记录继续扫"（理由见"对设计的影响"）。
- **崩溃夹具**：主进程用 `current_exe()` 以 `writer` 子命令启动自己；子进程写完 `N-1` 条完整记录后，
分块写第 `N` 条并在写完 `kill-after-chunks` 块后 `raise(SIGKILL)`。
主进程只负责重新打开日志并判断恢复到哪一条。撕裂点参数：负载撕裂 `chunk=40, kill-after=1`
（记录 76 B，只写下 40 B）；头撕裂 `chunk=20, kill-after=1`（只写下 20 B < 头长 32 B）。
- **随机截断**：200 条记录、基准日志 13 379 B，种子 `20260916`，24 轮（12 轮截断 + 12 轮损坏），
前 4 轮用固定边界位置（文件末尾、倒数第一字节、0、第 100 条记录边界；魔数首字节、CRC 首字节、
某条记录负载首字节、文件末字节），其余用 `SmallRng::seed_from_u64`。
- **产物签名**：xelatex 会把运行时间与随机 PDF ID 写进 PDF，同一份源码两次编译的**字节**不同，
因此判据不用文件哈希，而用"跨运行稳定的内容签名"（`pdftotext` 文本 + 页数的 SHA-256；
Typst 的文本产物直接取文件哈希）。文件哈希仍打印出来作对照。

### 夹具本身踩过的三个坑（都会把用例测成假通过）

1. **`OpenOptions` 只开 `.append(true)` 不隐含可写**：崩溃子进程的 `write_all` 立刻以
`Bad file descriptor` 失败，子进程带退出码 1 退出——`signal` 是 `None` 而不是 9，
看起来像崩溃，其实是夹具自己报错。
2. **`bytes.chunks(k)` 不整除时循环会写完整条记录**：夹具"写满循环再自杀"，
于是 76 B 的记录被 40 B 分块写成 `[40, 36]` 两块，记录完整落盘，
"尾部撕裂"用例静默退化成"正常关闭"。
3. **控制组把文字追加到 `\end{document}` 之后**：页面上什么都不出现，签名当然不变，
"负向对照"变成永远通过的空断言。

三条都靠断言失败暴露（第 2 条第 3 条最初表现为"个别用例静默失效"），
修法是：显式 `write(true)`、显式 `kill-after-chunks` 参数并在夹具里直接断言"必须停在记录中间"、
控制组插到 `\end{document}` 之前。这正是"逐用例判定"要抓的东西。

## 结果

一次连续运行的逐用例汇总（完整输出共 267 行 `[PASS]`、0 行 `[FAIL]`）：

```text
== 逐用例汇总 ==
  [PASS] build.isolated-outputs             14/14 断言通过
  [PASS] build.decoy-isolation              6/6 断言通过
  [PASS] build.concurrent-paths             6/6 断言通过
  [PASS] build.source-fidelity              3/3 断言通过
  [PASS] build.output-lock                  3/3 断言通过
  [PASS] wal.encoding                       6/6 断言通过
  [PASS] wal.normal-close                   6/6 断言通过
  [PASS] wal.tail-truncated                 12/12 断言通过
  [PASS] wal.tail-corrupted                 7/7 断言通过
  [PASS] wal.synthetic-rewrite-boundary     3/3 断言通过
  [PASS] conflict.external-change           8/8 断言通过
  [PASS] conflict.no-external-change        4/4 断言通过
  [PASS] conflict.unchanged                 2/2 断言通过
  [PASS] conflict.removed                   3/3 断言通过
  [PASS] truncate.random-rounds             169/169 断言通过

本轮总结论: PASS（用例 15 个独立判定）
```

两轮独立运行（不同目录、不同进程）的逐用例汇总逐行一致：

```text
== 两轮一致性 ==
  [PASS] 第 1 轮与第 2 轮逐用例汇总完全一致
```

| 判据 | 结果 | 证据 |
|---|---|---|
| 1.1 分目录隔离 | Pass | `latex.源目录无中间文件泄漏`（内容 `[".build","main.tex"]`，多余项 `[]`）、`typst.源目录无中间文件泄漏`（`[".build","main.typ"]`）、`latex.确实产生了中间文件`（`main.aux/fdb_latexmk/fls/log`）、`typst.不产生中间文件` |
| 1.2 并发不互相覆盖 | Pass | `并发 latex 产物签名与串行基线一致 — c0537db58e055d50 vs c0537db58e055d50`、`并发 typst … 1b97928643967068 vs 1b97928643967068`、`同目录并发写被锁拒绝` |
| 1.3 A 不读 B 的中间文件 | Pass | 毒饵 `main.aux` 声明 999 页仍 `A 重编产物签名与干净基线一致 — c0537db58e055d50 vs c0537db58e055d50`；`B 的毒饵没有被 A 的构建改写 — 毒饵仍为 46 B` |
| 1.4 负向对照 | Pass | `对照：A 自己的源码变化会改变产物签名 — 对照 6eb1371e5af0aaa7 vs 基线 c0537db58e055d50` |
| 2.1 正常关闭 | Pass | `恢复正常关闭日志 — 文件 527 B，恢复 8 条，good_bytes=527，defect=None`、`没有字节被丢弃 — discarded=0` |
| 2.2 尾部截断 | Pass | `子进程被 SIGKILL 杀死 — code=None signal=Some(9)`；`崩溃现场 — 日志 274 B：完整 234 B + 尾部残缺 40 B`；`恢复到崩溃前的最后一条完整记录 — 4 / 4`；`缺陷分类为负载截断 — TruncatedPayload { declared: 76, remaining: 40 }`；头撕裂：`TruncatedHeader { remaining: 20 }`，同样 `4 / 4` |
| 2.4 协同改写边界（负向） | Pass | `协同改写 — 第 2 条负载改为 23 字节的 'Z' 并同步重算 CRC — 恢复 4 条，defect=None`；`恢复出的负载是改写后的内容 — 前 8 字节=[90,90,90,90,90,90,90,90]`；`同一条记录前后的记录不受影响` |
| 2.3 尾部损坏 | Pass | `缺陷分类为 CRC 不匹配 — expected 0xf54bc89d / actual 0xd8492710`；`恢复到损坏记录之前的那一条 — 6 / 6`；`被丢弃的正好是那条损坏记录 — 90 / 90` |
| 3 外部冲突拒写 | Pass | `attempt=Rejected(ExternalChange { loaded: "ef82726c…", current: "92745178…" })`；`磁盘内容仍是外部版本（未被静默覆盖） — 磁盘 423 B / app 想写 402 B`；`磁盘文本不含 'app edit'`；报告含 `第 12 行 … + 现在: % external edit by another tool` |
| 3 反向（无冲突必须能写） | Pass | `无人改动时保存成功写盘 — Written { bytes: 412 }`；`内容未变时归类为 unchanged`；`文件被删除时归类为 removed` |
| 4 随机截断 | Pass | 种子 `20260916`，`基准日志长度等于全部记录长度之和 — 13379 B / 200 条`；24 轮全部 169 条断言通过，每轮 `good_bytes` 均等于真实边界、`丢弃字节数 = 文件长度 − 边界` |

### 判据 1 关键输出

```text
  [----] latex 构建 — 1229 ms / 11610 B / 页数 Some(1) / 产物 …/project/latex/.build/main.pdf / Latexmk, John Collins, 15 June 2025. Version 4.87；XeTeX 3.141592653-2.6-0.999998 (TeX Live 2026/Arch Linux)
  [----] latex 输出目录 — main.aux:fbabe80c main.fdb_latexmk:67c05602 main.fls:0815d870 main.log:a7b9cc68 main.pdf:05d8d0f1 main.tex:ef82726c main.xdv:699e6188
  [PASS] latex.拿到了跨运行稳定的产物签名 — signature=Some("c0537db58e055d50")；产物文件哈希 05d8d0f1d6086578（PDF 含时间戳，文件哈希跨运行会变，判据用签名）
  [PASS] latex.源目录无中间文件泄漏 — 目录=…/project/latex 内容=[".build", "main.tex"] 多余项=[]
  [----] typst 构建 — 1832 ms / 77 B / 页数 Some(1) / 产物 …/project/typst/.build/main.out / typst 0.15.1 进程内编译；warnings=0；page-hash=bf9e51825abfe52e
  [PASS] typst.不产生中间文件 — 沙箱文件=["LOCK", "main.out", "main.typ"]
  [PASS] 两条路径的沙箱内容不同（目录确实分离） — latex=["LOCK","main.aux","main.fdb_latexmk","main.fls","main.log","main.pdf","main.tex","main.xdv"] typst=["LOCK","main.out","main.typ"]
  [----] 毒饵 — …/project/typst/.build/main.aux ← 内容 "\\relax\n\\@setckpt{999}{\\setcounter{page}{999}}"（原 aux 32 B，语义声明 999 页）
  [PASS] A 重编产物签名与干净基线一致（没读到 B 的毒饵） — 基线 c0537db58e055d50 vs 有饵 c0537db58e055d50
  [PASS] 对照：A 自己的源码变化会改变产物签名（一致性不是恒等） — 对照 6eb1371e5af0aaa7 vs 基线 c0537db58e055d50
  [----] 并发耗时 — latex=203 ms typst=905 ms
  [PASS] 并发 latex 产物签名与串行基线一致 — c0537db58e055d50 vs c0537db58e055d50
  [PASS] 并发 typst 产物签名与串行基线一致 — 1b97928643967068 vs 1b97928643967068
  [PASS] 同目录并发写被锁拒绝 — 锁=…/latex/.build/LOCK 结果=Some("构建目录已被占用")
  [PASS] 被拒绝的构建没有在输出目录留下任何文件 — ["LOCK"]
  [PASS] 释放锁后重建成功且产物签名与对照一致 — fa9608330cc38141 vs fa9608330cc38141
```

关于**并发延时**的一句诚实记录：`latex=203 ms` 比串行首次编译（1229 ms）小一个量级，
这是 latexmk 判定无需重跑（`fdb_latexmk` 记录）的结果，不是"并发让编译变快"。
并发用例证明的是**互不覆盖**，不是加速；产物签名仍与基线一致才是证据。

### 判据 2 关键输出

```text
  [PASS] 子进程正常退出（退出码 0） — code=Some(0) signal=None
  [----] 恢复正常关闭日志 — 文件 527 B，恢复 8 条，good_bytes=527，defect=None
  [PASS] 没有字节被丢弃 — discarded=0
  [PASS] 子进程被 SIGKILL 杀死（不是返回错误） — code=None signal=Some(9)
  [----] 崩溃现场 — 日志 274 B：完整 234 B + 尾部残缺 40 B
  [PASS] 恢复到崩溃前的最后一条完整记录 — 4 / 4
  [PASS] 缺陷分类为负载截断 — Some(TruncatedPayload { declared: 76, remaining: 40 })
  [PASS] 恢复点等于已恢复记录的长度之和（只能落在记录边界上） — good_bytes=234 条数=4
  [PASS] 截断修复后再次恢复：无缺陷且记录不变 — defect=None 条数=4 文件=234 B
  [PASS] 头撕裂场景中子进程也被 SIGKILL — signal=Some(9)
  [PASS] 缺陷分类为头截断 — 残缺 20 B，Some(TruncatedHeader { remaining: 20 })
  [PASS] 头撕裂时仍恢复到同一完整边界 — 4 / 4
  [PASS] 子进程写完后被 SIGKILL（保留坏负载文件） — signal=Some(9)
  [----] 损坏现场 — 日志 483 B；恢复 6 条；defect=CRC 不匹配: 头内 0xf54bc89d vs 实算 0xd8492710
  [PASS] 恢复到损坏记录之前的那一条 — 6 / 6
  [PASS] good_bytes 停在损坏记录起点 — 393 / 393
  [PASS] 被丢弃的正好是那条损坏记录 — 90 / 90
  [PASS] 丢弃损坏记录后，之前的记录逐字节完好 — 末条 seq=6
  [----] 协同改写 — 第 2 条负载改为 23 字节的 'Z' 并同步重算 CRC — 恢复 4 条，defect=None
  [PASS] 协同改写（含 CRC）确实不会被结构性校验拦住 —— 这是已知能力上界 — 恢复 4 条，defect=None
  [PASS] 恢复出的负载是改写后的内容（数据确实被替换，而不是被丢弃） — 前 8 字节=[90, 90, 90, 90, 90, 90, 90, 90]
  [PASS] 同一条记录前后的记录不受影响 — 第 1 条 seq=1 第 3 条 seq=3
```

### 判据 3 关键输出

```text
  [PASS] 保存被拒绝，原因是外部改动 — attempt=Rejected(ExternalChange { loaded: "ef82726c…", current: "92745178…" })
  [PASS] 磁盘内容仍是外部版本（未被静默覆盖） — 磁盘 423 B / app 想写 402 B
  [PASS] 磁盘上找不到 app 的改动 — 磁盘文本不含 'app edit'
源码冲突报告: …/conflict/external/main.tex
  判定: external-change
  加载时哈希: ef82726ccd23e2d5cd8f18daf7e6f1e27bef4c64121ecd848ddc454dc4bcc0d7
  当前哈希:   92745178d87962d8c4c1d8bf351673d9a7ce6c4013a1e8dbd0448dd69973b525
  动作: 拒绝写入（磁盘内容由外部改动，不静默覆盖）
  差异行数: 1
    第 12 行（首个差异在第 0 个字符）
      - 加载:
      + 现在: % external edit by another tool
```

### 判据 4 关键输出（种子 20260916）

```text
种子=20260916，轮数=24，记录=200
  [PASS] 基准日志长度等于全部记录长度之和 — 13379 B / 200 条
  [轮  1/24] round00(truncate@13379) 文件 13379 B → 恢复 200 条 / 边界 13379 B / 丢弃 0 B / 无
  [轮  2/24] round01(corrupt@28)     文件 13379 B → 恢复   0 条 / 边界     0 B / 丢弃 13379 B / CRC 不匹配
  [轮  3/24] round02(truncate@0)     文件     0 B → 恢复   0 条 / 边界     0 B / 丢弃 0 B / 无
  [轮  4/24] round03(corrupt@13315)  文件 13379 B → 恢复 198 条 / 边界 13260 B / 丢弃 119 B / CRC 不匹配
  [轮  5/24] round04(truncate@5213)  文件  5213 B → 恢复  78 条 / 边界  5208 B / 丢弃 5 B / 尾部头不完整（剩余 5 B）
  [轮  7/24] round06(truncate@7804)  文件  7804 B → 恢复 117 条 / 边界  7803 B / 丢弃 1 B / 尾部头不完整（剩余 1 B）
  [轮 11/24] round10(truncate@979)   文件   979 B → 恢复  14 条 / 边界   938 B / 丢弃 41 B / 尾部负载不完整（声明 93 B）
  [轮 24/24] round23(corrupt@7582)   文件 13379 B → 恢复 112 条 / 边界  7495 B / 丢弃 5884 B / CRC 不匹配
  [----] 轮次分布 — 截断 12 轮 / 损坏 12 轮，种子均为 20260916
```

## 失败与不确定性

**结论是 Pass，但下面每一项都是真实缺口，不要当成"已经实现"。**

1. **Typst 路径没有产出 PDF。** 依赖清单里没有 `typst-pdf`，Typst 产物是
"编译成功 + 页数 + 页内容哈希"的文本摘要（`main.out`，77 B），**不是 PDF**。
"两种引擎各自产出可比 PDF"这件事只测了 LaTeX 一半。这是本报告最大的简化。
2. **夹具只有 1 页、1 段。** 两种引擎的分页、多轮 latexmk 收敛、跨页中间文件都没测到。
3. **隔离靠夹具目录，不是产品的真实写盘路径。** 产品会用"用户项目目录 + `-outdir`/jobname"，
本 spike 每次构建都把源码复制进沙箱。真正的风险（用户目录里已有的 `.aux` 被引擎读取）
没有被覆盖，只覆盖了"两个沙箱之间互不读取"。
4. **PDF 字节跨运行不确定。** xelatex 写入时间戳与随机 PDF ID，因此判据用
"`pdftotext` 文本 + 页数"的签名。**签名只对文本敏感**：只改字体、间距、颜色而不改文字的
变化，签名不会变。这是签名法的已知盲区。
5. **SIGKILL ≠ 断电。** 子进程被 `SIGKILL` 后，它已经 `fsync` 的字节由内核负责落盘，
本 spike 没有、也无法在用户态验证"掉电时写入到一半的扇区"。真实的断电语义
（页缓存丢失、文件系统日志重放、`fsync` 前数据未落盘）**没有测**。
6. **没有并发写同一个 WAL。** "两个实例同时追加同一份草稿日志"未验证（正式实现需要单写者）。
7. **没有测真实草稿负载。** WAL 负载是构造的字符串，不是 SDG/用户动作的序列化；
记录格式的稳定性（版本、epoch、迁移）完全没有覆盖。
8. **"epoch/语言门禁"没测。** 记录里带 `epoch` 字段，但恢复时没有验证"旧 epoch 不得回灌"。
9. **冲突检测只有内容哈希。** 没有测合并路径、非 UTF-8 源码、只读文件系统、权限错误、
多实例同时保存。判据只说"拒绝静默覆盖"，没说怎么继续，这是有意的，但也是缺口。
10. **判据 4 只做了单次损坏。** 没测"截断 + 损坏同时发生"、多次损坏、日志规模跨量级
（当前 13 KB / 200 条）。
11. **项目级门禁只跑了一部分。** 已跑：`cargo build --release --offline` 与
`cargo run --release --offline` **零警告**（含 `lints.rust`/`lints.clippy` 的
`missing_docs`、`unwrap_used`、`too_many_lines` 等），`cargo clippy --release --all-targets`
**零警告**（零 `to_string`/`format!`/单字符 `push_str` 类风格问题；`render.rs` 里一处
`clippy::single_char_add_str` 被显式 `allow`，因为它建议的改法会丢掉 `\left`，
是语义变化而不是风格问题）。
**未跑**：`cargo fmt --check`（本仓库还没有 `rustfmt.toml`）、`cargo doc`、
`cargo deny`、`cargo audit`、`cargo crap` 复杂度门禁、覆盖率。
spike 是独立 workspace，正式 crate 落地时仍需过这些门禁。
12. **只在 Linux/x86_64 上跑过。** Windows/macOS 的文件系统语义、`libc::raise`
可用性、TeX 发行版差异都没验证。
13. **未做**：恢复后自动修复（`truncate_to_good` 只在用例里被调用）、草稿与共享历史的整合、
GUI、构建性能基准。

## 对设计的影响

1. **"两种引擎隔离构建"应当实现为"每次构建一个独占的中间目录 + 排他锁"**，
而不是"共用项目目录、靠 `-outdir` 区分"。本 spike 证明前者成立（1.1–1.3 全过），
但没有证明后者；产品若选后者，需要另一组夹具（在工作目录预置同名 aux/fls 再断言不泄漏）。
2. **产物比对不能用文件字节。** LaTeX 产物含时间戳与随机 ID，任何"构建可复现"的验收
都必须定义"内容签名"口径（文本/页数，或规范化后的 PDF 结构）。这条会直接影响
阶段 1 的"保存—重开—输出闭环"验收脚本，建议写进测试规范。
3. **WAL 恢复语义定为"顺序扫描、首条不完整即停、其后全丢"**，不做坏记录跳过与重同步。
理由：跳过坏记录会把物理上更晚的状态当成有效状态，恢复结果不可解释。
代价是"中间一条损坏会丢掉后面所有已提交记录"，正式实现需要**分段日志或 checkpoint**
来限制单条损坏的影响范围——这是需要在架构里补的，不是实现细节。
4. **记录头必须让 CRC 覆盖长度字段本身**。本 spike 的格式做到了（CRC 覆盖 `[4,28)` 含
`total_len` 与 `payload_len`）。但**负载与 CRC 被一起改写成自洽组合时，恢复器会照单全收**：
用例 `wal.synthetic-rewrite-boundary` 把第 2 条记录负载全改成 `'Z'`、同步重算 CRC，
恢复结果 4 条、`defect=None`、负载确实被替换、前后记录不受影响。
结论：**CRC 只防随机损坏与撕裂，不防蓄意改写，也不提供完整性保护**。
正式实现若需要防改写，必须另加机制（对前缀做哈希链/带密钥的 MAC，或对每段做校验和）；
若只把 WAL 当作崩溃恢复工具（本 spike 的定位），可以接受这个边界，但**必须写进模块文档**，
不能让人以为 CRC 顺带保证了完整性。
5. **"拒绝静默覆盖"应作为保存路径的默认行为**，不接受"覆盖"作为可选参数。
本 spike 的保存只有一个出口：哈希一致才写；外部改动只能走显式的接受动作。
这个默认值应当在 `docs/modules/` 的存储/源码模块里写死，而不是留给调用方。
6. **`fsync` 是恢复正确性的前提**，不是优化。每条记录写完立即 `fsync`；
产品实现若为了吞吐批量提交，必须同时给出"批量窗口内崩溃会丢什么"的语义说明。
7. **后续需要新 ADR 的候选**：WAL 记录格式与恢复语义（含分段/checkpoint）、
"内容签名"作为构建可比性的定义、外部冲突的接受/合并流程。
8. **阶段 0 第 5 项的出口条件**目前只到"机制成立"。要在阶段 1 出口签字，
至少还需补：Typst 真 PDF、多页夹具、产品真实写盘路径、断电语义的另行验证。

## 复现步骤

从干净环境开始：

```bash
cd /home/ation_ciger/Projects/Mogan/scholium
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"

# 1) 编译（独立 workspace，离线可复现；首次约 90 s）
cargo build --release --offline --manifest-path spikes/recovery/Cargo.toml

# 2) 跑全部判据并打印证据（约 5 s，退出码 0 表示全部用例通过）
cargo run --release --offline --manifest-path spikes/recovery/Cargo.toml
```

期望输出：4 个判据小节、14 行逐用例汇总全部 `PASS`，
`本轮总结论: PASS（用例 14 个独立判定）`，进程退出码 0。

```bash
# 3) 两轮一致性（不同目录、不同进程，逐用例汇总逐行相同）
SCHOLIUM_SPIKE_REPEAT=2 cargo run --release --offline --manifest-path spikes/recovery/Cargo.toml

# 4) 保留现场排查（默认在 /tmp 下自动删除）
SCHOLIUM_SPIKE_KEEP=1 cargo run --release --offline --manifest-path spikes/recovery/Cargo.toml

# 5) 单独复现崩溃夹具（父进程之外手工验证 SIGKILL 与尾部残缺）
BIN=spikes/recovery/target/release/scholium-spike-recovery
rm -rf /tmp/wal-manual && mkdir -p /tmp/wal-manual
"$BIN" writer --wal /tmp/wal-manual/draft.wal --mode crash --crash-at 5 --chunk 40 --kill-after-chunks 1
# 期望：进程被 SIGKILL（shell 报 exit 137），日志 274 B = 4 条完整记录 234 B + 残缺 40 B
"$BIN" writer --wal /tmp/wal-manual/draft.wal --mode normal --count 8   # 对照：正常退出，退出码 0
```

环境变量：`SCHOLIUM_SPIKE_WORKSPACE`（根目录，默认 `/tmp/scholium-recovery-workspace`）、
`SCHOLIUM_SPIKE_KEEP=1`、`SCHOLIUM_SPIKE_REPEAT=2`。
