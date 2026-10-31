# 2026-10-31 — Session core phase 2b

## 范围与基线

在指定 slot `scholium-wt1` 开始时工作树干净，fetch 后从 `origin/main`
`6fa1469` 建立 `ation_ciger/refactor/session-core-scene`。本轮完成 T20 留给 phase 2b 的
主编辑面 worker/Scene ownership、栅格请求调度和 deferred-input 语义迁移。
未修改 LaTeX 工作区规划、ADR 0043、workflow、窗口检查、场景文件、独立 stock reference 或任何阈值。

## 唯一路径

`EditorCore` 持有真实 `EditorWorker`、wanted 完整 SceneStamp、已采纳 EditorScene、可见 ViewRange 和 RasterSpec。
app 将视口转换成 `Input::Viewport`；40 ms 滚动节流与 120 ms 栅格稳定策略位于 core，
app 只反馈已显示纹理的 sharp scale，并按 core 返回的时间请求 repaint。原 app 的 worker 访问、
Scene 采纳校验、节流和去抖实现均删除。A4 源码预览是只读展示适配，其稳定策略复用同一 session 实现。

成功编辑在 `EditorCore::input` 的事务边界外提交已接受快照，失败不提交中间状态；回滚更换 epoch 后重新请求 Scene。
Scene 采纳逐字段匹配完整 stamp，旧 Scene 在新结果到达前继续显示，但不允许指针/视觉行导航读取旧几何。
`take_scene_rasters` 移动 base/sharp 位图条带，保留 core 的 Scene 身份、几何和统计；后续条带由 core 过滤身份后交 app 上传。
AUDIT 字段名称、含义和坐标系保持不变。

core 的抽象 admission 队列决定等待、256 条上限、被拒文字保留、指针拒绝、IME/Escape 取消例外及布局失败时只丢弃几何键。
app 的 egui payload 队列只与 core 的抽象记录一一匹配存取，不保留另一套规则。确认丢弃后两者同时清空。
现有测试通过 EditorSession 的既有 Deref 读取迁入 core 的 Scene/view/wanted，断言无需修改；
测试注入过期结果的 app helper 仅转调真实 worker 同样使用的 core 采纳函数。

## 验证记录

既有 app 编辑器 205 项单测在迁移 worker、迁移栅格策略、自动提交后均通过，断言原样保留。
一次运行的 SQLite fixture 创建报 `disk I/O error`，没有进入行为断言；改用本 slot 下 `target/t20-tmp`
作为 TMPDIR 后正常通过。未删除其他任务或旧验证产物。

共享场景重放改用真实 worker，逐输入等待当前 Scene 采纳，并检查 revision、caret 几何和零源码读取；
按场景的 modifiers、command 和语义 target 解析抽象输入。新增 worker/admission 集成测试覆盖延迟顺序、
pending 指针拒绝、旧 Scene 拒绝以及 raster hand-off 后 Scene 几何仍留在 core。


## app typst_editor* 行数

迁移前为 `6fa1469`；迁移后包含全部 Rust 文件及既有测试，0 表示新文件。

