# T41：Loro SDG adapter 与四版本性能复审

任务基线 `4bb9808ecf79aca8e17d7ba5247f4603688e8b8e`，工作槽 `scholium-wt2`，
分支 `ation_ciger/feat/loro-sdg-adapter`。依据 ADR 0009、0041、0042、0049 与任务停止规则执行。

## 当前结论（协调者授权续作后）

Loro 1.17.1 满足预先给定的两项标准，继续并完成独立 adapter：正式三轮最坏 batch-100 接收 p99
**9.270 ms ≤ 16 ms**；10k/端离线合并最坏两向合计 **105.555 ms ≤ 500 ms**。
没有切换引擎，没有实现生产 Yrs。稳定 NodeId 映射、Action/snapshot v1、结构冲突修复、actor undo/redo、
快照+乱序/重复 replay、固定种子随机交错已实现；未接入 app/session/storage。
下面的首轮停止记录是历史证据，后面的续作记录与 ADR 0009 修订为当前裁决。

## 首轮问题与结论（历史）

阶段 0 选定 Loro 的证据只覆盖功能，没有十万动作性能比较。
本次 Loro 的删除/移动密集负载合并明显慢于 Yrs，**触发 T41 明确停止条件**：
保留基准、门禁与负面结果，停止生产 adapter/Action payload 实现，不自行切换引擎。
这不是「adapter 已完成」；没有生产结构并发修复、actor undo 或永久 Action 格式。

## 改动

新增层 2 `scholium-sdg` 边界与可复现 benchmark；没有生产 API。
只依赖 Loro 进行原生/WASM 引擎编译，Yrs 和 JSON 输出仅供 dev/example；
不依赖 session/app/storage/build/latex，也没有修改它们或 model/document。
Cargo.toml、层级表、模块索引只追加该 crate；Cargo.lock 由 cargo 生成。
ADR 0009 追加性能复审结论；collab 规范明确基准不能成为生产映射。
未新增 ADR 0053：引擎性能复审尚未裁决，不能提前冻结映射或 Action 格式。

## 方法与复现

代码及完整测量边界见 [crate README](../../crates/scholium-sdg/README.md)。

```sh
cargo test --locked -p scholium-sdg --example engine_bench
cargo build --release --locked -p scholium-sdg --example engine_bench
python3 crates/scholium-sdg/examples/run-engine-bench.py \
  target/release/examples/engine_bench target/engine-bench-new
```

同一份 32 段、160 个节点夹具，两 actor 各执行 5000 个十动作循环，合计十万动作。
操作包含正文 Unicode 插删、数学 symbol 插删、radical 包裹/解除、数学子树及正文 inline 移动。
Yrs 使用稳定 id map + parent/order/alive + YText；Loro 使用 movable tree/meta + LoroText。
每个引擎每次独立进程，五轮交替先后，计时只涵盖指定阶段；不把初始化、预生成 trace 或投影混入动作吞吐。
所有十轮检查身份/类型/父级/有序子节点/文字的完整期望状态、双端收敛与快照往返。
[前置测试](evidence/2026-11-03-loro-sdg-adapter/benchmark-tests.log) 比较两种后端的中间状态和 Unicode 删除语义。

环境：Arch Linux 7.2.11、i7-13650HX、Rust 1.99.0，release 优化构建。
同机有其他执行者，绝对耗时不作产品承诺；monotonic/boottime 样本差均小于 1 秒，无挂起污染。
当前稳定版核对为 Loro 1.17.1 / Yrs 0.28.0；本次保持 ADR 0009 的 1.16.0 / 0.27.4，未升级引擎。
Loro 1.16.0 自动解析 internal 1.17.1 会编译失败，cargo 将 internal 降回 1.16.0；
common/kv-store 1.17.0 与其编译兼容，精确依赖图由 lockfile 固定。

首次探针暴露 Yrs 默认 Bytes 索引不能以长度 1 删除 `é`；改为显式 UTF-16 并补中间状态断言。
初版仅比较父级，随后加上 order 字段和有序子节点断言，重跑完整五轮；只以下列最终数据形成结论。

## 性能结果

原始十进程输出：[benchmark-raw.jsonl.gz](evidence/2026-11-03-loro-sdg-adapter/benchmark-raw.jsonl.gz)；
[中位数](evidence/2026-11-03-loro-sdg-adapter/benchmark-summary.json)。

