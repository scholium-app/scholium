# 数学上标/下标节点与逐字素数学文字

> 本文是第 1 阶段实现的追溯记录（过去时），不是阶段 0 验证证据，也不作为实现依据；当前规范见 `docs/modules/` 与 `docs/adr/`。

- 日期：2026-10-12。
- 对应：[ADR 0039](../adr/ADR-0039-math-attach.md)、[体验原则](../EXPERIENCE.md)“上下标是显式节点，不靠字符串猜当前槽位”。
- 基线：main（合并 A4 编辑面后）；分支 `ation_ciger/feat/math-attach`。

## 问题

数学内输入 `x^2 + y` 只存一个字面串 `"x^2 + y"`：编辑面把它排成一个多字符 Typst 文字 run（`^` 是字面脱字符、
字母直立、运算符没有数学间距），生成源码是 `$"x^2 + y"$`。

## 改动

- 模型：`MathBody::Attach { base, sub?, sup? }`（tag `attach`，至少一个槽，缺席槽不序列化），v1 信封内加性变更；
  校验、`ensure_filled`、深度/节点/身份规则同 Fraction。
- 文档：`Attach`/`LeaveSlot`/`RemoveSlot` 三个结构动作（独立模块 `structured_session/attach.rs`），每个是一次可撤销
  动作，返回数学叶光标；`base_before_slot` 给槽首左移目标。
- 编辑面：`AttachElem`（t=sup、b=sub，槽 1/2/3）；数学 Text 逐字素渲染为 `ClassElem` 内带原子 origin 的
  `TextElem`，ASCII 字母显示为数学斜体码位；几何把原子 origin（叶 << 64 | 起 << 32 | 止）映回叶内字节，
  每个字素边界都有精确 caret。
- 生成源码：`base^(sup)_(sub)`；数学文字逐原子：ASCII 字母/数字串/白名单运算符裸写并以空格分隔，其余转义引号串。
- 应用：数学内键入 `^`/`_`；槽尾右键离开、槽首左键回 base、Tab 沿 base→sub→sup→出、空槽退格删槽、上下键
  在 base/sub/sup 间切换；朗读 `上下标（底数：…；下标：…；上标：…）`。
- 独立参照：stock-Typst 对照新增 `scripts` 夹具（`x^2 + y_i = a_n^23`）。

