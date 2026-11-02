# T31：光标处分数与全结构光标审计

日期：2026-11-01。执行槽：`scholium-wt2`；分支：`ation_ciger/fix/fraction-at-caret`。
基线：`206b2336bf1a693406e47a1328d0837aa66fa640`。

## 问题与结果

在公式中输入 `abc` 后按 Ctrl+/，旧实现把完整叶放进分子。本次以 `FractionAt` 原子规划替换了
用户命令的 `WrapFraction`：无选区时在光标处插入空分数，前后文字留在同一数学行，进入空分子；
同一数学行选区只包装所选部分并进入空分母。Hole、结构边缘和嵌套槽位使用同一落点规则。
菜单、工具栏与快捷键复用同一核心命令，一次 Undo 恢复原内容、caret 和 anchor。
底层显式 `WrapFraction` 保留原有整子树 API 合约。

初始两个失败测试分别复现了整叶包装及 `Rejected(StructuralSelection)`；
[修复前输出](evidence/2026-11-01-fraction-at-caret/fraction-regressions.txt)保留。
随后增加了 Shift+左右的同一数学行选区、完整结构按原子选择、当前 Scene 选区高亮。
跨槽/跨公式范围仍拒绝；普通数学选区替换与剪贴板能力未扩展。

## 审计与实际修复

表驱动 session 审计包含 21 个夹具：仅上标、仅下标、双标、分数 base、上标内分数、嵌套分数、
同 Row 相邻结构、Row 两侧文字替代边缘、空分数、空上标槽、空公式、组合字素/ZWJ、块首公式、
块尾公式、无正文导入公式、空正文叶夹持分数、空正文叶夹持公式 Row（标题）、相邻公式、
粗体/强调相邻公式、标题、RawMath 相邻分数。

每个夹具用独立停点 oracle 检查 Right 到末端、Left 精确逆序，停点不重复；
从末端 Backspace、起点 Delete 记录每一步。仅同叶相邻字素、可移除空槽/结构或空公式产生动作，
其余严格匹配方向键且不增加历史、journal 或重置输入 run。空 optional script 槽仍优先移除。
[最终审计输出](evidence/2026-11-01-fraction-at-caret/structure-audit-final.txt)记录了完整语义轨迹。

发现并修复的其它实际失败：

1. **整公式 Row 选区缺几何。** 光标处分数使公式根成为 Text/Fraction/Text Row；旧 app 对根调用
   `selection_node`，但 Row 没有自己的 Frame bounds，正文跨公式选区报 `Geometry`。
   新失败测试先复现，再以 `selection_math` 递归收集子树实际记录的 bounds（包括分数横线）修复，
   同时用于数学行选区。没有猜矩形或插入打印占位。
2. **公式前后空正文叶缺 caret。** 新窗口检查在 Escape 离开公式后找不到 AUDIT caret。
   补充两侧空正文叶的分数/Row 夹具后，四个正文端点的 Scene 几何失败得到复现。
   Scene 现在由相邻公式已有 Frame bounds 的对应边界补全缺失正文 caret，
   与公式内边缘保留不同身份，不增加排版 Content；新增原生检查逐步验证 exact caret 与非空 IME caret。

已有 T30 导航/删除没有在本次夹具中发现其它失败。嵌套边缘可共享 ADR 0045 的整公式高度和
0.5 pt 命中区域；新场景对这种位置明确断言既有首命中策略及相同 y/半径内 x，未修改命中规则。
既有唯一边缘的精确 pointer 断言保留。

## RawMath 裁决

最初审计发现 RawMath 夹具没有 Scene，因为 Content adapter 返回 `EditorError::Raw`，曾按任务的
超出 ADR 0045 时停止规则报告。[当时输出](evidence/2026-11-01-fraction-at-caret/structure-audit.txt)保留。
协调者随后明确保留 ADR 0012/0023 的既有设计：原串绝不在进程内编译，显示路径另立任务。

据此 RawMath 继续参与全部遍历/删除审计，几何循环只排除该夹具，并以单独真实 worker 测试断言
**错误的确为 `EditorError::Raw`，且携带原 RawMath 节点身份**，没有用字符串匹配代替类型检查。
其余 20 个可渲染夹具的每个 oracle 停点均具有有限、非空 Scene caret。
这不是放宽 RawMath 执行边界，也没有丢弃或替换文档内容；测试注释和 ADR 修订同步说明。

## 原生场景与独立对照

新增 `fraction-at-caret.json` 和 `structure-audit-walks.json` 同时由 headless 输入和真实窗口 AUDIT 重放。
前者覆盖工具栏插入、分子落点、Undo/Redo、完整双向文字/槽位走查、选中部分文字及既有分数后经插入菜单包装。
后者覆盖外层分数、双标 Attach、上标内分数的全部双向停点与 Backspace/Delete 进入；
冻结的 cursor/path/affinity、undo_depth 和完整语义快照在两种执行器之间逐项比较。
`fraction-at-caret-check.py` 自动进入双语言窗口发现，额外断言每步 exact AUDIT caret、非空 IME caret、
无选区几何错误并截图。所有窗口仅在 `scripts/window-checks.py` 的私有 Xvfb 中运行。