| 指标（中位数） | Loro 1.16.0 | Yrs 0.27.4 |
|---|---:|---:|
| 十万动作 ms | 254.30 | 191.86 |
| 动作/s | 393242 | 521219 |
| 两向引擎导入 ms | 4350.97 | 106.13 |
| 两向本地缓存重建 ms | 33.23 | 10.18 |
| 两向完整合并 ms | 4381.97 | 116.35 |
| 两端全量 update bytes | 1894067 | 2881754 |
| 两端 update 导出 ms | 33.06 | 10.67 |
| 合并后 snapshot bytes | 1952007 | 2866614 |
| snapshot 导出 / 导入 ms | 78.75 / 35.54 | 10.54 / 106.69 |
| 编辑后两副本 RSS KiB | 108840 | 91676 |
| 全流程峰值 RSS KiB | 259576 | 265568 |

合并差异主要发生在引擎内，不是 cache 重建。Loro 约慢 41 倍；其 update/snapshot 体积、快照导入和
全流程峰值 RSS 较好，负面结论不能抹去这些优势。本数据只裁决该前置门禁，不证明 Yrs 可直接替换生产树。

update 是空 state vector 起的全量历史、bootstrap 计两次，不是逐 Action 编码体积。
Loro 保留操作历史，Yrs 默认 GC；两者不是相同的永久 checkpoint/undo 保留策略。
RSS 包含 trace、两副本、编码与第三个恢复副本，不等于引擎独占堆内存。
并发 actor 触及不同身份；同节点竞争、删除后子插入、冲突修复及 actor undo 不属于本基准通过声明。

## 验证