| 文件 | 前 | 后 | 变化 |
| --- | ---: | ---: | ---: |
| `crates/scholium-app/src/typst_editor.rs` | 417 | 407 | -10 |
| `crates/scholium-app/src/typst_editor/access.rs` | 267 | 267 | +0 |
| `crates/scholium-app/src/typst_editor/access/runs.rs` | 255 | 255 | +0 |
| `crates/scholium-app/src/typst_editor/access/speech.rs` | 106 | 106 | +0 |
| `crates/scholium-app/src/typst_editor/access/tests.rs` | 395 | 395 | +0 |
| `crates/scholium-app/src/typst_editor/attach.rs` | 7 | 7 | +0 |
| `crates/scholium-app/src/typst_editor/attach/tests.rs` | 444 | 444 | +0 |
| `crates/scholium-app/src/typst_editor/audit.rs` | 94 | 95 | +1 |
| `crates/scholium-app/src/typst_editor/clipboard.rs` | 14 | 14 | +0 |
| `crates/scholium-app/src/typst_editor/command_keys_tests.rs` | 107 | 107 | +0 |
| `crates/scholium-app/src/typst_editor/confirm.rs` | 49 | 50 | +1 |
| `crates/scholium-app/src/typst_editor/core_adapter.rs` | 75 | 71 | -4 |
| `crates/scholium-app/src/typst_editor/deferred.rs` | 114 | 87 | -27 |
| `crates/scholium-app/src/typst_editor/diagnose.rs` | 116 | 113 | -3 |
| `crates/scholium-app/src/typst_editor/diagnose/tests.rs` | 112 | 112 | +0 |
| `crates/scholium-app/src/typst_editor/follow.rs` | 98 | 98 | +0 |
| `crates/scholium-app/src/typst_editor/follow_tests.rs` | 228 | 228 | +0 |
| `crates/scholium-app/src/typst_editor/history.rs` | 39 | 39 | +0 |
| `crates/scholium-app/src/typst_editor/history/tests.rs` | 209 | 209 | +0 |
| `crates/scholium-app/src/typst_editor/indicator.rs` | 21 | 21 | +0 |
| `crates/scholium-app/src/typst_editor/indicator/tests.rs` | 167 | 167 | +0 |
| `crates/scholium-app/src/typst_editor/input.rs` | 227 | 227 | +0 |
| `crates/scholium-app/src/typst_editor/math_enter.rs` | 9 | 9 | +0 |
| `crates/scholium-app/src/typst_editor/math_enter/tests.rs` | 172 | 172 | +0 |
| `crates/scholium-app/src/typst_editor/paint.rs` | 283 | 282 | -1 |
| `crates/scholium-app/src/typst_editor/raster.rs` | 152 | 103 | -49 |
| `crates/scholium-app/src/typst_editor/raster/layers.rs` | 257 | 213 | -44 |
| `crates/scholium-app/src/typst_editor/raster/tests.rs` | 165 | 165 | +0 |
| `crates/scholium-app/src/typst_editor/scene.rs` | 44 | 38 | -6 |
| `crates/scholium-app/src/typst_editor/selection.rs` | 308 | 296 | -12 |
| `crates/scholium-app/src/typst_editor/selection/pointer.rs` | 39 | 39 | +0 |
| `crates/scholium-app/src/typst_editor/selection/pointer_tests.rs` | 364 | 364 | +0 |
| `crates/scholium-app/src/typst_editor/selection/tests.rs` | 467 | 467 | +0 |
| `crates/scholium-app/src/typst_editor/source_view.rs` | 464 | 464 | +0 |
| `crates/scholium-app/src/typst_editor/source_view/tests.rs` | 121 | 121 | +0 |
| `crates/scholium-app/src/typst_editor/startup.rs` | 54 | 54 | +0 |
| `crates/scholium-app/src/typst_editor/startup/tests.rs` | 113 | 113 | +0 |
| `crates/scholium-app/src/typst_editor/style.rs` | 13 | 13 | +0 |
| `crates/scholium-app/src/typst_editor/style/tests.rs` | 258 | 258 | +0 |
| `crates/scholium-app/src/typst_editor/tests.rs` | 502 | 502 | +0 |
| `crates/scholium-app/src/typst_editor/vertical.rs` | 16 | 16 | +0 |
| `crates/scholium-app/src/typst_editor/vertical/tests.rs` | 503 | 503 | +0 |
| `crates/scholium-app/src/typst_editor/window_tests.rs` | 490 | 490 | +0 |
| **合计** | **8355** | **8201** | **-154** |

## scholium-session 行数

迁移前为 `6fa1469`；迁移后包含全部 Rust 文件及既有测试，0 表示新文件。

