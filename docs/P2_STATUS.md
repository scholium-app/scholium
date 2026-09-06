# P2 执行情况 — 数学核心

更新日期：2026-09-06。

## 批次进度

| 批次 | 内容 | 状态 |
|---|---|---|
| A | Math AST(节点种类、固定槽位、wrap 语义、Typst 序列化、真实编译验证) | ✅ 本分支 |
| B | Frame 反查：数学字形 span → 槽位级命中测试、caret 进出结构 | ⏳ |
| C | 数学环境(行内/单行)接入 shell、多行环境 | ⏳ |
| D | 结构内导航(进分母/跳出根号)、结构化选区 | ⏳ |
| E | 按键触发结构(`^`/`_`/`/` 建 script/frac)、快照测试 | ⏳ |

## 批次 A 记录

- `NodeKind` 新增 9 个数学种类;`scholium-doc/src/math.rs` 定义 arity 契约与
  `slot` 常量(`NUM/DEN/BASE/SUB/SUP/RADICAND/DEGREE`)。
- **固定槽位模型**:可选槽(如 `x^2` 的下标)是空 `MathRow` 而非缺失子节点,
  槽位索引稳定;序列化跳过空槽。
- `WrapNode` 对数学包装有专用语义:原节点进 0 号槽(非 row 则包一层新 row),
  其余槽补空行;`MathRoot` 只建 radicand 槽,度槽由后续编辑动作追加。
  `EditOp` 枚举未改动。
- `InsertText` 在 `MathRow` 光标处插入 `MathSymbol` 子节点;
  在 `MathSymbol` 内追加文本。
- 序列化:`frac(a, b)` / `base_(sub)^(sup)` / `sqrt(x)` / `root(n, x)` /
  `lr(( x ))` / `hat(x)` / `$ x $`(display)。
  符号名按 Typst 约定:标识符裸输出(`alpha`),其余加引号(`"中"`)。

## 实测踩坑(与 P0 同类:文档/记忆不可靠)

- **typst 0.15 的 `lr()` 没有 `left`/`right` 命名参数**——定界符是 body 的
  一部分(`lr(( x ))`)。按老规矩查 registry 源码确认,真编译测试把关。
- `{`/`}` 在数学模式是分组符,定界符属性须映射为 `brace.l`/`brace.r`。
- `root()` 参数序是 `(degree, radicand)`,与本 AST 槽位序相反,序列化时换序。