| 门禁 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --locked --workspace` | PASS，602 通过、11 项既有忽略 |
| `cargo test --locked -p scholium-sdg --example engine_bench` | PASS，2 通过 |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | PASS |
| `python3 scripts/check-rust-size.py` | PASS |
| `python3 scripts/check-doc-links.py` | PASS |
| `python3 scripts/check-ui-strings.py` | PASS，既有 47 项白名单 |
| `python3 scripts/check-layers.py` | PASS |
| `cargo deny --locked check licenses bans sources` | PASS，仅既有 unused allowance 提示 |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | PASS |
| `cargo build --locked -p scholium-sdg --target wasm32-unknown-unknown` | PASS |
| `python3 scripts/window-checks.py --locale zh-Hans en --out target/t41-window-all` | 首轮 45/46；math-input 双语复验 2/2，全部组合有通过记录 |

首轮门禁为历史验证，详细原始记录可在提交 `9c8b3cf` 查阅；当前保留续作后的最终门禁与证据。
当时 stock Typst 系统/bundled-only 各 31/31 像素一致；当前目录只保留后文链接的最终两份比较。
WASM target 镜像下载 404 后以单命令官方服务器下载成功，未修改 rustup/git 配置。
首轮仅验证引擎编译，不含生产 adapter；完整 adapter 的 WASM 验收见续作验证。
首轮窗口 45/46，中文 math-input 启动前 XOpenDisplayFailed；同一 runner 双语复验 2/2。
历史结果不作为最新代码门禁，较早窗口日志不再重复保留。
两项基准契约测试通过 example 的 `test = true` 纳入默认 workspace test。

## 待办与会话接口点

1. 协调者决定是否先诊断 Loro 的批量合并、授权比较 1.17.1，或授权 Yrs 移动语义与编码迁移研究。
   本轮没有自行升级/切换，也没有宣布阶段 1 接手门禁 2 关闭。
2. 复审后再建立 NodeId↔tree 身份映射、文本容器、确定性非法结构修复、版本化 Action 身份/actor/origin/update envelope，
   以 ADR 0053 固定；补四类结构竞争、actor-only undo/redo、乱序/重复/随机交错和快照+Actions 重放验收。
3. 后续会话在 `EditorCore::run` 的原子命令边界内、成功的 `apply_joined` 结构规划之后降为 adapter 事务；
   不让 LocalSession 与 CRDT 成为两个可写权威，不沿用快照栈冒充协作 undo。失败恢复须包含 adapter 状态。
4. adapter 产生带领域版本的完整 Action 后，交给 `ProjectStore::append_action(&ActionRecord)`；
   storage 的外层 version/hash 不替代 Action payload 版本。只有 durable sequence 返回后发布已保存状态。
   在已提交序号边界保存 CRDT 快照，恢复先导入快照再重放后续 Actions；不得把完整 structured JSON 冒充 CRDT update。
5. 正式存储与会话接入、网络、协作 UI、历史 DAG 仍不在本批范围；未修改 wt1/wt3 所有权文件。

## 交付

按性能停止条件只提交可审查的测量、边界与复审结论，签署提交并推送指定分支；不创建 PR。
协调者可直接使用本文的“问题与结论 / 改动 / 验证 / 待办”组织 PR 正文。

## 协调者续作：问题 / 改动 / 验证 / 待办

### 问题

首轮全历史两向合并不能代表在线增量协作，也没有比较当前稳定引擎。协调者要求四版本、
batch 1/10/100、离线 1k/10k/50k 每端、操作分类，并预先给定继续 Loro 的两个阈值。
按这两个阈值裁决，不将 Yrs 的速度优势自动解释为可替代 movable tree/undo 语义。

### 改动

- 生产精确依赖 Loro 1.17.1；独立 legacy/current manifests 与 lockfiles 同源编译四版本比较，
  保留早期历史原始数据。新增 delta、逐批分布、串行完整状态 oracle、各操作分类及自动门槛 verdict。
- 实现层 2 `Replica`，只依赖 model/document，无 UI/storage/session/build/latex 依赖。
  `StructuralEdit` 在临时 LocalSession 规划后降为节点属性、字符和 movable tree 操作，
  不用全 JSON LWW 替代 CRDT。效果入口带预期 revision、Action 身份和 origin。
- 稳定 NodeId↔TreeID，真实备用身份、首次使用后补充备用、确定性投影修复与显式报告；
  所有当前 block/inline/math 形状和 RawMath 保留，跨父移动不复制节点身份。
- `SCSDGA01` Action 保存完整 identity/actor/origin/Loro update；`SCSDGS01` snapshot 保存
  引擎历史、base revision、receipt SHA-256 和待前驱 Action；恢复+后续重放可继续编辑合并。
- actor-only UndoManager 排除 system/remote；身份出生不撤销，用户变化单命令 checkpoint。
  undo/redo 都追加新 Action，保留远端文字。设备栈不跨重启持久化。
- 新增 ADR 0053，修订 ADR 0009；更新 collab/history/document 相关规范与索引。
  model 最小改动为 NodeId byte 往返和 WASM 目标限定 uuid/js 熵源，并同步 model 规范。
  没有新增 C/C++ 依赖或 JavaScript 业务逻辑，没有修改另外执行者所有的源文件。

测试先行曾发现并修复：结构 redo 重生 metadata 导致身份重复、MathText 清空与并发字符的冲突、
Loro Snapshot 丢失 pending 输入、复用备用节点跨父移动造成重复身份，以及同一旧 inline 并发改 kind
时确定性备用名碰撞。身份出生排除 undo、保留 pending envelope、补充带 Action 身份的备用分别覆盖这些问题。
回归用例及修复保留在实现提交中，最终测试记录见后文；没有通过删除回归用例解决失败。

### 性能证据与判定

[混合/增量 raw](evidence/2026-11-03-loro-sdg-adapter/collaboration-raw.jsonl.gz)、
[summary](evidence/2026-11-03-loro-sdg-adapter/collaboration-summary.json) 为 72 个进程；
[操作分类 raw](evidence/2026-11-03-loro-sdg-adapter/operation-breakdown-raw.jsonl.gz)、
[summary](evidence/2026-11-03-loro-sdg-adapter/operation-breakdown-summary.json) 为 252 个进程；
[正式阈值 raw](evidence/2026-11-03-loro-sdg-adapter/criteria-raw.jsonl.gz)、
[verdict](evidence/2026-11-03-loro-sdg-adapter/criteria-verdict.json) 为 6 个进程。
另保留 [6 个先验 threshold pilot](evidence/2026-11-03-loro-sdg-adapter/criteria-pilot.json)，以及
[12 个四版本吞吐原始进程](evidence/2026-11-03-loro-sdg-adapter/throughput-four-raw.jsonl.gz)与
[吞吐汇总](evidence/2026-11-03-loro-sdg-adapter/throughput-four-summary.json)。
全部有效样本检查 monotonic/boottime，未检测到挂起。
证据清理后只保留本文及 ADR 引用的文件；原始 `.jsonl` 与超过 200 KB 的失败日志使用 gzip 无损压缩，
可用 `gzip -dc <文件.gz>` 查看。stock 只保留最终系统/bundled 比较，窗口只保留最终轮和故障复验。

### 增量交换（三轮 pooled nearest-rank p50 / p99，ms/接收批次）

| 引擎 | 动作/端 | batch 1 | batch 10 | batch 100 |
|---|---:|---:|---:|---:|
| loro-1.16.0 | 1k / 10k / 10k | 3.604 / 11.304 | 16.344 / 45.627 | 18.349 / 41.902 |
| loro-1.17.1 | 1k / 10k / 10k | 0.148 / 1.369 | 0.504 / 1.980 | 1.102 / 2.337 |
| yrs-0.27.4 | 1k / 10k / 10k | 0.174 / 0.311 | 1.125 / 3.398 | 1.427 / 4.017 |
| yrs-0.28.0 | 1k / 10k / 10k | 0.160 / 0.310 | 0.761 / 4.528 | 1.369 / 3.562 |

batch 1 使用 1k/端（每引擎 6000 次接收），batch 10/100 使用 10k/端（6000 / 600 次接收）。
1.16.0 的 batch 1 在 10k/端的初次运行超过十分钟而尚未完成；该进程按 PID 结束，不作为有效样本。
为覆盖长会话，另对 1.17.1 实测三轮 50k/端、batch 100（3000 次接收），每轮 p99 为
6.980 / 9.743 / 9.573 ms；包含引擎导入和身份索引维护，最坏一轮仍小于 16 ms。

### 一次离线合并（三轮中位数，ms，两向合计）

| 引擎 | 1k/端 | 10k/端 | 50k/端 |
|---|---:|---:|---:|
| loro-1.16.0 | 9.374 | 171.220 | 4123.647 |
| loro-1.17.1 | 7.718 | 73.491 | 427.583 |
| yrs-0.27.4 | 2.259 | 19.471 | 118.480 |
| yrs-0.28.0 | 2.337 | 19.689 | 122.553 |

1.17.1 的 10k/端判定专用三轮两向合计为 63.902 / 94.532 / 101.653 ms；
使用比单向合并更保守的两向合计，最坏轮小于 500 ms。导出、用户编辑及完整投影不计入接收时间。

### 操作分类（三轮中位数，引擎导入 / 索引维护 ms，两向合计）

| 操作 | 动作/端 | Loro 1.16.0 | Loro 1.17.1 | Yrs 0.27.4 | Yrs 0.28.0 |
|---|---:|---:|---:|---:|---:|
| text_insert | 1000 | 3.318 / 0.011 | 2.263 / 0.010 | 0.247 / 0.110 | 0.242 / 0.093 |
| text_insert | 10000 | 36.894 / 0.016 | 25.394 / 0.015 | 3.431 / 0.126 | 3.448 / 0.137 |
| text_insert | 50000 | 173.578 / 0.016 | 123.217 / 0.014 | 16.213 / 0.127 | 17.066 / 0.128 |
| text_delete | 1000 | 3.932 / 0.010 | 2.456 / 0.011 | 0.106 / 0.108 | 0.113 / 0.127 |
| text_delete | 10000 | 47.087 / 0.014 | 29.990 / 0.014 | 1.132 / 0.161 | 1.154 / 0.127 |
| text_delete | 50000 | 315.064 / 0.014 | 263.245 / 0.017 | 7.185 / 0.151 | 7.814 / 0.149 |
| node_create | 1000 | 48.792 / 1.011 | 28.727 / 1.006 | 4.698 / 1.363 | 4.579 / 1.398 |
| node_create | 10000 | 3407.193 / 18.518 | 356.337 / 21.350 | 56.340 / 19.554 | 56.891 / 19.390 |
| node_create | 50000 | 169327.227 / 83.015 | 1953.937 / 108.389 | 294.713 / 90.492 | 290.022 / 83.278 |
| node_delete | 1000 | 13.642 / 0.087 | 14.077 / 0.091 | 1.740 / 1.314 | 1.723 / 1.355 |
| node_delete | 10000 | 183.882 / 1.394 | 186.802 / 1.302 | 26.079 / 16.758 | 26.105 / 18.109 |
| node_delete | 50000 | 1002.526 / 7.831 | 1013.585 / 7.272 | 128.491 / 82.058 | 126.208 / 83.689 |
| wrap | 1000 | 36.494 / 1.649 | 36.164 / 1.650 | 7.858 / 3.264 | 7.589 / 2.985 |
| wrap | 10000 | 456.042 / 19.151 | 477.732 / 19.782 | 97.874 / 37.939 | 93.494 / 39.153 |
| wrap | 50000 | 2597.293 / 95.457 | 2719.492 / 108.926 | 497.902 / 211.672 | 487.858 / 194.285 |
| unwrap | 1000 | 32.064 / 0.184 | 32.603 / 0.198 | 7.814 / 3.482 | 7.148 / 3.314 |
| unwrap | 10000 | 376.607 / 3.478 | 493.245 / 3.830 | 90.911 / 41.781 | 89.320 / 41.661 |
| unwrap | 50000 | 2412.073 / 17.605 | 2347.125 / 14.966 | 447.457 / 206.928 | 437.463 / 198.782 |
| cross_parent_move | 1000 | 3.188 / 0.010 | 2.980 / 0.010 | 0.853 / 0.102 | 0.909 / 0.111 |
| cross_parent_move | 10000 | 42.242 / 0.020 | 35.802 / 0.015 | 13.456 / 0.143 | 12.353 / 0.132 |
| cross_parent_move | 50000 | 145.705 / 0.015 | 143.512 / 0.015 | 44.032 / 0.115 | 46.207 / 0.111 |

分类实验不把 bootstrap、删除目标/包裹内容的准备算进动作；wrap 一次动作创建 wrapper 并移动内容，
unwrap 一次动作移出内容并删除 wrapper。每次动作提交一次。文字删除预先填入对应数量的 `é`；
节点删除预先创建目标，wrap/unwrap 每次作用于不同内容节点，cross-parent move 在两个段落间往返。
50k/端纯节点类压力用例可超过产品 100000 个可见节点的上限，只用于定位引擎成本。
与混合负载不同，这些分类用例不是对真实编辑比例的估计，不能把分类均值相加来预测产品延迟。

纯 node create 的旧版超线性成本是最明显慢点，10k/端旧版约 3.4 s、新版约 0.36 s；
50k/端旧版约 169 s、新版约 1.95 s（两向 engine import，不含 index）。
分类已足以支持本轮裁决，不凭空归因到未经 profiler 验证的具体上游函数，没有修改第三方引擎。
最新 Loro 在 bulk wrap/unwrap 仍比 Yrs 昂贵；50k/端离线是明确记录的已知边界，
不能把混合测量外推为所有结构分布都满足 500 ms，也不把引擎测量冒充 adapter 全文预演成本。

### 续作验证

保留实现提交 `ef166b6`；随后按共享规则签署合并 main `42fa7ce` 与 `0648a0e`。
第二次合并只解决 ADR 索引的并行追加，保留 0051/0052/0053；Cargo.lock 由 Git 合并。
相对最新 main 的功能差异不涉及 app/session/storage/build/latex，相关文件仅来自 main 合并。

最新 main 上的[门禁命令与退出码](evidence/2026-11-03-loro-sdg-adapter/final-gates.json)：

| 门禁 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --locked --workspace` | PASS，645 通过、0 失败、13 项既有忽略 |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | PASS |
| `python3 scripts/check-rust-size.py` | PASS |
| `python3 scripts/check-doc-links.py` | PASS |
| `python3 scripts/check-ui-strings.py` | PASS，41 项既有白名单 |
| `python3 scripts/check-layers.py` | PASS |
| `cargo deny --locked check licenses bans sources` | PASS，仅 unused allowance 提示 |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | PASS |
| `cargo build --locked -p scholium-sdg --target wasm32-unknown-unknown` | PASS，完整生产 adapter |

