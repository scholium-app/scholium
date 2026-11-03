# T41：Loro SDG adapter 的性能前置门禁

任务基线 `4bb9808ecf79aca8e17d7ba5247f4603688e8b8e`，工作槽 `scholium-wt2`，
分支 `ation_ciger/feat/loro-sdg-adapter`。依据 ADR 0009、0041、0042、0049 与任务停止规则执行。

## 问题与结论

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

原始十进程输出：[benchmark-raw.jsonl](evidence/2026-11-03-loro-sdg-adapter/benchmark-raw.jsonl)；
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

主要命令输出保留在 [evidence](evidence/2026-11-03-loro-sdg-adapter/tests.log)。
stock Typst 按 workflow 跑独立 fork/reference 与 compare.py，
[系统字体](evidence/2026-11-03-loro-sdg-adapter/stock-system.json) 与
[bundled-only](evidence/2026-11-03-loro-sdg-adapter/stock-bundled.json) 各 31/31 像素相等，未修改 reference 或阈值。
WASM target 镜像下载 404 后以单命令官方服务器下载成功，未修改 rustup/git 配置。
实际 `cargo build --locked -p scholium-sdg --target wasm32-unknown-unknown` 编译了锁定 Loro 引擎并通过；
该探针没有 adapter，因此不能称为生产 adapter 的 WASM 验收。
完整双语窗口检查使用共享机器锁、私有 Xvfb，原样运行现有检查，不触及用户桌面。
[首轮 45/46](evidence/2026-11-03-loro-sdg-adapter/window-all.log)；中文 math-input 在执行断言前遇到
[XOpenDisplayFailed](evidence/2026-11-03-loro-sdg-adapter/xvfb-startup-failure.log)。
未修改 app 或脚本，通过同一共享 runner 在新目录中双语[复验 2/2](evidence/2026-11-03-loro-sdg-adapter/window-math-rerun.log)：
`python3 scripts/window-checks.py --only math-input --locale zh-Hans en --out target/t41-window-math-rerun`。
23 项 × 2 种语言的全部组合均有通过记录，不将首轮伪报为全通过。
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
