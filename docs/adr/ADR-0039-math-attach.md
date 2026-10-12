# ADR 0039：数学上标/下标节点与逐字素数学文字

## 状态

接受。在 [ADR 0033](ADR-0033-structured-local-session-migration.md) 的 structured v1 候选信封内加性新增
`MathBody::Attach`，并改变数学 Text 的编辑面渲染与生成源码的输出方式。

## 背景

数学里输入 `x^2 + y` 只会把字面串 `"x^2 + y"` 存进一个 `MathBody::Text`：编辑面把它作为一个多字符
Typst 文字 run 排版，`^` 是字面脱字符、字母直立、运算符没有数学间距；生成源码是 `$"x^2 + y"$`。
[体验原则](../EXPERIENCE.md)要求上下标是显式节点而不是字符串猜测。

## 决定

1. **模型**：`MathBody::Attach { base, sub?, sup? }`，serde tag `"attach"`，至少一个槽存在；缺席的槽不序列化。
   深度、节点、身份规则与 Fraction 相同；`ensure_filled` 递归检查所有存在的槽。
2. **版本**：不升 schema 版本。变更是信封内的加性 tag；新构建读旧文档无影响，旧构建遇到含 `attach` 的
   文档会因未知 tag 拒绝（`deny_unknown_fields` / 枚举 tag），不会按错误语义解释。
3. **编辑**：三个结构动作，各为一次可撤销的动作，光标由核心返回，应用不猜测：
   `Attach { leaf, at, slot }`（Text 内取前一个字素为 base 并拆成 左/base/右 并入父 Row；`at == 0` 或 Hole 时
   base 为新 Hole；光标紧随已有 Attach 时补缺的槽或仅移到槽尾）、`LeaveSlot { leaf, at }`（槽尾离开到
   Attach 之后，其后无叶时才追加占位 Hole）、`RemoveSlot { leaf }`（空槽退格删槽，无槽的 Attach 退化为
   base 并合并相邻 Text，也可删 Attach 后的占位 Hole）。Row 永不直接嵌套 Row，单子节点的 Row 折叠。
4. **输入**：数学内键入 `^`/`_` 触发 Attach，不插入字面字符；粘贴保持字面。导航：槽尾右键离开到 Attach
   之后，槽首左键到 base 末尾，Tab 沿叶序 base→sub→sup→出，上下键在 base/sub/sup 间切换（同分子/分母）。
   朗读：`上下标（底数：…；下标：…；上标：…）`，槽路径名 底数/下标/上标。
5. **渲染**：`AttachElem`（t=sup、b=sub），槽以编辑槽 origin 包装（base 1、sub 2、sup 3）。数学 Text 逐字素
   渲染为原子：`ClassElem`（Typst 默认数学类）内一个带原子 origin 的 `TextElem`，ASCII 字母显示为数学斜体码位
   （Typst 自动斜体只作用于符号元素，而符号元素没有簇几何）。原子 origin 编码 叶/字节范围，几何把原子两端
   caret 映回叶内字节，每个字素边界都有精确 caret。
6. **生成源码**：Attach 写为 `base^(sup)_(sub)`，多原子 base 加括号；数学文字逐原子输出：ASCII 字母与数字串
   裸写并以空格分隔，白名单运算符（`+-=<>!,;:.?`）裸写，其余为转义引号串。用户字符不会成为 `#`、`$`、
   `\`、未转义 `"`、`/`、`^`、`_`、`&`、`@` 等语法。

## 后果

- 旧构建不能打开含 `attach` 的文档（明确失败而非静默降级）；这是接受的代价，直到引入版本协商。
- 编辑面与预览的数学文字现在按原子排版：`sin` 等多字母词暂显示为独立斜体字母（已知限制，需要后续的
  运算符/标识符节点），`x` 与 `y` 之间输入的空格在预览里只是分隔符。
- 光标位置是 `(叶, 字节)`；LeaveSlot 在 Attach 之后无叶时追加占位 Hole，它只在光标位于其中时存在，
  光标离开且无其他编辑时应用端回到 LeaveSlot 之前的快照并丢弃该撤销项；保存时恰在其中则可能持久化。
- 编辑面几何多出每字素一个 cluster；叶内 cluster 数量随文字线性增长，与既有 Text 路径同阶。

## 备选

- **升 schema 版本**：旧构建会以版本错误拒绝，但需要迁移器与版本协商，且当前没有发布过 v1 的第三方构建，
  收益有限；放弃。
- **把含上下标的公式存为 RawMath**：保留源码但失去结构编辑、槽导航与读屏，违背体验原则；放弃。
- **用 Typst `SymbolElem` 逐字素渲染**：自动斜体与类间距最原生，但符号 glyph 路径不产生簇几何，无法
  给每个字素边界精确 caret；放弃。
