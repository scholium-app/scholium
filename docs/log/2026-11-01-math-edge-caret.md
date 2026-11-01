# 数学结构边缘光标与结构删除

2026-11-01，T30 修复了分数槽位方向键直接跳出公式、公式后正文段首 Backspace 不能逐层进入公式的问题。
工作位于指定 `scholium-wt1`，基线为 `0aa825e4a6d4d6e582cae043d87e94e8e0b743b8`。
设计决定见 [ADR 0045](../adr/ADR-0045-math-edge-caret.md)。

## 实现

- `scholium-typst/editor/stops.rs` 统一了 Fraction/Attach 的逻辑停点顺序与几何停点生成。
  复合节点 ID 的 `byte=0/1` 表示前/后；Text/Hole 保持原来的 UTF-8 偏移。
  父 Row 相邻 Text/Hole 提供同侧边界时省略复合节点边缘。遍历空间按节点数增长，只扫描当前叶的字素。
- `editor/edge_geometry.rs` 使用当前 Scene 的现有结构 bounds 合并结果，x 取结构左右界，y 取整公式高度。
  没有添加排版 Content、修改字体或改变栅格输出。数学指示底色与外层槽框适用于边缘光标。
- `scholium-session/typst/math_caret.rs` 处理边缘导航、输入和新结构命令；`structural_delete.rs`
  先删除同叶字素，再移除空结构/空公式，否则按方向只移动一个停点，不生成历史，也不重置输入分组。
  空 optional Attach 槽仍优先 RemoveSlot。T27 块合并继续适用于直接以公式开头或结尾的导入块。
- `scholium-document/structured_session/math_edge.rs` 在同一临时权威副本内原子插入兄弟、移除空结构/公式。
  根或槽根按需包 Row；边缘插入的文字删空后摘除该叶并折叠单子 Row，恢复真实边缘光标。
  没有更改文档 schema、存储格式，也没有常驻锚点叶。显式 LeaveSlot API 的 Hole 生命周期保持兼容。
- Home/End 从边缘到所在槽 Row（无槽则整公式）的规范首尾停点；Tab、上下键及正文 Shift 选择沿用原契约。
  公式内距离边缘 0.5 pt 内的点击优先命中边缘；正文视觉行命中路径保持原有职责。
- App 新增中英文 Fluent 边缘播报，AccessKit 正文文本与偏移不加入结构文字。Undo/redo 恢复完整边缘 Position。

同时更新了 document/session/typst/app 模块规范、体验规范与 ADR 索引，没有修改共享开发日志索引或 spike 状态。

## 先失败、后修复

以下失败输出保存在本次证据目录中；行号反映当时测试文件，后来格式化和补充用例改变了行号。

- 新增八个会话测试，执行 `cargo test --locked -p scholium-session math_edge_tests -- --nocapture`，
  八个均失败：缺失双向边缘、Backspace/Delete 不能进入、边缘输入 WrongTarget、空结构不移除等。
  [原始失败输出](evidence/math-edge-caret/session-tests-first.txt)。
- Typst 几何用例首先因无 edge caret 失败。早期 fixture 从 Hole 调整为文字，以避开既有 Hole 预览差异；
  提交的失败记录已使用文字 fixture，失败点为真正缺失边缘几何。
  [几何失败输出](evidence/math-edge-caret/geometry-tests-first.txt)。
- AccessKit 用例首先读到整公式描述而不是「公式内，分数前」。
  [播报失败输出](evidence/math-edge-caret/speech-tests-first.txt)。
- 补充导入文档公式块边界用例，首先发现两块未合并。
  [块边界失败输出](evidence/math-edge-caret/block-boundary-tests-first.txt)。
- 补充显式 LeaveSlot Hole 的空范围替换用例，首先发现 `Ok(true)` 而不是 `Ok(false)`。
  修复为只有非空删除范围才触发空叶清理，保留显式占位和空操作语义。
  [占位兼容失败输出](evidence/math-edge-caret/placeholder-tests-first.txt)。
- 补充边缘 Home/End 用例，首先发现 End 落到被相邻文字抑制、没有几何的边缘。
  修复为使用包含该边缘的槽 Row 或公式的规范首尾位置。
  [Home/End 失败输出](evidence/math-edge-caret/home-end-tests-first.txt)。

