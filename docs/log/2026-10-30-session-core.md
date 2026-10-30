# 2026-10-30 — Session core phase 1

## 范围与基线

- 在指定 slot `scholium-wt1` 工作，初始 `git status --short` 为空。
- 按规格 fetch 后从 `origin/main` `19811228785048b080177f91f487df04bec5372f` 建立 `ation_ciger/refactor/session-core`。
- 只处理 T15；未修改 wt2 所属的 toolchain-sandbox、rtex-eval 或开发日志。
- ADR 0042 草稿逐字节复制到正式路径，没有事实修订。

## 实现

提取了不依赖 UI 的 `scholium-session::typst::EditorCore`：LocalSession/Arc 投影、store 与 saved 基线、
恢复/迁移/保存、历史与时钟、光标/anchor、样式、Attach 占位、数学回车、纯语义选区与诊断来源。
app 的 registry action 沿用同一 CommandId，文字、结构命令、删除/拆段和历史全部经 `run`；
app 没有保留第二份可写 LocalSession，也没有另一条 apply_structural 路径。
worker/Scene、栅格/绘制、placement/IME、指针/点击序列、垂直目标、follow、队列、access、源码与确认框留在 app。

命令执行前生成完整 checkpoint。多步操作（插入后应用 armed style、清理占位后数学拆段）包含于同一 unwind 边界；
Err/panic 恢复快照、请求日志、两端、undo/redo、run/clock、pending style、placeholder、保存基线与诊断来源。
回滚副本预先验证；实际 authority 已改变时按既有 restore 语义更换布局 epoch，app 重新请求 Scene，防止旧几何恢复命中。
仅参数/校验拒绝而 authority 未变时保留原 epoch。unwind 边界内没有 store 写入，显式 save 成功后才更新 saved 基线。
`AssertUnwindSafe` 的原因记录在 run 旁：捕获的可变状态可全部恢复，UI 与存储不进入该边界；注入时钟只读时间。

只读 authority wrapper 不提供 DerefMut 或公共构造器，既有测试断言可原样保留。
唯一修改的既有测试文件是 RawMath fixture 的恢复入口（`selection/tests.rs`），断言、预期和阈值均未改。
UI 错误原文仍留在原 app 文件，core 返回纯原因枚举；UI 字串 allowlist 无改动，仍为 69 项。

## Tests first 与失败注入

先写了 headless 验收测试，第一轮在尚无 typst 核心模块时编译失败（E0583），然后接入实现。
最终 6 个测试覆盖：正文输入与输入组 undo/redo、选区替换/两端恢复、公式进出与 fraction、sup/sub Hole、
两种占位离开后的完整快照与撤销深度、bold/emphasis、保存重开、参数/结构拒绝，以及 Err/panic 修改后回滚。

失败注入从已有保存 head 的编辑会话开始，同时保留 run、redo、pending style 和 placeholder。
测试命令先插入文字，再改变两端、清空历史/run/style/placeholder、替换 clock、改诊断和 saved 基线，随后 Err 或 panic。
两种路径均比较完整 checkpoint、clock Arc 身份、同一 store 的 saved head，并重新打开文件核对保存内容；均通过。

## Layer gate 与负对照

新增 `scripts/check-layers.py`，读取 `cargo metadata --format-version 1 --locked` 的完整 resolve 图，
登记 model 0 → document 1 → typst/storage 2 → session 3 → app 4，拒绝未登记 workspace crate、向上/同层依赖，
以及 app 以下直接或传递依赖 egui/eframe/winit/accesskit（包括 dev/build edges）。CI lint 增加一步。

真实负对照：临时在 session 的 dependencies 加 `egui = "=0.36.2"`，更新临时 lock 后运行 gate，退出码 1：

```text
UI below app: scholium-session -> egui
UI below app: scholium-session -> egui -> accesskit
```

finally 恢复原 manifest 与 lock 的字节，gate 再次通过。负对照没有进入提交。