原先通过“先输入、再分数”建立已填分子/分母的旧测试，改成先明确 Shift 选择，
或插入后 Right 到分母，保留原来的语义/播报/布局断言；多事件分数插入测试改为断言前文字留在行中。
旧的“所有数学 Shift 均拒绝”测试改为仍不支持的垂直 Shift，新增水平 Shift 正向测试。

独立 stock fixture 新增 `fraction-at-caret`：`ab`、已填 `n/d`、`c` 在同一 Row，stock 端独立构造 Content。
System 和 Bundled 两个 profile 各 15 个夹具均逐字节 RGBA 一致，尺寸/像素阈值没有放宽。
[System 对照](evidence/2026-11-01-fraction-at-caret/stock-system.json)和
[Bundled 对照](evidence/2026-11-01-fraction-at-caret/stock-bundled.json)保存了哈希及计数。

两个 stock 对照步骤使用以下命令；Bundled 步骤对三条命令均设置 `SCHOLIUM_TEST_FONTS=bundled`，
并把输出目录中的 `system` 换成 `bundled`：

```sh
cargo run --locked -p scholium-typst --features editor --example typst-main-fork-check -- target/t31-stock-final/system/fork
cargo run --locked --manifest-path spikes/typst-main-editor/reference/Cargo.toml --target-dir target/stock-reference -- target/t31-stock-final/system/stock
python3 spikes/typst-main-editor/compare.py target/t31-stock-final/system/fork target/t31-stock-final/system/stock
```

## 验证

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --locked --workspace` | PASS：502 passed / 4 ignored |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | PASS |
| `python3 scripts/check-rust-size.py` | PASS |
| `python3 scripts/check-doc-links.py` | PASS |
| `python3 scripts/check-ui-strings.py` | PASS：58 个既有 allowlist 字串未增加 |
| `python3 scripts/check-layers.py` | PASS |
| `cargo deny --locked check licenses bans sources` | PASS（只有既有 unmatched allowance 提示） |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | PASS |

[代码门禁输出](evidence/2026-11-01-fraction-at-caret/code-gates.txt)记录了上述命令输出。
显式 ignored 的 4 个测试为已有基准/宿主字体等手动检查，不是本次新增跳过。

`python3 scripts/window-checks.py --out target/t31-window-all --locale zh-Hans en`
完整运行了 20 个检查 × 2 语言；首次为 **39/40**。唯一失败为中文 AT-SPI fixture 仍依赖旧分数包装语义，
并非产品辅助技术回归：在 `12` 后显式 Shift+Left 两次，再插入分数，保留原完整 math object/分母状态断言。
英文完整运行已经通过该修正后的 fixture。随后在同一机器锁/私有 Xvfb 下执行
`python3 scripts/window-checks.py --only a11y --out target/t31-window-a11y-fixed --locale zh-Hans en`，
**2/2 通过**。最终所有 **40 个检查/语言组合均有通过结果**。随后还执行了最终完整双语言门禁，见下方。
[完整运行输出](evidence/2026-11-01-fraction-at-caret/windows-all.txt)保留了首次失败；
[AT-SPI 修正重跑输出](evidence/2026-11-01-fraction-at-caret/windows-a11y-fixed.txt)保留了最终结果。

`python3 scripts/window-checks.py --out target/t31-window-final --locale zh-Hans en`
最终完整双语言运行 **40/40 通过，退出码 0**。
[最终完整窗口输出](evidence/2026-11-01-fraction-at-caret/windows-final.txt)保存了全部 20 项 × 2 语言的通过汇总。

新增检查在两种语言均通过：40 步 `fraction-at-caret` 与 66 步 `structure-audit-walks`，
同时验证冻结语义摘要、exact AUDIT caret、非空 IME caret 和无选区几何错误。
[光标处空分数截图](evidence/2026-11-01-fraction-at-caret/fraction-at-caret-4.png)和
[同一行选区包装截图](evidence/2026-11-01-fraction-at-caret/fraction-at-caret-39.png)经人工查看，
保留 `ab`/`c` 在行中的位置与新分子/分母光标。

## 范围与待办

- 文档更新了 document/session/app/typst 模块合约、体验说明和 ADR 0045 的日期修订。
- 没有新增依赖、修改 schema/存储格式、修改 Git 配置、合并 PR 或轮询 CI。
- **待办：RawMath 独立显示/导入路径另立任务**；本次按裁决继续显式布局拒绝。
- 跨槽/跨公式数学选区、通用数学选区替换、数学拖选与协作光标不在本次范围。

完整[改动文件清单](evidence/2026-11-01-fraction-at-caret/changed-files.txt)随证据保存。
