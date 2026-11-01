# 空段落 caret 与输出一致性（T29）

## Part A：空段落首行缩进

- 先增加几何回归：空段落 caret 与同位置、同样式非空段落的首字 x 比较，覆盖首段、后续段落及一二级标题。
  修改前段落测试失败（caret 0 pt，首字 24 pt），标题测试通过。
- 在 fork 的空行范围内寻找文本 origin，并将 caret 放到前置行项目的累计 advance 之后；删除 adapter 的取消缩进分支。
  `0003` 补丁同步更新，13 库相对上游新增从 592 变为 593 行、删除 16 行（最终净增 577 行），未超过预算。
- `cargo test --locked -p scholium-typst --features editor empty_paragraph -- --nocapture`：2/2 通过。
- `python3 scripts/verify-typst-fork.py /tmp/typst-origin`：按固定上游对象与四份补丁重建，全部文件一致。
- 工作流的 fork/stock 两条 `cargo run --locked …` 及 `compare.py`：原有 full/scripts/styles/heading
  四组尺寸及全部 RGBA 字节一致；没有调整任何阈值。

## Part B：显式保留空段落

- 先增加生成源码与文本位置回归：修改前 View 只有换行符，Preview 的开头空段落让后续首行少了
  22.257421875 pt 的垂直 advance，两个测试均失败。
- 用相同字体的双文本段落夹住一个空段落进行测量：编辑面的第二段基线为 124.372265625 pt；
  无显式内容时为 102.11484375 pt；`#par[]` / 零宽 box 为 116.51484375 pt；
  `#par[#text("")]` 与单独空 text 均为 124.372265625 pt。选择显式段落形式，便于源码阅读。
- View 与 Preview 都输出 `#par[#text("")]`，保留 `SourceBlock` 的完整非空范围、前后 metadata 锚点。
  三个连续空段落的锚点各自递增，预览点击仍逐段定位。
- 独立 stock 参照直接编译普通 Typst 源码，不依赖 adapter，新增开头、中间一行、中间三行、结尾、
  标题旁、两标题之间与全空文档七组夹具。自动页高同时验证末尾空行的高度；加上原有四组，共 11 组
  尺寸及全部 RGBA 字节一致。保留原有严格比较，未改变阈值。
- `typst-fork.yml` 路径过滤补齐独立参照源码及夹具文件，确保单独修改这些文件也触发 stock 对照；
  没有添加逐项窗口检查工作流。

## 最终验证与交付边界

- [完整门禁命令与结果](evidence/empty-paragraph-parity/gates.txt)：全部通过；workspace 456 passed、4 个既有 ignored、0 failed。
- [窗口检查摘要](evidence/empty-paragraph-parity/window-summary.txt)：`zh-Hans` / `en` 各 16 项，32/32 通过。
  等待了共享机器锁后运行，全部使用私有 Xvfb，没有绕过锁或重试；完整窗口产物在 slot 的 `target/t29-window/`。
- [修改前 caret 回归](evidence/empty-paragraph-parity/caret-before.txt)、
  [修改前源码回归与构造测量](evidence/empty-paragraph-parity/source-before.txt)、
  [fork 来源摘要](evidence/empty-paragraph-parity/fork-source.json)、
  [11 组 stock 字节对照](evidence/empty-paragraph-parity/stock-comparison.json) 保存了复现与校验结果。
- 没有更改 T27 的 session 输入路径或 T28 的 `paint.rs`、`ContentCache::inline` / `atoms`；
  `content.rs` 只删除了 `block()` 的取消缩进分支及对应 helper。没有引入依赖。
- 按要求分为 Part A / Part B 两个 signed-off 提交，PR 目标为 main；不合并、不轮询 CI。无规格偏离或剩余实现待办。


## CI 跟进：无系统字体的 stock 源码编译

- 协调者报告 PR #82 的 stock job（run 41319077997）在 `stock_empty.rs` 的零警告断言处失败。
  原始验证只覆盖了安装完整字体的本机，漏掉了缺失商业字体的 runner 环境。
- 修改前用 `SCHOLIUM_TEST_FONTS=bundled` 禁止系统字体扫描，stock 参照以退出码 101 复现相同 panic；
  [失败输出](evidence/empty-paragraph-parity/fonts-before.txt) 留档。
- 空段落源码参照改为复用现有独立 stock 参照的 `BODY_FONTS` / `HEADING_FONTS`，按实际 FontBook
  排除不存在的家族并保留偏好顺序。它与 Content 的有效 fallback 选择一致：仅打包字体时使用
  Liberation Serif 与 Noto Sans CJK SC；安装了优先字体时仍使用它们。零警告断言原样保留。
- stock 工作流保留默认字体对照，并增加仅打包字体的同一严格像素对照，将该复现场景纳入持续门禁。
- [默认字体集](evidence/empty-paragraph-parity/fonts-system-comparison.json)与
  [仅打包字体集](evidence/empty-paragraph-parity/fonts-bundled-comparison.json)均为 11/11 尺寸及 RGBA 字节完全一致。
- [本轮完整门禁](evidence/empty-paragraph-parity/fonts-gates.txt)全部通过：workspace 456 passed、4 个既有 ignored、0 failed；
  [两语言窗口检查](evidence/empty-paragraph-parity/fonts-window-summary.txt)32/32 通过。仅 stock 对照启用 bundled 环境，
  普通窗口检查保留原有字体环境；没有改动零警告断言、像素阈值或应用字体策略。
- 本轮作为原 PR #82 的独立 signed-off 跟进提交推送，保持 PR 未合并，不轮询 CI。
