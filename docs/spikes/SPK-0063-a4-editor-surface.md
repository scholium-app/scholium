# SPK-0063：编辑面与 A4 输出同版式

- 日期：2026-10-12；结论：**Pass**。
- 对应：[P1 排版工作区规划](../plan/P1_UI.md)的纸面编辑、[报告 0062](SPK-0062-structured-source-view.md)的
  A4 快速预览；所见即所得要求编辑面就是输出的版式。
- 基线：main `da9a39f`；分支 `ation_ciger/feat/a4-editor-surface`。

## 问题

编辑面与 A4 快速预览（Ctrl+2）版式不一致：编辑面宽 420 pt、内边距 10 pt、Typst 默认文字样式，
标题用 Strong + 20/16 pt 伪造；预览是 A4、页边距 2.5 cm / 2.54 cm、宋体/黑体 12 pt、两端对齐、
行距 1 em、首行缩进 2 em。字体、字号、断行、缩进都不同。

## 改动

| 参数 | 之前（编辑面） | 之后（编辑面 = 预览） |
|---|---|---|
| 版心宽 | 420 pt | 210 mm − 2×2.5 cm ≈ 453.5 pt |
| 内边距 | 10 pt | 左右 2.5 cm，上下 2.54 cm |
| 高度预算（内容） | 2000 pt | 2400 pt（≈ 3.4 个 A4 版心高 697.9 pt） |
| 正文 | Typst 默认样式 + 12 pt | Times New Roman / SimSun 12 pt，lang zh-CN |
| 段落 | 默认 | 两端对齐，行距 1 em，首行缩进 2 em（all） |
| 标题 | Strong + 20/16 pt | 真实 `HeadingElem`，Times New Roman / SimHei 粗体，同预览默认字号与间距 |
| 标题编号 | 无 | 无（预览 preamble 也去掉了 `numbering: "1.1"`） |

- 新 `scholium-typst/src/style.rs` 是文档样式的唯一定义（字体、字号、行距、缩进、语言、页边距、版心宽）。
  `preamble()` 由它生成预览 preamble；`editor/style.rs` 由同一组常量构造编辑面的 Content 样式。
- 标题编号移出 preamble：编辑面以空 introspector 布局，无法运行 counter；编号随文档模板回归。
  `HeadingElem` 在无 introspector 时可直接布局（编号未设置则不查 counter），故无需复刻标题样式的回退方案。
- 容量：内容高度 2000 pt → 2400 pt。按实测行距（正文行 ≈ 20.3 pt）约 118 行，约合 3.4 个 A4 版心；
  旧值约 124 行（默认 0.65 em 行距，估算）。纹理约 1191×5088 px，约 24 MB。超过预算仍拒绝而非截断；
  分页或虚拟化渲染不在本报告范围。
- 预览源码的块起点锚点原先写在标题正文之前（`= #metadata(..) <blkstart..>标题`），实测会改变标题块的间距
  （标题 1 → 标题 2 基线距 34.6 pt，无锚点 27.2 pt）。现在标题的起点与终点锚点各占一行，放在 `=` 之前和标题行之后；
  正文段落的锚点位置不变。
- 空块不设首行缩进：fork 以行文本锚定空行 caret，缩进的占位字符使该行失去 caret
  （空段落竖向停靠、AT-SPI 空段 run 几何的既有测试因此失败）。代价：空段落里的 caret 在页边而非缩进处，
  键入第一个字符后移到缩进位置。

## 断行一致性测试

`editor::parity::editor_and_preview_break_every_block_into_the_same_lines`：同一夹具文档
（标题 1、标题 2、两段中英混排且各 3 行以上、第二段含一个行内公式）经编辑面（`ContentCache` + `kernel::layout`）
和预览编译路径（`generate_source(Preview)` + `typst::compile`）各排一次，字体取自同样的 `FontStore`
（内置 + 系统）。断言：每块行数相同、每行首字符相同、整行文本相同，且各行基线相对首行的偏移相差小于 0.5 pt
（涵盖标题字号、间距与行距）。

## 复现

```sh
cargo test --locked -p scholium-typst --features editor parity
cargo build --locked -p scholium-app -p scholium-storage --bins
xvfb-run -a -s '-screen 0 1280x900x24' \
  python3 spikes/typst-main-editor/a4-surface-check.py target/debug/scholium-app /tmp/scholium-a4
```

`a4-surface-check.py` 用 Ribbon 的“标题 1/2”按钮坐标设置标题，只用于取证，未并入 CI 窗口作业。

## 结果

| 检查 | 结果与证据 |
|---|---|
| 工作区测试 | 全部通过（含断行一致性测试；插入 caret/命中/选区几何测试无需改期望值） |
| gates | fmt、clippy `-D warnings`、`check-rust-size.py` |
| 窗口检查 | native 33 步、a11y 9/9、source-view 5/5、default-launch 均通过 |
| 实机对照 | 上：Visual 编辑面；下：Ctrl+2 A4 预览；[对照图](evidence/SPK-0063/03-visual-above-preview.png)，单图：[Visual](evidence/SPK-0063/01-visual-editor.png)、[预览](evidence/SPK-0063/02-a4-preview.png)，[summary.json](evidence/SPK-0063/summary.json)；两处断行、缩进与标题层级一致 |

## 未覆盖

不分页：编辑面是连续长纸面，没有页边界、页眉页脚与页码；分页/虚拟化渲染留待后续。标题编号待文档模板。
夹具使用系统安装的字体族，缺字体时两条路径同样回退到 typst-kit 内置字体，一致性仍成立但外观不同。
阶段判定不升级。