## 验证

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --locked --workspace` | PASS，438 passed / 1 既有 ignored；app 302、session 6 |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | PASS |
| `python3 scripts/check-rust-size.py` | PASS |
| `python3 scripts/check-doc-links.py` | PASS |
| `python3 scripts/check-ui-strings.py` | PASS，69 项原 allowlist 不变 |
| `python3 scripts/check-layers.py` | PASS；真实 egui 负对照按预期 FAIL |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | PASS |
| `cargo deny --locked check licenses bans sources` | PASS（既有未使用许可 allowance 警告） |
| `python3 scripts/window-checks.py --locale zh-Hans en --out target/t15-window` | PASS，28/28，未重试 |
| `python3 scripts/verify-typst-fork.py target/t15-typst-origin` | PASS，上游对象为 workflow 固定的 9dfd3a08500b7896045f907433cf7b4b02434fad |
| `cargo run --locked -p scholium-typst --features editor --example typst-main-fork-check -- target/t15-typst-fork-pixels` | PASS |
| `cargo run --locked --manifest-path spikes/typst-main-editor/reference/Cargo.toml --target-dir target/stock-reference -- target/t15-typst-stock-pixels` | PASS，构建时仅用 2 jobs/nice 10 降低窗口检查竞争 |
| `python3 spikes/typst-main-editor/compare.py target/t15-typst-fork-pixels target/t15-typst-stock-pixels` | PASS，4/4 equal_pixels |

窗口检查使用脚本的机器锁与私有 Xvfb，未访问用户桌面。zh-Hans/en 各 14 项全部通过：a4-surface、caret-follow、chrome、default-launch、diagnostics、inline-style、interaction、math-attach、math-enter、native、sharp-raster、source-view、undo-group、a11y。
stock 的 full/scripts/styles/heading 均逐像素相等；独立 reference 与所有阈值不变。
本地证据与完整日志在 slot 的 `target/t15-*.log`、`target/t15-window/` 和 `target/t15-stock-comparison.json`，未提交构建/窗口产物。

## 行数

`typst_editor.rs`：543 → 425 行。新增 session 各模块如下（相对 `crates/scholium-session/src/`）：

| 模块 | 之前 | 之后 |
|---|---:|---:|
| `lib.rs` | 0 | 2 |
| `typst/attach.rs` | 0 | 135 |
| `typst/command/checkpoint.rs` | 0 | 115 |
| `typst/command.rs` | 0 | 247 |
| `typst/diagnose.rs` | 0 | 41 |
| `typst/edit.rs` | 0 | 60 |
| `typst/history.rs` | 0 | 155 |
| `typst/indicator.rs` | 0 | 137 |
| `typst/input.rs` | 0 | 274 |
| `typst/math_enter.rs` | 0 | 65 |
| `typst/persistence.rs` | 0 | 158 |
| `typst/selection.rs` | 0 | 139 |
| `typst/style.rs` | 0 | 166 |
| `typst/tests.rs` | 0 | 289 |
| `typst.rs` | 0 | 114 |

session 总计 0 → 2097 行（含无界面测试）；所有文件均低于 600 行。

## 必要范围补充与后续

- 除规格列出的文件外，修改 `crates/scholium-app/Cargo.toml` 一行以接入 session；这是 app 依赖新 crate 的必要 wiring。
- 新 crate 保留 `unicode-segmentation = "=1.13.3"`：它来自原 app 的同版本既有逻辑，保证插入后字素重锚和样式范围不变；
  未新增第三方包或版本，未为凑依赖表自行实现另一套 Unicode 分段。
- ADR 草稿没有修订；没有行为/断言/窗口或像素阈值变更。
- phase 2 另行处理指针/点击、垂直导航/goal、follow 与抽象事件翻译边界；当前未实施这些移动。
- 推送前重新 fetch 确认 origin/main 仍为上述基线，无需 merge。只开 PR 到 main，不合并、不轮询 CI。

## PR #63 性能评审修复

评审指出 checkpoint 对每次 Text 克隆 undo/redo 时，会递归复制 512 份完整文档和请求日志。
本轮在同一分支 `a02a104f34adcc9c6936d484f5f674c6964d342e` 上继续修复，开始时工作区干净；
没有重新从 main 建分支，没有修改既有断言、窗口检查、reference 或任何阈值。

### 共享与回滚

历史条目的 `HistorySnapshot` 内持有 `Arc<StructuredDocument>`，requests 为 `Arc<[RequestId]>`；
checkpoint 克隆 undo/redo 和 placeholder 只复制指针。包装类型保留与 owned 文档的值比较，
原有 `record().snapshot` 比较断言原样编译；restore_document 同时接受 Vec 和 Arc 请求切片。

为避免 checkpoint 再复制一份权威文档，LocalSession 的 accepted snapshot 和 action journal
改为内部 Arc。`shared_snapshot` 让权威、投影、历史与 saved 基线复用同一份已接受文档；
`rollback_checkpoint` 直接保留已验证的快照和日志，并提前分配恢复用的新 epoch，构造为 O(1)。
命令执行前不再 restore/validate 全文或重建请求日志。结构编辑仍只在独立规划副本上验证后接受，
action journal 提交时写时复制；checkpoint 不因此变为另一份持续可写正文。
回滚直接替换已保留权威，无再次验证、无可失败恢复；未修改权威的拒绝保留原 epoch。

新增指针共享回归测试，填满历史后保留 undo + redo，检查所有条目的文档和请求 Arc、
checkpoint 权威/投影/saved 的 Arc 身份及 journal 切片地址。修改后 Err/panic 均恢复相同指针与请求、
更新 epoch，原 retained 文档 revision 不变，redo 仍可工作。既有完整状态/磁盘 saved-head 回滚测试继续通过。
本轮仅在既有 tests.rs 尾部登记计时子模块，未改原测试主体或断言。

### 修复前后计时

新增显式 ignored 计时测试及 `scripts/benchmark-session-checkpoint.py`，均随本修复提交。
先只加入计时钩子，在未修复实现上测 before，然后修改共享实现、用同一夹具和步骤测 after。
钩子仅在 cfg(test) 下，包围真实 `run(Text, ...)` 内的 checkpoint 构造，计时结束后再记录样本；
不包含命令执行、checkpoint 析构、夹具构建、历史填充或预览编译，也不修改生产路径。

- 合成夹具：286 个 paragraph，每段重复两次固定英文 literal，初始正文 60,060 UTF-8 bytes；
  用正式 PreviewCompiler / SourceMode::Preview 在计时外实际编译，断言初始 A4 预览为 **26 页**。
- 用真实 Text 命令形成 512 个独立撤销组，再预热 8 次；采样前逐次断言 undo 深度仍为 512。
  采样 100 次，nearest-rank p50/p95（排序后第 50/95 个），每次输入一个 x，强制新组以保持满栈。
- 同机 Intel Core i7-13650HX，Linux x86_64，rustc 1.99.0；两次均为 test profile：
  workspace opt-level 0，依赖 opt-level 2。不是 release 或跨机器延迟承诺。
- 包装脚本比较 CLOCK_MONOTONIC 与 CLOCK_BOOTTIME，差值 >1 秒则废弃整轮；
  两轮差值均远小于 1 秒，无休眠跨越。

| checkpoint（100 次，512 步历史） | 修复前 | 修复后 | 前/后 |
|---|---:|---:|---:|
| p50 | 16.071072 ms | 0.020276 ms | 792.6 |
| p95 | 16.564576 ms | 0.022358 ms | 740.9 |

复现命令（before 对应仅加入相同计时钩子、尚未结构共享的 a02a104 工作树）：

```sh
python3 scripts/benchmark-session-checkpoint.py --label before --out docs/log/evidence/2026-10-30-session-core/checkpoint-before.json
python3 scripts/benchmark-session-checkpoint.py --label after --out docs/log/evidence/2026-10-30-session-core/checkpoint-after.json
```

原始样本与环境/时钟记录：[before](evidence/2026-10-30-session-core/checkpoint-before.json)、
[after](evidence/2026-10-30-session-core/checkpoint-after.json)。JSON 的 revision 是测量时的 HEAD，
after 在该 HEAD 的未提交修复工作树上测量；label 区分实现，未将 HEAD 冒充无修改源码。
这里只证明 checkpoint 的深拷贝回退已消除；结构规划及首次 journal 写时复制仍随当前文档/日志大小增长。

### 本轮范围补充

为让 checkpoint 的权威也共享，必要修改原 T15 ownership 外的 document/src/lib.rs、
structured_session.rs 及 document 模块规范，保留既有 owned snapshot/restore 调用兼容。
新增 benchmark 脚本及 JSON 证据；无新依赖、无格式迁移，wt2 文件和共享日志索引未动。

### 本轮门禁

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --locked --workspace` | PASS，439 passed / 2 ignored；app 302、session 7，新增手动计时另行显式运行 |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | PASS |
| `python3 scripts/check-rust-size.py` | PASS |
| `python3 scripts/check-doc-links.py` | PASS |
| `python3 scripts/check-ui-strings.py` | PASS，69 项原 allowlist |
| `python3 scripts/check-layers.py` | PASS |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | PASS |
| `cargo deny --locked check licenses bans sources` | PASS，既有 unused allowance 警告 |
| `python3 scripts/window-checks.py --locale zh-Hans en --out target/t15-review-window` | PASS, 28/28（zh-Hans 14/14、en 14/14） |
| `python3 scripts/verify-typst-fork.py target/t15-typst-origin` | PASS，固定上游 9dfd3a08500b7896045f907433cf7b4b02434fad |
| `cargo run --locked -p scholium-typst --features editor --example typst-main-fork-check -- target/t15-review-fork-pixels` | PASS |
| `cargo run --locked --manifest-path spikes/typst-main-editor/reference/Cargo.toml --target-dir target/stock-reference -- target/t15-review-stock-pixels` | PASS |
| `python3 spikes/typst-main-editor/compare.py target/t15-review-fork-pixels target/t15-review-stock-pixels` | PASS，full/scripts/styles/heading 4/4 equal_pixels |

完整本地门禁日志在 `target/t15-review-*.log`，窗口证据在 `target/t15-review-window/`。
未改任何既有断言、窗口检查或 stock 阈值；测试仅新增共享回归和显式计时。

本轮相对 a02a104 的 session 行数变化：

| 模块（src/typst/） | 修复前 | 修复后 |
|---|---:|---:|
| `attach.rs` | 135 | 135 |
| `command.rs` | 247 | 251 |
| `command/checkpoint.rs` | 115 | 116 |
| `edit.rs` | 60 | 60 |
| `history.rs` | 155 | 158 |
| `persistence.rs` | 158 | 162 |
| `tests.rs` | 289 | 292 |
| `checkpoint_bench.rs` | 0 | 90 |
| `command/checkpoint/tests.rs` | 0 | 68 |
| `history/snapshot.rs` | 0 | 29 |

`typst.rs` 仍为 114 行；app 的 `typst_editor.rs` 仍为 425 行。所有文件低于 600 行。
