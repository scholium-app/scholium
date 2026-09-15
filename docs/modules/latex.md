# scholium-latex

## 职责

提供容错、无损 LaTeX token/CST，项目 include graph，符号与引用索引，常用论文子集到 SDG/IR 的投影，
SDG 稳定生成、生成源码 reconcile、语义编辑降级为最小源码 patch，以及 TeX 日志位置解析。

## 非职责

不实现完整 TeX 展开或排版，不执行宏，不直接运行 TeX，不下载 package，不擅自规范化用户源码。

## 内部结构

```text
src/
├── lexer.rs        控制序列、字符、注释、空白、数学切换
├── parser.rs       group、argument、environment、error recovery
├── cst.rs          lossless nodes/token ranges
├── verbatim.rs     声明式 verbatim 环境识别
├── project.rs      input/include/bibliography/resource graph
├── symbols.rs      label/ref/cite/command/environment 定义与使用
├── semantics.rs    常见命令/环境 capability
├── projection.rs   CST → IR + coverage
├── generate.rs     SDG/IR → LaTeX + package requirements
├── reconcile.rs    generated CST diff → SemanticPatch
├── edit.rs         rename/insert/wrap/math patch
└── log.rs          TeX log → Diagnostic
```

## 解析策略

lexer 不展开 catcode；首发假定常见 LaTeX catcode 语义。遇到可能改变词法规则的命令，标记后续区域为
`lexically_uncertain`，仍保留 token。parser 使用同步点 `}`、环境结束、段落和行边界恢复错误。

未知命令节点保存命令 token 与词法可辨认参数，不推测可选参数数量。`\newcommand` 只索引名称、参数数
和定义范围，默认不展开；白名单纯宏可在独立受限投影器中展开，且保留 provenance 链。

## 语义编辑

- rename label：从 workspace symbol index 获取定义与引用，生成跨文件 patch。
- insert citation：依据当前命令风格生成 `\cite{}` 或配置模板。
- wrap selection：验证组/环境边界后包裹，不跨 verbatim 或不平衡 error node。
- replace math：只在确认的数学范围内替换；保留周围 delimiter 风格。
- add resource：选择相对路径并按项目风格生成引用，不复制文件。

每个 patch 保存 precondition hash；CST 过期即拒绝。

## 项目图

识别 input/include、bibliography/addbibresource、常见图片命令和 documentclass/usepackage。解析路径时保留原
字符串与解析结果。循环边形成诊断但不递归。动态拼接路径标记 unresolved，不能假装缺失或存在。

## 不变量

- parse 后所有 token 拼接等于输入字节。
- 未修改节点输出直接使用原 source slice。
- error/unknown/uncertain 区域不能执行破坏性语义重构。
- 日志映射失败时保留原日志位置，不猜测到错误文件。
- 构建能力和解析能力分离：能构建不代表能理解。

## 测试门禁

真实论文夹具、嵌套组、注释吞换行、escaped percent、数学 delimiter、verbatim、自定义环境、损坏输入、
catcode 警告、include 循环、动态路径、label rename 和 log 映射。fuzz 要求不 panic 且 token 全覆盖。

## 团队源码语言与混合项目合约

新增 ForeignSource 的 LaTeX 组件入口、宏/模板作用域、依赖闭包与符号导入/导出描述；编译由 build 执行。
LaTeX 主入口引用 Typst 组件需显式绑定与生成资源，不能直接拼入 Typst 语法。原文持久保存，动态路径 unresolved。
支持的跨引擎宏/参数桥接输出声明式合约，不运行任意宏推断参数。模板全局副作用只能在声明宿主/章节范围生效。
新增混合正文、跨页表格、自定义宏、模板、双向引用及标准工具链包的真实夹具，未支持组合给明确诊断。

共同要求见 [混合源码与团队编辑](../MIXED_SOURCE_EDITING.md) 与 [ADR 0001](../adr/0001-mixed-source-team-editing.md)。

## 候选与参考实现

Rust 容错解析器配 Rowan CST 待验证；Tree-sitter 仅作可选高亮/导航，不承担完整宏语义。纯解析/投影优先兼容 WASM。参考[Mogan 源码调研](../research/MOGAN_LATEX.md)拆分上下文、模板适配、宏包规划、序列化和 source provenance；独立实现，不复制其源码。不确定区域仍遵守本模块保留原文规则。
