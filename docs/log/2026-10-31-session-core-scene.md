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

迁移前为 `6fa1469`；迁移后为本轮评审修复，包含全部 Rust 文件及既有测试，0 表示新文件。

| 文件 | 前 | 后 | 变化 |
| --- | ---: | ---: | ---: |
| `crates/scholium-app/src/typst_editor.rs` | 417 | 407 | -10 |
| `crates/scholium-app/src/typst_editor/access.rs` | 267 | 267 | +0 |
| `crates/scholium-app/src/typst_editor/access/runs.rs` | 255 | 255 | +0 |
| `crates/scholium-app/src/typst_editor/access/speech.rs` | 106 | 106 | +0 |
| `crates/scholium-app/src/typst_editor/access/tests.rs` | 395 | 395 | +0 |
| `crates/scholium-app/src/typst_editor/attach.rs` | 7 | 7 | +0 |
| `crates/scholium-app/src/typst_editor/attach/tests.rs` | 444 | 444 | +0 |
| `crates/scholium-app/src/typst_editor/audit.rs` | 94 | 100 | +6 |
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
| `crates/scholium-app/src/typst_editor/input.rs` | 227 | 238 | +11 |
| `crates/scholium-app/src/typst_editor/math_enter.rs` | 9 | 9 | +0 |
| `crates/scholium-app/src/typst_editor/math_enter/tests.rs` | 172 | 172 | +0 |
| `crates/scholium-app/src/typst_editor/paint.rs` | 283 | 274 | -9 |
| `crates/scholium-app/src/typst_editor/raster.rs` | 152 | 103 | -49 |
| `crates/scholium-app/src/typst_editor/raster/layers.rs` | 257 | 213 | -44 |
| `crates/scholium-app/src/typst_editor/raster/tests.rs` | 165 | 165 | +0 |
| `crates/scholium-app/src/typst_editor/scene.rs` | 44 | 38 | -6 |
| `crates/scholium-app/src/typst_editor/selection.rs` | 308 | 299 | -9 |
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
| **合计** | **8355** | **8212** | **-143** |

## scholium-session 行数

迁移前为 `6fa1469`；迁移后为本轮评审修复，包含全部 Rust 文件及既有测试，0 表示新文件。

| 文件 | 前 | 后 | 变化 |
| --- | ---: | ---: | ---: |
| `crates/scholium-session/src/lib.rs` | 2 | 2 | +0 |
| `crates/scholium-session/src/typst.rs` | 193 | 206 | +13 |
| `crates/scholium-session/src/typst/attach.rs` | 135 | 135 | +0 |
| `crates/scholium-session/src/typst/checkpoint_bench.rs` | 90 | 90 | +0 |
| `crates/scholium-session/src/typst/command.rs` | 291 | 293 | +2 |
| `crates/scholium-session/src/typst/command/checkpoint.rs` | 145 | 145 | +0 |
| `crates/scholium-session/src/typst/command/checkpoint/tests.rs` | 68 | 68 | +0 |
| `crates/scholium-session/src/typst/deferred.rs` | 0 | 110 | +110 |
| `crates/scholium-session/src/typst/diagnose.rs` | 41 | 41 | +0 |
| `crates/scholium-session/src/typst/edit.rs` | 60 | 60 | +0 |
| `crates/scholium-session/src/typst/events.rs` | 218 | 154 | -64 |
| `crates/scholium-session/src/typst/follow.rs` | 79 | 79 | +0 |
| `crates/scholium-session/src/typst/history.rs` | 158 | 158 | +0 |
| `crates/scholium-session/src/typst/history/snapshot.rs` | 29 | 29 | +0 |
| `crates/scholium-session/src/typst/indicator.rs` | 145 | 145 | +0 |
| `crates/scholium-session/src/typst/input.rs` | 550 | 541 | -9 |
| `crates/scholium-session/src/typst/key.rs` | 195 | 195 | +0 |
| `crates/scholium-session/src/typst/math_enter.rs` | 65 | 65 | +0 |
| `crates/scholium-session/src/typst/persistence.rs` | 178 | 185 | +7 |
| `crates/scholium-session/src/typst/pointer.rs` | 288 | 325 | +37 |
| `crates/scholium-session/src/typst/projection.rs` | 0 | 70 | +70 |
| `crates/scholium-session/src/typst/raster.rs` | 0 | 59 | +59 |
| `crates/scholium-session/src/typst/runtime.rs` | 0 | 213 | +213 |
| `crates/scholium-session/src/typst/runtime_tests.rs` | 0 | 109 | +109 |
| `crates/scholium-session/src/typst/scenario_tests.rs` | 0 | 150 | +150 |
| `crates/scholium-session/src/typst/selection.rs` | 139 | 139 | +0 |
| `crates/scholium-session/src/typst/style.rs` | 166 | 166 | +0 |
| `crates/scholium-session/src/typst/tests.rs` | 292 | 292 | +0 |
| `crates/scholium-session/src/typst/vertical.rs` | 326 | 326 | +0 |
| **合计** | **3853** | **4550** | **+697** |

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