[workspace 原始测试输出](evidence/2026-11-03-loro-sdg-adapter/final-tests.log) 包含生产 adapter 17 项、
engine example 2 项、collaboration example 3 项。两个独立 benchmark manifests 的 fmt/clippy/test/doc/deny
也[全部通过](evidence/2026-11-03-loro-sdg-adapter/standalone-gates-continuation.log)，不拿根 workspace 门禁代替独立依赖图。

按 `.github/workflows/typst-fork.yml` 的两组 cargo run + compare 命令复跑，
[系统字体](evidence/2026-11-03-loro-sdg-adapter/stock-system-final.json) 与
[bundled-only](evidence/2026-11-03-loro-sdg-adapter/stock-bundled-final.json) 均 31/31 像素一致，
[命令输出](evidence/2026-11-03-loro-sdg-adapter/stock-commands-final.log)保留。

最新 main 上的[完整双语窗口轮](evidence/2026-11-03-loro-sdg-adapter/window-final-all.log)为 **53/54**；
唯一英文 sharp-raster 在滚动时私有 Xvfb 被 SIGKILL，见[失败记录](evidence/2026-11-03-loro-sdg-adapter/window-final-sharp-xvfb-failure.log)。
通过同一 runner、原脚本和全新目录只复验 `--only sharp-raster --locale en`：
[第一次 0/1](evidence/2026-11-03-loro-sdg-adapter/window-sharp-rerun1.log.gz)、
[第二次 0/1](evidence/2026-11-03-loro-sdg-adapter/window-sharp-rerun2.log.gz) 均有 Xvfb SIGKILL/连接中断，
[第三次 1/1](evidence/2026-11-03-loro-sdg-adapter/window-sharp-rerun3-pass.log) 通过，耗时 87.7 秒。
最后一轮未改变输入速度、超时、像素参考或门槛。
**27 项 × 两语言的 54 个组合均有通过记录**，不把完整轮或前两次复验写为全通过。
截图、完整 AUDIT 和实际项目保留在槽内 `target/t41-window-0648a0e-*`、`target/t41-sharp-en-*`。

