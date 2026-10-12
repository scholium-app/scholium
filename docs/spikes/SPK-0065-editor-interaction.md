# SPK-0065：编辑面基本交互修复

- 日期：2026-10-12；结论：**Pass**（限定范围：下列四项，数学槽内选区仍未接入）。
- 对应：[P1 排版工作区规划](../plan/P1_UI.md)的页面编辑、[ADR 0035](../adr/ADR-0035-typst-page-selection.md)
  的同 Frame 选区；阶段 0 条件 2 的编辑面可用性补强，不改变阶段判定。
- 基线：main `702374d`；分支 `ation_ciger/fix/editor-interaction`。

## 复现与根因

四项均在当前 main（A4 版式、逐字素数学文字、Attach）上先复现，再修。

1. **点击页面空白落进公式。** 旧 `pointer_hit` 在所有 caret（含公式槽位）里取二维最近点，
   版心下方没有正文 caret，最近的往往是公式内的槽位 caret。复现：三段文档（第二段含分数），在末行下方
   200 pt 处点击，光标落在末行首（byte 0），而非文档末尾（测试在旧实现上失败：期望 11，得到 0）。
   修复：点在公式 bounds 内才进入公式；否则只用正文精确 caret，按与上下键同一套视觉行划分
   （`vertical::Span` 合并重叠行），取纵向距离最小的行，再取 x 最近停靠点；首行之上取文档首，末行之下取文档末。
2. **双击不选词。** 根因是缺失而非回归：指针处理只有按下/拖动，没有点击次数。修复：画布用
   `Response::double_clicked/triple_clicked`，在释放时调用 `pointer_multi`；双击取指针所在叶的 UAX #29
   词段（指针在边界 caret 的哪一侧决定取前一段还是后一段，空白与标点各自成段），三击取该块全部正文 Text
   叶（首叶 0 到末叶末尾），写入既有 anchor/cursor。egui 的三击判定只看最近两次点击的时间，不核对中间一次的位置，
   所以另一处点击后 0.6 秒内的双击会被报成三击；窗口脚本因此在双击前等待 1 秒。
   数学槽内双击：选区模型只接受正文端点（见 app.md），因此保持光标并提示“数学槽位选区尚未接入”，不另造
   第二套选区。
3. **样式叶没有光标几何。** 根因在 `scholium-typst` 的 `content.rs`：origin 通过 `keep` 挂在 `StrongElem`/
   `EmphElem` 外壳上，而 Typst 的行内布局只为自身带 origin 的 `TextElem` 登记可映射范围
   （`typst-layout/src/inline/collect.rs`），Strong/Emph 在布局前已被 show 规则展开，外壳 origin 随之丢失，
   整片叶没有 cluster。旧版本正文没有样式时一直成立，A4 版式后更常被触发。修复：`style_text` 把 origin 直接放在
   最内层 `TextElem`。回归测试对 Strong 与 Emphasis，在“带重音、CJK、ZWJ 序列”的叶上断言起点、中间每个字素边界与
   末尾都有精确 caret，再输入一个字符并移动光标重排版后重复断言，同时断言状态栏没有“无法显示光标”提示。
4. **公式内没有模式提示。** 新增 `indicator.rs`：光标在公式内时取整个公式（行内 Math 拥有者根节点）的 Scene
   bounds，页面在其后画淡色圆角矩形（accent 约 10% 透明度），在当前 Fraction/Attach 槽外画更淡的描边
   （槽范围取该槽各叶 caret 的并集）；状态栏新增“公式”段（`EditorView::in_math`）。均为形状，文字仍只由 Typst 绘制；
   Scene 过期时不画。Esc 语义不变。

## 改动

- `scholium-typst/src/editor/content.rs`：样式叶的 origin 位置。
- `scholium-app`：`selection/pointer.rs`（命中、双击/三击）、`indicator.rs`、`paint.rs`（点击次数与覆盖层）、
  `chrome.rs`（“公式”段）、`typst_editor.rs`（`in_math`）、`vertical.rs`（仅放宽若干辅助项的可见性以复用划行规则）。
- 窗口脚本 `spikes/typst-main-editor/interaction-check.py`，并入 CI 窗口作业。
- 文档：`docs/modules/app.md`、`docs/modules/typst.md`。

## 证据

| 项 | 结果 |
| --- | --- |
| 单元测试 | `selection/pointer_tests.rs`（下/上/右侧/行间/公式内命中、词/空白/标点/CJK、三击与剪切、数学内双击）、`indicator/tests.rs`（公式/分数/Attach 槽/状态元数据/过期 Scene）、`tests.rs::styled_leaf_has_exact_carets_at_every_grapheme_before_and_after_relayout` |
| 失败在先 | 命中三项（下/上/右侧）在旧命中规则下失败；样式叶测试在旧 `content.rs` 下于 byte 0 失败 |
| 窗口脚本 | `interaction-check.py` 四项 PASS：公式淡色底仅在公式内出现、下方点击到文档末尾、双击选中 `world`（6..11）、三击选中整段 |
| 截图 | 公式内 [淡色底与“公式”状态](evidence/SPK-0065/interaction-check/01-caret-in-formula.png)；离开后 [无淡色底](evidence/SPK-0065/interaction-check/02-caret-in-body.png)；[双击选词](evidence/SPK-0065/interaction-check/03-double-click-word-selected.png)；[三击选段](evidence/SPK-0065/interaction-check/04-triple-click-block-selected.png) |

## 复现

```sh
cargo test --locked --workspace
cargo build --locked -p scholium-app -p scholium-storage --bins
xvfb-run -a -s '-screen 0 1280x900x24' \
  python3 spikes/typst-main-editor/interaction-check.py target/debug/scholium-app /tmp/scholium-interaction
```

## 未做

- 数学槽内的选区（双击选数学文字节点）依赖选区模型接受数学端点，仍未接入。
- 窗口脚本未覆盖真实窗口里的“粗体后 caret”，该项由 Scene 级回归测试固定。