## PR #73 评审：真实 parity 负对照与阻塞（未完成交付）

本轮在同一分支 `ation_ciger/refactor/session-core-scene`、提交 `8b3503a` 后继续；开始时工作树干净，
fetch 后 main 未推进。未修改任何 scenarios JSON、frozen expected、既有测试断言或阈值。

本地重写了 `parity-check.py`：逐输入回放文字、按键、命令及由 AUDIT caret 解析的指针目标，
记录每一步的规范化快照、cursor/anchor、undo_depth；窗口不存在、超时、字段缺失或 expected 差异均失败。
AUDIT 新增 anchor 三字段与 undo/redo 深度，只增加观测，没有改变编辑行为。
随机 document/node 身份与 revision 从快照内容比较中排除，cursor/anchor 用稳定结构路径与 byte/affinity 比较。
功能键按 `chrome_ui.keycode` 发送，CJK 与含空白的一次文字提交使用剪贴板，短 ASCII 使用 xdotool。
无界面测试新增 frozen undo_depth 核对，原有断言保留。实现尚未完成共享 expected 的全字段验收，不能称为门禁通过。

### 先做负对照

在 `target/t20-parity-fixtures/negative/` 复制 mixed-cjk-english 场景，保持文字输入不变，
为该临时副本写入明确的正文摘要、cursor（UTF-8 byte 24 / Upstream）、null anchor，
仅将 undo_depth 故意改为 999（仓库原值为 1）。
以 machine-wide lock + private Xvfb 启动真实 app；检查器退出码 **1**，唯一字段差异：

```json
{"undo_depth":{"expected":999,"actual":1}}
```

输出：`target/t20-parity-negative.log`；最后 AUDIT、逐步结果和字段差异在
`target/t20-parity-negative/mixed-cjk-english/`。负对照没有改动仓库场景。
复现入口：

```sh
env -u WAYLAND_DISPLAY SCHOLIUM_LOCALE=zh-Hans TMPDIR="$PWD/target/t20-tmp" flock /tmp/scholium-window-checks.lock xvfb-run -a -s '-screen 0 1280x900x24' python3 spikes/typst-main-editor/parity-check.py target/debug/scholium-app target/t20-parity-negative --scenarios target/t20-parity-fixtures/negative
```

复现时使用新的 output 路径；检查器拒绝覆盖已有场景证据目录。

### 未通过的固定验收

1. 所有七份原场景的 expected 都只有 undo_depth，缺少 snapshot、cursor、anchor。
   严格检查将其报告为缺少 expected 字段，未自动生成/接受输出作为 expected。
2. `formula-fraction-superscript`：无界面与窗口最终内容、cursor、anchor、undo 均相同，
   但两边 undo_depth 都为 **5**，原 frozen expected 为 **7**。快照实际是正文空叶、分数 12/3、正文 `^2`，
   最终 cursor 为 `blocks/0/content/2` byte 2 / Downstream，anchor=null。
   第 5 步 ArrowRight 已离开公式，第 6 步 `^` 成为正文；原场景没有得到名称所说的上标。
   窗口检查退出码 **1**。规范化两端原始对照：`target/t20-parity-formula-comparison.json`；
   窗口最终证据：`target/t20-parity-formula-character/formula-fraction-superscript/result.json`。
