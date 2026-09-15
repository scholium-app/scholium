# Liii STEM 体验调研

调研时间：2026-09-14。入口为[给 LaTeX 用户的指南](https://liiistem.cn/docs/latex-user-guide)，并继续阅读
快速入门、第一篇文档、文本/数学模式、高效编辑、图片表格、文献、引用和样式等关联页面。

本调研只提炼公开文档中的交互思想，不复制其实现、商标、图标、文案或私有格式。

## 1. 值得继承的体验模型

### 模式与环境分离

[第一篇文档](https://liiistem.cn/docs/base-editing/guide-first-document)将 Mode 定义为当前输入类型，Environment
定义为嵌套结构，光标所在最内层环境是焦点。模式工具栏随输入类型变化，焦点工具栏随结构变化。

Scholium 对应：文本、数学、源码三模式；TreeCursor 决定焦点；工具栏由 capability 生成。

### 同一操作有多条入口

公开指南强调菜单、快捷键和 `\` 命令可以完成同一操作。LaTeX 用户可沿用熟悉命令，新手可从菜单发现，
熟练用户使用键盘。

Scholium 对应：所有入口调用同一个 command registry 和 SemanticEdit，不分别实现业务逻辑。

### 高速数学输入

[数学快速入门](https://liiistem.cn/docs/tutorial/tuto-math-edit)和
[数学模式指南](https://liiistem.cn/docs/guide-equation)展示乐高符号、Tab 符号循环、LaTeX 命令输入、
行内/单行/多行公式和自动对齐。[高效编辑指南](https://liiistem.cn/docs/guide-tab)继续定义结构变体、结构
光标、矩阵/列表增删、解除最内层环境以及带空槽通配的结构搜索。

Scholium 对应：声明式 symbol composition、cycle registry、slot navigation、variant command 和 pattern tree。

### 焦点工具栏是属性编辑器

[LaTeX 用户指南](https://liiistem.cn/docs/latex-user-guide)展示通过焦点工具栏控制公式编号、环境样式、图片尺寸
和当前环境取消，而不是要求用户修改底层命令。[图片表格指南](https://liiistem.cn/docs/base-editing/guide-table)
还将行列增删、单元格选区、属性和继承绑定到当前焦点。

Scholium 对应：FocusContext 返回当前节点属性 schema、selection scope 和 commands，前端不写死节点判断。

### 引用是可导航对象

[参考文献指南](https://liiistem.cn/docs/guide-cite)从 `.bib` 导入并以 citation key 插入引用；
[双向链接指南](https://liiistem.cn/docs/guide-lines)将 label/ref 做成可标识、补全和跳转的对象。

Scholium 对应：workspace symbol graph、引用悬停、跳转、跨文件 key rename 和 BibTeX/CSL source studio。

### 连续写作与最终页面检查分离

[第一篇文档](https://liiistem.cn/docs/base-editing/guide-first-document)把连续滚动作为默认，并提供单页、双页和
全景布局。[排版样式指南](https://liiistem.cn/docs/guide-styling)把语言、断行和局部排版属性暴露为文档操作。

Scholium 对应：视觉编辑默认连续流，最终 Typst/LaTeX 预览提供分页视图；语义样式与查看布局分开。

### 粘贴是格式入口

[魔法粘贴指南](https://liiistem.cn/docs/tutorial/tuto-magic-paste)区分网页普通粘贴、Markdown/LaTeX 源码粘贴、
图片/OCR 和选择性粘贴。

Scholium 首发先实现确定性剪贴板路由：HTML/MathML、LaTeX、Markdown、纯文本和图片；粘贴前显示识别格式，
选择性粘贴允许覆盖。OCR/大模型识别后置，不能承诺“无损”百分比。

## 2. Scholium 必须不同的地方

- Liii STEM 公开文档以自身结构格式和源码模式为中心；Scholium 还要让 LaTeX 与 Typst 成为真正可写的
  Source Studio，并明确谁是权威模型。
- Scholium 的 Typst 快速预览不能冒充 LaTeX 最终排版，两个后端的状态和 revision 必须可见。
- LaTeX/Typst 双向转换必须逐节点报告 Exact/Equivalent/Raw/Degraded/Dropped。
- 所有视觉、源码和转换动作进入同一非线性历史与协作系统。
- 未知源码必须保留为 Raw 节点或留在 SourceAuthority 项目，不能为了 WYSIWYG 丢弃。

## 3. 首发体验优先级

1. 焦点、文本/数学模式和连续流结构编辑。
2. `\` 命令、槽位、Tab 循环、乐高符号与结构变体。
3. 公式、矩阵、表格、图、定理、label/ref/cite 的结构操作。
4. Typst 快速预览与源码 lens。
5. 完整 LaTeX/Typst Source Studio 和 reconcile。
6. 结构搜索、双后端比较、剪贴板格式路由。

模板中心、OCR、AI 自动排版、脚本会话、绘图和幻灯片不因竞品具备就自动进入首发。