此前 main `42fa7ce` 的窗口轮为 38/52，有 Xvfb SIGKILL、连接失败、输入/截图中断和排版超时。
只读进程检查曾发现无关任务向全机同名 Xvfb 发送 SIGKILL；未干预该任务。
另一次复验因误用旧输出目录而中止，随后以全新目录运行；两次中间记录均不作为有效门禁。
当前目录只保留上面的最终全量轮与故障/复验日志，不重复保留旧轮或误用目录的中止输出。
没有修改窗口脚本、应用代码、参考或容差。

adapter 定向测试覆盖：
同叶并发文本、wrap/delete、同节点不同 parent 移动、delete/remote child insert、并发 wrap，
empty math/concurrent text、备用补充后移动、并发 kind、actor undo/redo、所有节点形状、
快照+pending/乱序/重复 replay 与固定种子 `0x5eed0041..0x5eed004a` 三 actor 各轮 120 步。
随机对已隐藏目标的 WrongTarget 同样验证拒绝原子性。

### 待办与限制

1. 下一批在 `EditorCore::run` / 成功 `apply_joined` 命令边界用 Replica 替换本地权威；
   不保持 LocalSession 与 Replica 双向可写。临时 JSON session Action 编码由正式 Action::encode bytes 替换。
2. 完整 Action bytes 交给 `ProjectStore::append_action`，commit 返回 durable sequence 后才发布成功；
   已提交边界调用 `save_snapshot(sequence, bytes)`，恢复后逐 Action receive。失败回滚需覆盖 actor/undo/receipts，
   不能以旧快照回退冒充共享撤销。本批未修改 session/app/storage。
3. 未接入网络、权限、冲突 UI 与历史 DAG；projection repair 必须显示诊断，不能静默用于发布。
   多 root 选择首项、非法层级抑制和容量前缀投影都保留引擎原值，仍可能需要用户解决结构意图。
4. adapter 使用全文规划/索引/投影与 fork 预演，当前不承诺逐键成本独立于文档大小；
   存储/网络/渲染延迟均不在引擎基准内。长离线合并应放工作线程并分批同步。
5. receipt 100k、pending 1 MiB、binary payload 64 MiB 等边界会明确拒绝，当前无 GC 或迁移器；
   undo 栈是设备会话状态、恢复后为空，不声称跨重启产品历史已实现。

交付保持同一分支、签署提交并推送，不创建 PR，也不轮询 CI。