3. `triple-click-delete` 在第 **2** 步出现两端语义差异：JSON 为
   `{"pointer":{"target":"end of leaf 0","kind":"Release","clicks":3}}`。
   无界面直接发送 PointerKind::Release，core 只结束拖选，不会选段；其后 Delete 在末尾不改变正文。
   窗口三击会经过真实 press/release，选中整段（anchor byte 0 / Downstream，cursor byte 19 / Upstream）；
   第 **3** 步 Delete 后全文为空、cursor byte 0 / Upstream、undo_depth=**2**。
   无界面保留 `paragraph to delete`，cursor byte 19 / Upstream、anchor=null、undo_depth=**1**。
   frozen expected 为 **2**，只符合窗口。证据：`target/t20-parity-baseline.log`、
   `target/t20-parity-triple-commit/triple-click-delete/steps.json`。

初次 ASCII 多词输入通过逐键事件产生了三个撤销组，已将这类一次性文字提交改用剪贴板后再定位上述差异；
未用输入节奏/修改 expected 消除真实指针语义差异。初次 Character `^` 用 keysym 未产生文字，
已改为 xdotool type 后，公式两端内容完全一致。前两次仅为回放器实现调试，不作为最终差异证据。

遵循评审“若某场景在窗口中无法与无界面一致，停下汇报”的要求，现已停止：
**未重跑整套门禁、未完成双语窗口验收、未 commit/push 本地修订**。本地 ADR 状态改回 Accepted，
不得以旧的空 parity 入口 30/30 结果宣称本轮 Fully implemented。PR 描述标明阻塞，不合并、不轮询 CI。
在保持 frozen expected 不变的约束下，不能把公式的 5 改为 7，也不能静默改写三击场景的 Release 为 Press。


## PR #73 继续修订：统一多击与实质 parity

收到评审继续指令后保留未提交的回放器修订，没有重新建分支。已 fetch 并签署 merge
`origin/main` 的 `3bfc176`（README 与 upstream tracking）；本 PR 未修改带入的其他任务文件。

### 先失败，再修复多击入口

先新增无界面回归：输入 `paragraph to delete`，等待真实 Scene，在段末发送带点击数的
`Input::Pointer { kind: Release, clicks: 2/3 }`，断言选词/选段，再 Delete，断言正文和 undo=2。
修复前失败于 `selection_text` 为 None（预期 `delete`），修复后两个分支均通过。
[失败记录](evidence/2026-10-31-session-core-scene/multiclick-before.txt)、
[修复后记录](evidence/2026-10-31-session-core-scene/multiclick-after.txt)。

准确原因是：词/段范围算法 `pointer_multi` 已在 core，但此前只从 Press 调用；
app 绘制后的 egui Response 再补发 Press，掩盖了 Release 入口没有完成多击选择的问题。
本轮在 core 的 Release 分发里调用同一个 `pointer_multi`，并结束拖选；Press 与 Release 共享选词/选段算法。
app 绘制后的 triple_clicked/double_clicked 分支删除，不保留生产副本。
原回调中的无精确命中提示改为按 core Outcome.applied 呈现，保留数学位置无法选词时的既有提示，不再次执行选择。
既有 app 测试调用的 `pointer_clicks` 仅留 cfg(test) 适配，仍转调 core，未修改既有测试断言。
app 的 ClickRun 只负责原生时间/像素距离转点击数；选择范围、端点与删除语义都来自 core。

### 公式场景撤销推算

按当前 session `history::joins/note_run/push_undo` 与模块规范，并对照
[按词撤销日志](2026-10-13-undo-grouping.md)：同叶、连续 caret、间隔 <1 秒的 Typed 插入共组；
公式/结构命令另起一步；导航打断连续键入但不自己增加历史项；离开公式已有的后置正文叶只移动光标。
本场景 worker 结果在输入间等待，但不引入人为一秒停顿。

