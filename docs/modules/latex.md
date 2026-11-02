# scholium-latex

## 职责

提供容错、无损 LaTeX token/CST，项目 include graph，符号与引用索引，常用论文子集到转换 IR 的投影、
IR 到 LaTeX 的导出生成、语义编辑降级为最小源码 patch、TeX 日志位置解析与实时预览适配。

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
├── generate.rs     转换 IR → LaTeX + package requirements
├── preview.rs      实时引擎适配与事件转换
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

## 公共接口

- `parse(resource, revision, text) -> LatexCst`；`reparse(previous, edits) -> LatexCst`。
- `symbols(cst) -> SymbolIndex`；`dependencies(cst) -> Vec<ResourceDependency>`（input/include、bibliography、
  图片、documentclass/usepackage）。
- `project(cst, selection) -> Projection { ir, coverage, diagnostics }`。
- `generate(ir, profile) -> GeneratedArtifact + PackageRequirements`。
- `lower_edit(cst, SemanticEdit) -> PatchResult`；每个 patch 携带 precondition hash。
- `parse_log(text) -> Vec<Diagnostic>`；预览适配层输出段落更新、页面更新、定位与诊断事件。

## 不变量

- parse 后所有 token 拼接等于输入字节。
- 未修改节点输出直接使用原 source slice。
- error/unknown/uncertain 区域不能执行破坏性语义重构。
- 日志映射失败时保留原日志位置，不猜测到错误文件。
- 构建能力和解析能力分离：能构建不代表能理解。

## 失败处理

- catcode 可能改变词法时标记 `lexically_uncertain`，并拒绝该区域的破坏性语义重构。
- 未知命令保留命令 token 与词法可辨认参数，不推测可选参数数量。
- include 循环、缺失资源、根外路径和动态拼接路径产生诊断；动态路径标记 unresolved，不假装存在或不存在。
- CST 过期或 precondition hash 不匹配时拒绝 patch，重算或进入冲突预览。
- 日志位置无法映射时保留原日志位置，不猜测到其他文件。
- fuzz 输入不得 panic，且 token 拼接仍等于输入字节。

## 测试门禁

真实论文夹具、嵌套组、注释吞换行、escaped percent、数学 delimiter、verbatim、自定义环境、损坏输入、
catcode 警告、include 循环、动态路径、label rename 和 log 映射。fuzz 要求不 panic 且 token 全覆盖。

## 项目语言合约

本模块属于 LaTeX 内核，创建/导入时固定项目语言，以 .tex 源码为权威；预览与最终输出使用同一 LaTeX 引擎。
实时预览适配层以 LuaLaTeX + rtex 为候选（待评估），对上只暴露段落更新/页面更新/定位/诊断事件；pdfLaTeX/XeLaTeX 标明整编预览。
引擎与常驻管道须经 build 的 OS 沙箱、运行时白名单及资源预算约束，未隔离不得接入。
跨语言只经转换或导出及逐节点报告；混合组件、跨引擎桥接与团队语言切换已推迟。
共同要求见 [项目语言、跨格式转换与文件操作](../MIXED_SOURCE_EDITING.md) 与 [ADR 0041](../adr/ADR-0041-separate-latex-typst-kernels.md)。

## 候选与参考实现

Rust 容错解析器配 Rowan CST 待验证；Tree-sitter 仅作可选高亮/导航，不承担完整宏语义。纯解析/投影优先兼容 WASM。参考[Mogan 源码调研](../research/MOGAN_LATEX.md)拆分上下文、模板适配、宏包规划、序列化和 source provenance；独立实现，不复制其源码。不确定区域仍遵守本模块保留原文规则。

## 上游跟进

- 实时预览候选锁定于 [经评审的 upstream main commit](../../spikes/rtex-eval/PIN)，只作为 `rtex` OS 沙箱中的独立子进程；不加入 Scholium Rust 依赖图。
  repository、branch main 与精确 commit 由该 pin 记录唯一维护，评估 checkout 和构建留在仓库外。
- 每周一 03:37 UTC，`.github/workflows/rtex-upstream.yml` 调用 `scripts/rtex-upstream-drift.py`；也可手动指定上游 ref。
  它比较 upstream main 与 pin 的精确 commit，报告 pin 之后的提交、锁定构建、未过滤的 cargo-deny 结果，以及 `rtex serve` 启动/编辑/段落更新/布局冒烟。
  只有当前 Ubuntu runner 的真实 bubblewrap namespace 探测通过时，才运行 TeX 冒烟、E8 范围/路径拒绝子集与 E2 外部文件读取对照；
  否则将 TeX 检查标为未运行。此 job 只作外部监测，不在 push/PR 触发、不阻塞 PR、不自动开 issue；表格写入 job summary，原始日志作为 artifact。
- 定时监测不替代本地 E2 全负对照、E3/E5/E6 回归、完整 E8 故障注入或真实窗口 L1 验收。上游变化须按 ADR 0043 复验后才移动 pin。
- `python3 scripts/rtex-pin.py [完整 commit SHA]` 一条命令在仓库外 clone/build、顺序跑 E1/E2/完整 E8，输出 Pass/Fail；无参数取当前 main head。
  仅三项均通过才原子更新 PIN，失败保留旧 pin；不提交或推送。E3/E5/E6、文档评审与仓库门禁仍在提交前执行。
  main pin 用于保留尚未发布的上游合并补丁（math owners、unicode-math 空根指数），并不自动追踪每次 main 变更；构建和 driver 均按完整 commit 取源码。