最终新增用例覆盖嵌套 Fraction/Attach、双向顺序、同 Row 去重、空结构和空公式删除、字素删除、纯移动历史、
双边 typed/IME commit/paste、清空后返回边缘、结构命令、撤销重做、非法编辑原子拒绝及块边界合并。
共享 `scenarios/math-edge-caret.json` 冻结了每步光标与历史深度，并同时驱动 headless 与真实窗口。
headless 还验证了当前 Scene 的精确 caret、数学指示与边缘 pointer hit。

## 门禁

以下命令全部通过，机器原始日志保留在 `target/t30-final-*.log`。
[门禁摘要](evidence/math-edge-caret/gate-summary.json)、
[测试计数](evidence/math-edge-caret/workspace-test-summary.txt)。

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps
python3 scripts/check-rust-size.py
python3 scripts/check-doc-links.py
python3 scripts/check-ui-strings.py
python3 scripts/check-layers.py
cargo deny --locked check licenses bans sources
cargo build --locked -p scholium-app -p scholium-storage --bins
```

工作区 489 个测试通过、4 个既有测试忽略。cargo deny licenses/bans/sources 通过，只有既有未匹配允许项提示。
新增 UI 文案进入两套 Fluent catalog，没有扩大硬编码 allowlist。

按工作流执行 stock Typst 对照，并为边缘增加独立 reference 的 `edge-root`、`edge-row` 夹具：

```sh
cargo run --locked -p scholium-typst --features editor --example typst-main-fork-check -- target/t30-stock/system-fork
cargo run --locked --manifest-path spikes/typst-main-editor/reference/Cargo.toml --target-dir target/stock-reference -- target/t30-stock/system-reference
python3 spikes/typst-main-editor/compare.py target/t30-stock/system-fork target/t30-stock/system-reference
```

相同三条命令另在 `SCHOLIUM_TEST_FONTS=bundled` 下执行，输出到 `bundled-fork` / `bundled-reference`。
System 与 Bundled 各 14/14，均 RGBA 逐像素完全相等，未放宽阈值。
[System 结果](evidence/math-edge-caret/stock-system.json)、
[Bundled 结果](evidence/math-edge-caret/stock-bundled.json)。

真实窗口最终执行：

```sh
python3 scripts/window-checks.py --locale zh-Hans en --out target/t30-final-window
```

中英文各 19 项，最终 **38/38 全部通过**，没有跳过或重试。
[窗口汇总](evidence/math-edge-caret/window-summary.txt)、
[中文逐步 AUDIT](evidence/math-edge-caret/steps-zh-Hans.json)、
[英文逐步 AUDIT](evidence/math-edge-caret/steps-en.json)、
[中文最终对比](evidence/math-edge-caret/result-zh-Hans.json)、
[英文最终对比](evidence/math-edge-caret/result-en.json)。
两语言最终语义 diff 均为空。完整原始日志、截图、临时 SQLite 留在忽略的 `target/t30-final-window/`，
只提交精简语义证据和代表截图。人工查看了中文场景的
[结构后缘](evidence/math-edge-caret/edge-10-zh-Hans.png)与
[结构前缘](evidence/math-edge-caret/edge-16-zh-Hans.png)截图：光标可见，数学指示底色正常；
第一张分母仍为 `aaa`，第二张为按场景删除一个字素后的 `aa`。

## 既有测试预期与交付边界

本次是交互行为变更。既有 Attach/Enter 测试从方向键生成临时 Hole 改为边缘 Position；
显式 LeaveSlot 兼容路径仍有独立测试。native 窗口脚本从一步跳出嵌套分数改为三步，分别经过
内分数后缘、外分数后缘、正文边界；重开后点击与右边缘重合的位置明确断言边缘命中，再按 Right 回到正文
继续整公式选择。原保存重开、Unicode、选区、撤销和像素阈值断言保留。
首轮窗口结果为 36/38，期间有脚本修订，只用于定位问题，不计为最终门禁；
两个 native 旧预期修正后单独复验为 2/2，随后以最终脚本执行全量门禁。

SSH fetch 不可用，改用相同仓库 HTTPS origin 及既有 gh 认证完成 fetch/push；没有更改源码访问权限。
Mogan/TeXmacs 只用于停点和删除设计结论，没有复制代码。所有 GUI 验证使用 runner 的机器级锁与私有 Xvfb，
没有访问用户桌面，没有启动子代理。

数学内部范围选区、协作光标及尚未存在的 Root/Delimiter 节点属于后续范围，未扩大本任务。
本次无模型或存储迁移。[完整改动文件清单](evidence/math-edge-caret/changed-files.txt)。
按要求推送并创建面向 main 的 PR，不合并、不轮询 CI。