| 步 | 原始输入 | 语义与分组依据 | 新增撤销项 | 累计 |
| --- | --- | --- | ---: | ---: |
| 1 | Text `$` | Typed 分隔符进入公式，创建数学结构 | 1 | 1 |
| 2 | Text `12` | 数学叶首次输入，独立插入组 | 1 | 2 |
| 3 | Ctrl+/ | 结构命令建立分数，并移动到分母 | 1 | 3 |
| 4 | Text `3` | 分母新叶首次输入 | 1 | 4 |
| 5 | ArrowRight | 离开分母/公式到已有后置正文空叶；导航无文档修改 | 0 | 4 |
| 6 | Character `^` | 此时在正文，字面输入，开始正文插入组；不会创建上标 | 1 | 5 |
| 7 | Text `2` | 与 `^` 同叶、相邻且未超时，继续同组 | 0 | 5 |
| 8 | ArrowRight | 正文末尾导航，无文档修改 | 0 | 5 |

因此按本次明确授权，将这个场景的 expected.undo_depth 从 **7 改为 5**，不修改输入或放宽窗口判定。
新增逐输入深度断言 `[1,2,3,4,4,5,5,5]` 验证推算；两端最终正文为 `^2`，并非数学上标。
原场景名称保持；真实上标行为仍由既有 math-attach 窗口检查及 app/session 单测覆盖。

### 单一稳定摘要与窗口回放

新增生产接口 `EditorCore::semantic_projection()`，headless 与 app AUDIT.semantic 都直接调用它：
完整快照仅剥离随机 node/document 身份及 revision，保留每个内容/样式字段；cursor/anchor 为结构路径、
UTF-8 byte 与 affinity，anchor 无值时为 null；并输出 undo_depth。
`serde_json` 从该 crate 的 dev dependency 改为普通 dependency，版本保持锁定，没有新增包或修改 lockfile。
共享场景测试按职责移至 scenario_tests.rs，既有 Scene/current-caret/source_reads 断言原样保留。
七个场景补齐同一 expected 的四项；窗口不自行生成摘要，不读取 headless 结果作为期望，也不容忍差异。

parity-check 逐项重放全部 inputs：CJK/整段文字经剪贴板，短 ASCII 与字面字符经 xdotool，
命令映射为原生和弦，功能键按 chrome_ui.keycode。指针按 AUDIT 的稳定叶目标定位 exact caret，
其 x/y 已由 app 完成页面 pt→窗口像素换算，再由 xdotool --window 转到实际窗口坐标。
三击 Release 在窗口由完整原生多击序列实现；core 的 Release 现在独立提供相同选择语义。
每步保存规范化结果，最后读取已稳定且 current/deferred=0 的最后 AUDIT，严格比较全部 expected 字段；
未知事件/按键、找不到 PID 对应可见窗口、超时、app 退出、布局错误、缺失 expected 或任何字段差异均非零退出并打印差异。

[新的负对照](evidence/2026-10-31-session-core-scene/parity-negative.txt)再次以当前共享摘要实现运行，
临时副本仅把 undo_depth 改坏为 999，结果仅该字段 expected=999/actual=1，退出码 **1**；
[完整 expected/actual/diff](evidence/2026-10-31-session-core-scene/parity-negative.json)。仓库场景未被负对照修改。
先前停止记录描述的是修复前状态，本节按后续授权解决该阻塞，不覆盖追溯记录。

### 本轮门禁

