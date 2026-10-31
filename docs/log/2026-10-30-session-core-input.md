# 2026-10-30 — Session core phase 2a

## 范围

本阶段把 app 的文字提交、按键、页面指针、点击选区、视觉上下导航、Home/End、目标列和 follow 意图统一翻译为
`scholium_session::typst::Input`，并经 `EditorCore::input` 执行。app 只保留 egui/IME 适配、页面坐标换算、滚动、绘制、
AccessKit、对话框和 AUDIT；没有保留第二套编辑实现。拒绝输入的文字草稿、数学槽位选区拒绝和 pending style 清理均由
core 的输入事务处理，失败仍保持文档、端点和历史不变。

## 分阶段待办

为控制本 PR 的行为风险，排版 worker/Scene 所有权与 Scene 采纳，以及 app 的 deferred-input 队列仍留在 app；它们不再
承载编辑规则，列入下一 PR 的明确待办。下一 PR 完成后再更新 ADR 0042 的状态，不在本阶段宣称 Fully implemented。

## 验证

- `scholium-session` 的抽象输入回放和 rollback 测试覆盖文字、按键、指针、选区、公式和垂直导航。
- 共享场景位于 `spikes/typst-main-editor/scenarios/`；session 测试逐文件解析，窗口入口为 `parity-check.py`。
- 迁移后 app 单测初始剩余 5 个失败，修复 core 输入事务后 `cargo test -p scholium-app typst_editor` 为 205 passed；既有断言、窗口阈值和 workflow 未修改。

## 行数

下面的 app 行数以 `origin/main` 为迁移前基线，当前工作树为迁移后；包括 `typst_editor*` 下每个 Rust 文件。

| 文件 | 前 | 后 | 变化 |
| --- | ---: | ---: | ---: |
| `crates/scholium-app/src/typst_editor.rs` | 425 | 424 | -1 |
| `crates/scholium-app/src/typst_editor/access.rs` | 267 | 267 | 0 |
| `crates/scholium-app/src/typst_editor/access/runs.rs` | 255 | 255 | 0 |
| `crates/scholium-app/src/typst_editor/access/speech.rs` | 106 | 106 | 0 |
| `crates/scholium-app/src/typst_editor/access/tests.rs` | 395 | 395 | 0 |
| `crates/scholium-app/src/typst_editor/attach.rs` | 7 | 7 | 0 |
| `crates/scholium-app/src/typst_editor/attach/tests.rs` | 444 | 444 | 0 |
| `crates/scholium-app/src/typst_editor/audit.rs` | 94 | 94 | 0 |
| `crates/scholium-app/src/typst_editor/clipboard.rs` | 14 | 14 | 0 |
| `crates/scholium-app/src/typst_editor/command_keys_tests.rs` | 107 | 107 | 0 |
| `crates/scholium-app/src/typst_editor/confirm.rs` | 49 | 49 | 0 |
| `crates/scholium-app/src/typst_editor/core_adapter.rs` | 116 | 72 | -44 |
| `crates/scholium-app/src/typst_editor/deferred.rs` | 114 | 114 | 0 |
| `crates/scholium-app/src/typst_editor/diagnose.rs` | 122 | 116 | -6 |
| `crates/scholium-app/src/typst_editor/diagnose/tests.rs` | 112 | 112 | 0 |
| `crates/scholium-app/src/typst_editor/follow.rs` | 133 | 134 | +1 |
| `crates/scholium-app/src/typst_editor/follow_tests.rs` | 228 | 228 | 0 |
| `crates/scholium-app/src/typst_editor/history.rs` | 39 | 39 | 0 |
| `crates/scholium-app/src/typst_editor/history/tests.rs` | 209 | 209 | 0 |
| `crates/scholium-app/src/typst_editor/indicator.rs` | 21 | 21 | 0 |
| `crates/scholium-app/src/typst_editor/indicator/tests.rs` | 167 | 167 | 0 |
| `crates/scholium-app/src/typst_editor/input.rs` | 225 | 227 | +2 |
| `crates/scholium-app/src/typst_editor/math_enter.rs` | 9 | 9 | 0 |
| `crates/scholium-app/src/typst_editor/math_enter/tests.rs` | 172 | 172 | 0 |
| `crates/scholium-app/src/typst_editor/paint.rs` | 283 | 283 | 0 |
| `crates/scholium-app/src/typst_editor/raster.rs` | 152 | 152 | 0 |
| `crates/scholium-app/src/typst_editor/raster/layers.rs` | 253 | 257 | +4 |
| `crates/scholium-app/src/typst_editor/raster/tests.rs` | 165 | 165 | 0 |
| `crates/scholium-app/src/typst_editor/scene.rs` | 42 | 44 | +2 |
| `crates/scholium-app/src/typst_editor/selection.rs` | 316 | 308 | -8 |
| `crates/scholium-app/src/typst_editor/selection/pointer.rs` | 341 | 39 | -302 |
| `crates/scholium-app/src/typst_editor/selection/pointer_tests.rs` | 364 | 364 | 0 |
| `crates/scholium-app/src/typst_editor/selection/tests.rs` | 467 | 467 | 0 |
| `crates/scholium-app/src/typst_editor/source_view.rs` | 463 | 464 | +1 |
| `crates/scholium-app/src/typst_editor/source_view/tests.rs` | 121 | 121 | 0 |
| `crates/scholium-app/src/typst_editor/startup.rs` | 54 | 54 | 0 |
| `crates/scholium-app/src/typst_editor/startup/tests.rs` | 113 | 113 | 0 |
| `crates/scholium-app/src/typst_editor/style.rs` | 13 | 13 | 0 |
| `crates/scholium-app/src/typst_editor/style/tests.rs` | 258 | 258 | 0 |
| `crates/scholium-app/src/typst_editor/tests.rs` | 502 | 502 | 0 |
| `crates/scholium-app/src/typst_editor/vertical.rs` | 328 | 16 | -312 |
| `crates/scholium-app/src/typst_editor/vertical/tests.rs` | 503 | 503 | 0 |
| `crates/scholium-app/src/typst_editor/window_tests.rs` | 490 | 490 | 0 |
| **app 合计** | **9058** | **8395** | **-663** |

`crates/scholium-session/src/typst.rs` 与 `src/typst/*.rs` 合计：2297 → 3585 行（+1288）；按键实现另拆为 `key.rs`，所有文件低于 600 行。

## 待办

- 下一 PR 迁移 `EditorWorker`、Scene 所有权/采纳和 deferred-input 队列到 session core。
- 下一 PR 完成后，将 ADR 0042 从 Accepted（phase 1 + phase 2a）更新为最终实现状态。
