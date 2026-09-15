# scholium-markdown

## 职责

实现指定方言的 lossless Markdown CST、增量解析、符号/链接/引用/资源索引、IR 投影、语义 patch 和
安全 HTML 生成输入。

## 非职责

不自动猜测并切换方言，不在普通保存时格式化，不执行代码块，不允许 raw HTML 绕过预览消毒。

## 内部结构

```text
src/
├── dialect.rs      CommonMark 基线与扩展开关
├── lexer.rs        block/inline token 与 trivia
├── parser.rs       容错 CST、增量失效边界
├── cst.rs          原始标记和范围
├── symbols.rs      heading、link definition、footnote、citation
├── projection.rs   CST → IR + raw coverage
├── edit.rs         heading/list/link/table/math 等 patch
└── html.rs         IR/受限 raw HTML 输出描述
```

## 方言

`project.toml` 固定基线与扩展。打开未知项目时探测只产生建议，不自动改配置。方言变化是显式项目动作，
修改后重新解析全项目并展示新增诊断。

## 无损要求

保留 emphasis 标记选择、列表 marker、编号、缩进、围栏字符与长度、info string、换行、链接定义位置和
raw HTML。语义编辑只重写最小 block/inline 祖先；无法稳定重写时退化为纯文本编辑命令。

## 语义与预览

heading 生成 slug 仅用于索引，保存不插入隐式 ID。raw HTML 在 IR 中保留，但预览必须经 sanitizer；
脚本、事件属性和危险 URL 不进入预览。代码块只高亮，不执行。

## 不变量

- parse token 全覆盖输入。
- 方言是 ParseInput 的一部分，缓存 key 必须包含方言 hash。
- HTML 输出默认不信任 raw HTML。
- reference/citation rename 使用 workspace patch，并验证目标未发生变化。
- 格式化命令与普通保存严格分离。

## 测试门禁

CommonMark 兼容夹具、启用扩展夹具、Unicode heading、嵌套列表、围栏、脚注、数学、raw HTML XSS、
损坏表格、增量编辑与完整重解析一致性、未触及范围字节不变。
