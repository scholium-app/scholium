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