| 文件 | 前 | 后 | 变化 |
| --- | ---: | ---: | ---: |
| `crates/scholium-session/src/lib.rs` | 2 | 2 | +0 |
| `crates/scholium-session/src/typst.rs` | 193 | 203 | +10 |
| `crates/scholium-session/src/typst/attach.rs` | 135 | 135 | +0 |
| `crates/scholium-session/src/typst/checkpoint_bench.rs` | 90 | 90 | +0 |
| `crates/scholium-session/src/typst/command.rs` | 291 | 293 | +2 |
| `crates/scholium-session/src/typst/command/checkpoint.rs` | 145 | 145 | +0 |
| `crates/scholium-session/src/typst/command/checkpoint/tests.rs` | 68 | 68 | +0 |
| `crates/scholium-session/src/typst/deferred.rs` | 0 | 110 | +110 |
| `crates/scholium-session/src/typst/diagnose.rs` | 41 | 41 | +0 |
| `crates/scholium-session/src/typst/edit.rs` | 60 | 60 | +0 |
| `crates/scholium-session/src/typst/events.rs` | 218 | 284 | +66 |
| `crates/scholium-session/src/typst/follow.rs` | 79 | 79 | +0 |
| `crates/scholium-session/src/typst/history.rs` | 158 | 158 | +0 |
| `crates/scholium-session/src/typst/history/snapshot.rs` | 29 | 29 | +0 |
| `crates/scholium-session/src/typst/indicator.rs` | 145 | 145 | +0 |
| `crates/scholium-session/src/typst/input.rs` | 550 | 570 | +20 |
| `crates/scholium-session/src/typst/key.rs` | 195 | 195 | +0 |
| `crates/scholium-session/src/typst/math_enter.rs` | 65 | 65 | +0 |
| `crates/scholium-session/src/typst/persistence.rs` | 178 | 185 | +7 |
| `crates/scholium-session/src/typst/pointer.rs` | 288 | 288 | +0 |
| `crates/scholium-session/src/typst/raster.rs` | 0 | 59 | +59 |
| `crates/scholium-session/src/typst/runtime.rs` | 0 | 213 | +213 |
| `crates/scholium-session/src/typst/runtime_tests.rs` | 0 | 68 | +68 |
| `crates/scholium-session/src/typst/selection.rs` | 139 | 139 | +0 |
| `crates/scholium-session/src/typst/style.rs` | 166 | 166 | +0 |
| `crates/scholium-session/src/typst/tests.rs` | 292 | 292 | +0 |
| `crates/scholium-session/src/typst/vertical.rs` | 326 | 326 | +0 |
| **合计** | **3853** | **4408** | **+555** |

## 本轮完整门禁

推送前 fetch 发现 main 已推进至 `936659a`（PR #71 的 LaTeX 文档）；已 fast-forward 同步，
没有改写该 PR 的文件。全部门禁在同步后重新运行。

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --locked --workspace` | PASS，443 passed / 2 ignored；app 302，session 11 passed / 1 ignored |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | PASS |
| `python3 scripts/check-rust-size.py` | PASS |
| `python3 scripts/check-doc-links.py` | PASS |
| `python3 scripts/check-ui-strings.py` | PASS，63 条原 allowlist，未扩充 |
| `python3 scripts/check-layers.py` | PASS |
| `cargo deny --locked check licenses bans sources` | PASS，既有 unused allowance 提示 |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | PASS |
| `python3 scripts/window-checks.py --locale zh-Hans en --out target/t20-window` | PASS，30/30，两种语言各 15 项 |
| `python3 scripts/verify-typst-fork.py target/t20-typst-origin` | PASS |
| `cargo run --locked -p scholium-typst --features editor --example typst-main-fork-check -- target/t20-fork-pixels` | PASS |
| `cargo run --locked --manifest-path spikes/typst-main-editor/reference/Cargo.toml --target-dir target/stock-reference -- target/t20-stock-pixels` | PASS |
| `python3 spikes/typst-main-editor/compare.py target/t20-fork-pixels target/t20-stock-pixels` | PASS，full/scripts/styles/heading 四项 equal_pixels |

完整命令输出在 slot 的 `target/t20-*.log`，窗口证据在 `target/t20-window/`。
测试与验证采用 `TMPDIR=$PWD/target/t20-tmp`，避免共享 `/tmp` 的 SQLite I/O 失败；
源码重建校验额外设置 `GIT_CEILING_DIRECTORIES=$TMPDIR`，防止临时目录中的 `git apply`
向上发现本工作树而跳过新增文件。未改验证脚本、源码补丁或校验断言。
Clippy 发现扩展后的共享场景测试函数超出行数限制，已提取指针 target 解析 helper，未禁用 lint 或改动断言。

## 验证边界与待办

本 PR 完成 T20 的 worker/Scene、viewport/raster 调度与抽象 deferred-input 迁移；
无编辑逻辑保留两份，既有 app 测试文件、断言、场景 JSON、窗口脚本及阈值均未修改。
ADR 0042 状态改为 Fully implemented，并链接三个阶段日志。

既有 `parity-check.py` 实际只启动各场景窗口，没有重放 inputs 或比较 expected；
本轮遵循窗口检查保持不变的约束，未扩充它。新增的 headless 路径确实通过真实 worker
重放输入并验证 Scene revision、caret 几何与零源码读取，但不将现有窗口 parity 入口的通过
表述为完整语义对照证据。该既有 parity 验证缺口需另行补齐。
