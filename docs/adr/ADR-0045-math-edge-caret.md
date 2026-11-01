# ADR 0045：数学复合结构的边缘光标

- 状态：Accepted
- 日期：2026-11-01
- 影响模块：document、typst、session、app
- 接续：[ADR 0031](ADR-0031-structural-cursor.md)、[ADR 0042](ADR-0042-session-editing-core.md)

## 问题

Text/Hole 叶内字素位置不能表达「公式内、分数外」。公式根直接是 Fraction 时，叶遍历会从槽位
跳到公式外；正文中的 Backspace 也无法逐层进入公式。该缺口同样影响 Attach 以及嵌套结构。

Mogan/TeXmacs 的树路径光标和结构删除提供设计结论：复合结构前后有停点；删除键遇到有槽位的结构
先进入参数，只有空结构才整体移除。参考只用于判断交互语义，不复用其源码。

## 方案与决定

采用**会话边缘位置**。`Position.leaf` 在边缘持有 Fraction/Attach 的稳定 NodeId，`byte=0/1`
分别表示前/后；Text/Hole 上的 byte 继续是叶内 UTF-8 字素边界。节点类型消除两者歧义，affinity
仍表示视觉行边界归属。当前模型尚无 Root/Delimiter；后续新增时沿同一规则扩展。

拒绝在模型里常驻空 Text/Hole 锚点：它们会污染保存、源码和撤销，需要维护额外生命周期，且空 Text
在 Typst 中没有字形几何。此次不修改文档 schema 或存储格式。既有显式 LeaveSlot 的临时 Hole API
保持兼容；普通方向键使用边缘位置，不创建占位节点。

## 顺序与编辑

逻辑顺序为：结构前 → 按 `math_children` 顺序遍历各槽 → 结构后。Fraction 为分子、分母；Attach
为 base、sub、sup。嵌套结构递归遵循同一规则。父 Row 中相邻 Text/Hole 已提供同侧停点时，省略结构
边缘停点；正文叶与公式内边缘是不同语义位置。Tab 继续按叶/槽循环，Shift 选区保持正文规则。
边缘 Home/End 到所在槽 Row（没有槽则整公式）的首末规范停点，避免落到被文字替代而不存在的另一侧边缘。

Backspace/Delete 方向对称：先删除同叶相邻字素；空 optional sub/sup 仍优先走 RemoveSlot；在槽位
对应边界遇到全空结构时移除结构；空公式移除 inline。其余情况只移动一个逻辑停点，无文档动作、
无撤销条目，不重置输入分组。块边界仍按已有合并规则处理，包括导入文档直接以公式开始/结束的块。
必填槽删除结构后留下新 Hole；从 Row 摘除结构保留相邻节点身份。

在边缘输入、IME Commit 或粘贴创建真正的数学 Text 兄弟；根/必填槽没有 Row 时包一层新 Row。
Ctrl+/ 或上下标命令在边缘创建空分数或空 base 的 Attach，光标进入新目标槽。删除这些兄弟 Text 的
最后一个字素会移除已空叶，并把光标恢复为结构边缘；单子 Row 随之折叠。每次都是同一权威的原子编辑。
Undo/redo 保存并恢复完整 Position，因此边缘不需要另一个可变状态或持久化字段。

## 几何与辅助技术

边缘 caret 从同一 Typst Scene 已有节点 Frame bounds 的并集生成：x 取结构左右界，y 使用整公式
范围。Attach/Row 没有独立 bounds 时只合并其后代的真实 bounds。边缘不生成排版 Content，不改变
栅格、源码或最终输出。worker 和键盘共用停点规则，导航内存按节点数增长，仅扫描当前叶字素。

公式指示底色覆盖边缘位置；嵌套边缘可以保留外层槽的指示框。公式内点击距边缘不超过 0.5 pt 时
优先命中边缘，公式外沿用正文视觉行规则。所有命中和几何仍要求完整当前 SceneStamp。

AccessKit 的独立 polite 状态节点播报「公式内，分数前/后」「公式内，上下标前/后」，嵌套时保留
外层槽路径。正文 Text 的内容和偏移不包含播报文字或结构边缘。

## 验证与边界

会话回归覆盖双向停点序列、嵌套、去重、删除、输入及撤销；同一冻结场景同时驱动 headless 与真实
Xvfb 窗口，并以 AUDIT 逐步核对。Typst parity 测试验证结构边缘几何及编辑面/预览行内容一致；
独立 stock 对照继续检验像素。数学内部范围选区、协作光标和新数学节点不在本决策中实现。
