# ADR 0046：数学根式与自动定界符节点

- 状态：Accepted
- 日期：2026-11-02
- 影响模块：model、document、storage、typst、session、app
- 接续：[ADR 0033](ADR-0033-structured-local-session-migration.md)、[ADR 0039](ADR-0039-math-attach.md)、[ADR 0045](ADR-0045-math-edge-caret.md)

## 背景

根式和可随内容伸缩的括号需要可编辑槽位，不能从用户键入的字面字符猜出结构，也不能把公式原串当成受信程序执行。
分数和上下标已有稳定节点、结构边缘、光标几何与本地历史；新结构需要相同的编辑和兼容语义。

## 决定

1. 在 structured v1 内加性新增 `root` 和 `delimited` tag。Root 包含可选 `index` 和必填 `radicand`；
   缺席 index 不序列化，空槽为 Hole。Delimited 包含必填 body，限定 `()`、`[]`、`{}`、`||`、
   `‖‖`、`⟨⟩`、`⌊⌋`、`⌈⌉`，其他配对拒绝。节点 ID、深度、容量及完整性验证与现有数学树一致。
2. 沿用 ADR 0039 的版本策略：新构建读取旧文档；旧构建遇到未知 tag 拒绝含新节点的文档，
   不静默降级或更改 schema 版本。legacy 公式迁移仍保留 RawMath，不推断节点。
3. 插入在当前字素或复合节点边缘拆分同一数学行；选区仅包装同一行的完整范围。根式默认无 index，
   定界符默认为 `()`；光标进入内容槽。焦点栏可加入/进入 index、循环定界符配对，每次变更可撤销。
   字面 `(` 仍是文字。ADR 0045 的结构边缘、槽遍历和仅空结构移除规则继续适用；Root 子节点顺序为
   index（存在时）再 radicand，Delimited 为 body。
4. 编辑面直接构造 Typst `RootElem` 和 `LrElem`。Root 的 index 为编辑槽 1、radicand 为槽 2；
   Delimited 的 body 为槽 1。fork 补丁保留 `lr` 展平后内部组件的槽 origin，同时保留节点 origin。
   光标、命中与指示框来自同一个 Frame；不添加打印锚点。生成视图使用 `sqrt(x)`、`root(n, x)`、
   `lr(( x ))` 等形式，数学文字按既有字面规则转义，空槽输出 `□`。

## 验证方法

模型与存储测试验证新 tag、非法配对、v1 round-trip；会话审计核对嵌套结构的停点、导航、删除和撤销。
Content 测试核对槽 origin 与 caret；生成源码测试实际编译各定界符和带逗号的参数。
在两种字体 profile 下将独立 stock Typst 和 fork 的根式及八种定界符像素逐字节对照；共享场景和
双语言 Xvfb 窗口检查命令、焦点操作及可见几何。具体验证记录见[开发日志](../log/2026-11-02-math-root-delimiter.md)。

## 候选与后果

- 把根式或括号保存为 RawMath 会丢失槽编辑与导航；根据字符自动配对又会改变普通文字输入，均不采用。
- 升 schema 版本可让旧程序更早拒绝，但当前 v1 本地候选已采用加性 tag 策略；将来需要版本协商时可替换。
- 旧构建不能打开含新节点的文档。当前结构选区仍限同一数学行；复杂源码解析和跨槽选区不由本决定承诺。