最终代码（含原数学双击无命中提示）通过以下全部门禁后，ADR 0042 恢复为 Fully implemented，链接三个阶段日志。

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --locked --workspace` | PASS，444 passed / 2 ignored；app 302，session 12 passed / 1 ignored |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | PASS |
| `python3 scripts/check-rust-size.py` | PASS |
| `python3 scripts/check-doc-links.py` | PASS |
| `python3 scripts/check-ui-strings.py` | PASS，63 条原白名单，未扩充 |
| `python3 scripts/check-layers.py` | PASS |
| `cargo deny --locked check licenses bans sources` | PASS，既有 unused allowance 提示 |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | PASS |
| `python3 scripts/window-checks.py --locale zh-Hans en --out target/t20-review-final-window` | PASS，30/30；zh-Hans/en 各 15 项，含全部七场景实质 parity |
| `python3 scripts/verify-typst-fork.py target/t20-typst-origin` | PASS，固定上游 9dfd3a08500b7896045f907433cf7b4b02434fad |
| `cargo run --locked -p scholium-typst --features editor --example typst-main-fork-check -- target/t20-review-final-fork-pixels` | PASS |
| `cargo run --locked --manifest-path spikes/typst-main-editor/reference/Cargo.toml --target-dir target/stock-reference -- target/t20-review-final-stock-pixels` | PASS |
| `python3 spikes/typst-main-editor/compare.py target/t20-review-final-fork-pixels target/t20-review-final-stock-pixels` | PASS，4/4 equal_pixels |

[完整窗口摘要](evidence/2026-10-31-session-core-scene/window-summary.txt)、
[14 份逐场景 expected/actual/diff](evidence/2026-10-31-session-core-scene/parity-results.json)、
[stock 对照](evidence/2026-10-31-session-core-scene/stock-comparison.json)、
[输入序列/授权期望变更核对](evidence/2026-10-31-session-core-scene/scenario-contract.json)。
命令输出在本 slot 的 `target/t20-review-final-*.log`；逐步 AUDIT 与原生输入轨迹在 `target/t20-review-final-window/`。
保持 slot 内 TMPDIR 和 Git ceiling 的验证环境设置，与前轮相同；不触碰桌面，不绕过机器窗口锁。

本轮修订没有修改七个场景的 inputs、既有 app 测试断言、其他窗口脚本、workflow、stock reference 或阈值。
仅按本次评审授权修订 parity 入口、补齐 expected，并将有逐步规则依据的公式 undo 7→5。
T20 及本轮四项评审要求已完成；没有遗留迁移或 parity 待办。不合并、不轮询 CI。


## PR #73 场景真实性修订（替代上一轮公式场景结论）

上一节保留名称却接受正文 `^2` 的决定不能证明上标一致性，本轮明确撤回该覆盖声明。
先按撤销规则推算再执行：删除分母 `3` 后的 ArrowRight，在分母内输入 `^`、`2`，
最后 Ctrl+M 从上标直接退出到现有后置正文空叶；不经 LeaveSlot，不生成临时 Hole。
第 6 步 expected 手工设为 Fraction(num=Text12, den=Attach(base=Text3, sup=Text2))，正文后置叶为空。
数学 Attach.sup 必须出现在摘要中。退出后输入句号，末光标 `blocks/0/content/2` byte=1 Upstream，anchor=null。

| 步 | 修正输入 | 规则推算（运行前） | 新增撤销项 | 累计 |
| --- | --- | --- | ---: | ---: |
| 1 | Text `$` | 分隔符建立 Math 结构 | 1 | 1 |
| 2 | Text `12` | 首次在数学叶键入，开始插入组 | 1 | 2 |
| 3 | Ctrl+/ | WrapFraction 结构命令打断键入，进入分母 Hole | 1 | 3 |
| 4 | Text `3` | 分母叶首次输入，开始新组 | 1 | 4 |
| 5 | Character `^` | 此时仍在分母；Attach(Sup) 结构命令创建上标 Hole，打断键入 | 1 | 5 |
| 6 | Text `2` | 上标新叶首次输入，开始新组 | 1 | 6 |
| 7 | Ctrl+M | 从上标直接退出到已有正文叶；只移动光标，无文档修改 | 0 | 6 |
| 8 | Text `.` | 公式后输入句号，正文新叶首次插入，独立一组 | 1 | 7 |

两端每一步都检查同一份 fixture 的 expected.undo_depth `[1,2,3,4,5,6,6,7]`；
第 6 步另检查整个含 Attach.sup 的快照和数学上标光标，第 8 步检查退出后的全部四字段。
若实际深度与表不符，停止，不按实际反填期望。原错误序列的 undo=5 推算仍是旧序列的追溯记录。

首次定向回放中前 6 步的深度与推算完全一致。第 7 步已退出公式，空后置正文没有独立 caret，
新增中间轨迹记录器最初要求每步都有 caret 而报错，尚未执行最终几何断言。
为保持既有最终 caret 必须存在的断言，运行前推算并加入第 8 步正文句号（+1，累计 7）。
中间记录允许空叶无 caret；垂直导航步骤与最终既有几何断言仍必须有 caret，没有修改既有断言。

### 七个名称的真实性审计

| 场景名称所述行为 | 快照/端点中可见证据（headless 与窗口使用同一 expected） | 输入修订 |
| --- | --- | --- |
| formula-fraction-superscript：分数与公式内上标 | 第 6 步快照为 Fraction(numerator=Text12, denominator=Attach(base=Text3, sup=Text2))，cursor 在 denominator/body/sup byte=1；末快照保留该节点，正文只有空前叶和后置句号，无字面 `^`；最终 undo=7 | 上标移到退出分母之前，Ctrl+M 退出，再输入正文句号 |
| vertical-goal：跨软换行保留列目标 | 快照为一个 373-byte Paragraph/Text，无换行符；从 byte12 下移到108、203，再上移到108、12；三条视觉行 y=76.16015625、96.48046875、116.80078125 pt；每步 cursor 必须落在相邻行距原始 x 最近的 exact caret，最终 byte12/Downstream，anchor=null、undo=1 | 原 56-byte 短句不能证明软换行；改为长的比例字体段落，Ctrl+Home 后 Right×12 定位，再 Down×2、Up×2 |
| double-click-bold：双击选词再加粗 | 第 2 步同叶 anchor=0/Downstream、cursor=4/Upstream，确实只选 `bold`；最终快照强样式 `bold` + 普通 ` word`，undo=2 | 输入不变，增加选词中间端点断言 |
| mixed-cjk-english：中英混排提交 | 单一 Text 精确保留 `中文 English paragraph`，cursor UTF-8 byte=24/Upstream，anchor=null、undo=1 | 输入与最终 expected 不变 |
| shift-selection-replace：Shift 选区替换 | 第 3 步 anchor=8、cursor=7（均 Downstream），选中字母 `m`；最终 `select Xe`、cursor byte8/Upstream、anchor=null、undo=2 | 去掉从未执行为重复次数的 `repeat:true` 标记，仍然只发送一次 ArrowLeft；增加替换前端点断言 |
| triple-click-delete：三击选段再删除 | 第 2 步 anchor=0/Downstream、cursor=19/Upstream，完整选中 `paragraph to delete`；末快照 Text 为空、cursor=0/Upstream、anchor=null、undo=2 | 输入不变，增加全段选区中间断言 |
| undo-redo：撤销再重做 | Ctrl+Z 后快照为空 Text、cursor=0/Downstream、anchor=null、undo=0；重做后恢复 `history`、cursor=7/Upstream、undo=1 | 输入不变，增加撤销后的完整摘要断言 |

垂直移动的目标 x=168.86773252383406 pt。四次移动实际 x 分别为
169.8892065888098、166.92801888533464、169.8892065888098、168.86773252383406 pt。
第 16 步另要求该 caret **不是**按上一行实际 x 最近的候选，确保输入能区分保留目标与重置目标。
这一步若误用上一行 x=169.8892065888098 pt 会选另一个字符；两端同一 fixture 的
`distinguish_previous_x` 检查拒绝这种空覆盖。新增 1e-4 只用于浮点坐标比较误差（窗口坐标经 f32 缩放）；
快照、端点、撤销均精确相等，不改已有任何窗口检查/stock 阈值。

回放器现在逐输入读取可选 expected（所列字段精确相等）；最终 expected 仍必须包含整个快照、
cursor、anchor、undo_depth 并全部精确相等。垂直步骤还读取真实 worker/current Scene 或 AUDIT carets，
检查相邻行、原始列目标和反漂移见证，headless/window 都会失败退出。
生产编辑实现没有变化；所有修订属于共享场景与验收器，不另建编辑路径。


负对照仍保留前轮 undo=999 的原始证据，本轮又仅在临时副本把**最终数学上标 expected 从 2 改为 9**，
输入和第 6 步期望保持正确；真实窗口在最后一步报告 snapshot 差异，退出码 **1**。
actual Attach.sup 为 Text2，cursor/anchor/undo_depth 均与期望一致，证明错误数学节点不能通过。
[新负对照日志](evidence/2026-10-31-session-core-scene/fidelity-negative.txt)、
[完整差异](evidence/2026-10-31-session-core-scene/fidelity-negative.json)。
[七场景 headless 逐步轨迹](evidence/2026-10-31-session-core-scene/fidelity-headless.json)保留快照、端点、深度及页面 pt 光标。

本次文件行数相对 `0f68c05`：app typst_editor* 每个文件均未变化，总计 **8212 → 8212**；
session 仅 scenario_tests.rs **150 → 241**（增加共享中间断言/几何检验并拆分回放助手），
其余文件不变，总计 **4550 → 4641**。相对 T20 基线 `6fa1469`，app **8355 → 8212（-143）**，
session **3853 → 4641（+788）**。
[逐文件三列核对](evidence/2026-10-31-session-core-scene/fidelity-line-counts.json)列出全部文件；生产 app 没有增加编辑逻辑。


### 修正场景后的最终门禁

所有门禁针对修正后的共享场景重跑通过。ADR 0042 在这次双语言完整验收通过后保持 Fully implemented。
原日志中的“输入不变、公式正文 ^2、undo=5”仅描述前一轮，已被本节的真实分数/上标场景替代。

| 命令 | 本轮结果 |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --locked --workspace` | PASS，444 passed / 2 ignored |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | PASS |
| `python3 scripts/check-rust-size.py` | PASS |
| `python3 scripts/check-doc-links.py` | PASS |
| `python3 scripts/check-ui-strings.py` | PASS，原白名单 63 条，未扩充 |
| `python3 scripts/check-layers.py` | PASS |
| `cargo deny --locked check licenses bans sources` | PASS，既有 unused allowance 提示 |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | PASS |
| `python3 scripts/window-checks.py --locale zh-Hans en --out target/t20-fidelity-final-window` | PASS，30/30，含两种语言各七个共享场景 |
| `python3 scripts/verify-typst-fork.py target/t20-typst-origin` | PASS，上游固定 9dfd3a08500b7896045f907433cf7b4b02434fad |
| `cargo run --locked -p scholium-typst --features editor --example typst-main-fork-check -- target/t20-fidelity-fork-pixels` | PASS |
| `cargo run --locked --manifest-path spikes/typst-main-editor/reference/Cargo.toml --target-dir target/stock-reference -- target/t20-fidelity-stock-pixels` | PASS |
| `python3 spikes/typst-main-editor/compare.py target/t20-fidelity-fork-pixels target/t20-fidelity-stock-pixels` | PASS，4/4 equal_pixels |