## 复现

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 scripts/check-rust-size.py
cargo build --locked -p scholium-app -p scholium-storage --bins
cargo run --locked -p scholium-typst --features editor --example typst-main-fork-check -- /tmp/typst-fork-pixels
cargo run --locked --manifest-path spikes/typst-main-editor/reference/Cargo.toml --target-dir target/stock-reference -- /tmp/typst-stock-pixels
python3 spikes/typst-main-editor/compare.py /tmp/typst-fork-pixels /tmp/typst-stock-pixels
xvfb-run -a -s '-screen 0 1280x900x24' python3 spikes/typst-main-editor/math-attach-check.py target/debug/scholium-app /tmp/scholium-attach
```

窗口检查另有 native / a11y / source-view / default-launch 四项（命令同 CI `typst-editor-window` 作业）。

## 结果

| 检查 | 结果与证据 |
|---|---|
| 回归 | **217 passed / 0 failed**；[tests.log](evidence/math-attach/tests.log)。新增：模型 4（往返、缺槽拒绝、身份/未填、深度）、文档 17（拆分、起始/Hole、补第二槽、重复槽、占位 Hole、删槽拍平并合并 Text、撤销、边界）、源码 4、编辑面几何 6（每个字素边界有 caret、52 个字母均为斜体原子、`x+y` 原子与二元间距、槽位置、空槽/Hole 基、Row 内叶）、应用 16（含占位 Hole 生命周期：点击/Left/Ctrl+Home/End 离开恢复原快照且无多余撤销步、一次撤销撤销用户上一步、空占位 Right 退出公式、在占位中输入则保留）+ 朗读 2 |
| 输入语义 | `$x^2` → 右键 → `+y` 得 Row[Attach{x, sup 2}, Text "+y"]；`x_i^2` 得单个带两槽的 Attach；正文与粘贴中的 `^`/`_` 保持字面 |
| 生成源码 | `#$\"/^_&@` 等逐个出现在预览编译输出的文字里、不在引号串之外出现；`x^(2)_(i)`、`(a b)^(2)`、`(x^(2))^(3)` 编译通过 |
| gates | fmt、clippy（`-D warnings`）、文件规模：[fmt.log](evidence/math-attach/fmt.log)、[clippy.log](evidence/math-attach/clippy.log)、[size.log](evidence/math-attach/size.log) |
| 窗口检查 | native 33 步、a11y 9/9、source-view 5/5、default-launch 全部通过：[native](evidence/math-attach/native-check.log)、[a11y](evidence/math-attach/a11y-check.log)、[source](evidence/math-attach/source-view-check.log)、[launch](evidence/math-attach/default-launch-check.log) |
| 上下标实机 | `math-attach-check.py`：键入 `$x^2` Right ` + y_i` 得 `[{base x, sup 2}, " + ", {base y, sub i}]`，模型中没有字面 `^`/`_`，Ctrl+2 预览 1 页无错误：[summary.json](evidence/math-attach/attach-check/summary.json) |
| 截图 | Visual 模式 [x² + yᵢ](evidence/math-attach/attach-check/03-visual-x2-plus-yi.png)；Ctrl+2 源码 `$x^(2) + y_(i)$` 与预览 [x² + yᵢ](evidence/math-attach/attach-check/04-preview-x2-plus-yi.png) |
| stock 像素对照 | 4 个夹具逐字节相同（`full`、`scripts`、`styles`、`heading`）：[stock-comparison.json](evidence/math-attach/stock-comparison.json)；上游 fork 重建通过：[typst-fork-source.json](evidence/math-attach/typst-fork-source.json) |

## 发现

- 逐字素 `ClassElem + TextElem` 与 stock 数字、`full` 夹具（`12`、`3`、`4`）逐像素相同，既有对照不变。
- 与 Typst 原生 `SymbolElem` 构造对比，唯一差异在 `y_i` 的下标 `i`（以及随后元素右移 1 像素列）：推测是原生符号走
  脚本字号的 `ssty` 字形，而 `TextElem` 的内联路径不应用（未单独验证）。因此 `scripts` 夹具的独立参照镜像编辑面原子构造
  （`ClassElem` + 斜体码位的 `TextElem`），数字仍用 stock 文字；预览走真实 Typst 符号路径，脚本字母可能与
  编辑面差一个像素列。未放宽任何比较阈值。
- Typst 原生不会给 `TextElem` 设自动斜体，且改变文字内容后簇几何会丢失（斜体化文字 ≠ 原文就不记录 `edit_text`），
  所以字母直接使用数学斜体码位；ASCII 以外的字母（希腊字母等）保持直立，与源码里加引号的输出一致。

## 未覆盖

- `sin` 等多字母词显示为独立斜体字母（已知限制，需要后续标识符/运算符节点）。
- 槽位选区、数学内换行、希腊字母/其他文字的自动斜体、脚本字号的 `ssty` 字形、嵌套脚本字号层级的专门验证。
- Right 离开槽后若后面没有叶会追加占位 Hole；光标离开且无其他编辑时自动移除并丢弃其撤销项；
  仅在光标恰在其中时保存才可能持久化。
- 数学内键入 `^`/`_` 时光标在槽尾会补另一槽而不是嵌套（`x^2^` 不能用键盘表达 `x^(2^…)`，需把光标放进槽中部）。
- 旧构建打开含 `attach` 的文档会明确失败。
- 编辑面仍是受控不分页流；阶段判定不升级。
