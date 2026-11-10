# ADR 0060：定界符键入转换与独立闭侧

- 状态：Accepted
- 日期：2026-11-10
- 影响模块：model、math-input、document、storage、scholium-sdg、session、typst、app
- 修订：[ADR 0046](ADR-0046-math-root-delimiter.md)；沿用 [ADR 0045](ADR-0045-math-edge-caret.md) 的结构停点。

## 背景

匹配配对无法表达半开区间；范数、上下取整也需要直接键入入口。
Liii STEM 的连续输入与闭侧 Backspace 行为是交互基线，规则必须属于共享数学引擎。

## 决定

1. `Delimited` 保留 open/close 字符字段。open 的八种字符不变；close 独立取八种支持闭符，
   或 `INVISIBLE_DELIMITER = U+0000`。JSON 编码为 `\u0000`，它是模型属性，不是正文字符；开侧不可隐形。
   model 验证、structured storage、SDG `Shape::Delimited`、Action/snapshot 重放共用该含义。
2. schema 与 envelope 维持 v1。新构建读取原匹配配对；旧构建的配对验证拒绝新值，不能静默降级。
   不使用缺字段/default 猜测隐形，也不引入第二份渲染状态。正式项目 SQLite schema 不变。
3. 引擎记录刚键入 `|` 的短暂窗口，并只读取无身份的 body 上下文。
   空绝对值内的紧接 `|`、`.`、`'` 分别转换为范数、下取整、上取整。
   任何其它字符、导航、其它动作或非空 body 结束窗口。转换保持节点/body/caret 身份；
   与创建、后续键入分别成独立撤销组，一次 Ctrl+Z 恢复空绝对值与 body 光标。
4. 外侧右边缘 Backspace 将可见 close 替换为隐形，并进入 body 末尾，独立撤销。
   close 已隐形时遵守仅空结构删除与逐停点进入；槽尾键入支持闭符恢复该侧并进入外侧右边缘，独立撤销。
   中部输入不恢复 close。循环命令按开侧寻找下一种配对并恢复可见匹配闭侧。
5. Content 使用 `SymbolElem` 的真实数学类，Lr 才能伸缩两侧；隐形 close 没有内容。
   源码用固定符号名绕开解析器匹配：`lr(bracket.l 0 , 1 paren.r)` 与
   `lr(bracket.l frac(1, 2))`。body 沿用字面转义，用户文本不成为代码。
   中英文朗读分别命名左右侧，隐形侧明确读出。

## 验证与后果

engine/session 测试覆盖转换窗口、分组、闭侧隐藏/恢复、导航、结构删除与 undo/redo；
model 枚举所有 8×9 种合法两侧并拒绝未知值；storage/SDG 验证保存、快照与 Action 重放。
两字体 profile 的独立 stock v0.15.1 源码编译和 fork 对照均要求完整尺寸与 RGBA 字节相等。
打包字体 stock hash 同时作为双语言真实窗口上传 raster 的固定参照，不比较 caret 等 UI 叠层。
开侧隐形、一般 Typst 数学源码解析与 LaTeX 页面适配不属于本决定。