[窗口完整摘要](evidence/2026-10-31-session-core-scene/fidelity-window-summary.txt)、
[14 份窗口最终结果与逐步快照/端点](evidence/2026-10-31-session-core-scene/fidelity-parity.json)、
[stock 四项对照](evidence/2026-10-31-session-core-scene/fidelity-stock.json)、
[前后输入及 expected 变更核对](evidence/2026-10-31-session-core-scene/fidelity-contract.json)。

公式两端实际深度均为预先推算的 `[1,2,3,4,5,6,6,7]`，未出现推算与实际不符。
除明确授权的公式和 vertical-goal 场景外，其他五个场景的最终 expected 完全未变；
仅增加中间状态检查，并移除 shift 场景没有被执行的 repeat 标记。
首次 Clippy 指出回放测试函数超过 100 行，随后按输入翻译/按键解析/中间断言拆分助手，未改生产编辑实现。
既有最终 Scene、source_reads=0、caret 几何断言全部保留；其他窗口脚本、既有断言、stock reference 和阈值未改。
命令原始输出在 `target/t20-fidelity-*.log`；窗口 AUDIT、截图及逐步原生结果在 `target/t20-fidelity-final-window/`。
验证继续使用 slot 内 TMPDIR，fork 核验加 Git ceiling，窗口使用私有 Xvfb 与机器级锁。
fetch 确认 origin/main 仍为 3bfc176，已是当前分支祖先；没有需要追加的 main 合并。
本轮要求均已完成，无遗留项；不合并、不轮询 CI。
