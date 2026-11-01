# 空段落 caret 与输出一致性（T29）

## Part A：空段落首行缩进

- 先增加几何回归：空段落 caret 与同位置、同样式非空段落的首字 x 比较，覆盖首段、后续段落及一二级标题。
  修改前段落测试失败（caret 0 pt，首字 24 pt），标题测试通过。
- 在 fork 的空行范围内寻找文本 origin，并将 caret 放到前置行项目的累计 advance 之后；删除 adapter 的取消缩进分支。
  `0003` 补丁同步更新，13 库相对上游净新增从 592 变为 593 行，未超过预算。
- `cargo test --locked -p scholium-typst --features editor empty_paragraph -- --nocapture`：2/2 通过。
- `python3 scripts/verify-typst-fork.py /tmp/typst-origin`：按固定上游对象与四份补丁重建，全部文件一致。
- 工作流的 fork/stock 两条 `cargo run --locked …` 及 `compare.py`：原有 full/scripts/styles/heading
  四组尺寸及全部 RGBA 字节一致；没有调整任何阈值。

Part B 及最终门禁记录随第二个提交补充。
