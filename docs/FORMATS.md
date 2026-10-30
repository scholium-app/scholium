# 格式系统设计

## 1. 能力层级

每种格式分别声明五级能力，不能用一个“支持”概括：

1. `edit`：原文可直接编辑和无损保存。
2. `understand`：能产生 CST、符号、诊断和语义命令。
3. `project`：能映射到规范化 IR。
4. `generate`：能从 IR 生成该格式并报告降级。
5. `reconcile`：编辑生成源码后能否可靠映射回语义图。

LaTeX/Typst 的 edit 覆盖任意文本，understand/project 只覆盖明确子集；生成源码 reconcile 属于 Typst 内核。
项目语言在创建/导入时固定，Markdown/BibTeX 作为资源不改变项目语言；DOCX 首发可能只有 generate。

## 2. FormatAdapter 合约

```rust
trait FormatAdapter {
    fn descriptor(&self) -> FormatDescriptor;
    fn parse(&self, input: ParseInput) -> ParseOutput;
    fn reparse(&self, previous: ParseTree, edits: &[TextEdit]) -> ParseOutput;
    fn symbols(&self, tree: &ParseTree) -> SymbolIndex;
    fn dependencies(&self, tree: &ParseTree) -> Vec<ResourceDependency>;
    fn lower_edit(&self, tree: &ParseTree, edit: SemanticEdit) -> PatchResult;
    fn project(&self, tree: &ParseTree, selection: Option<SourceRange>) -> Projection;
    fn generate(&self, ir: &DocumentIr, options: GenerateOptions) -> GeneratedArtifact;
    fn reconcile(&self, base: Generation, edited: &str) -> ReconcilePlan;
}
```

适配器必须是确定性的；解析和生成不访问网络、不启动进程、不读取声明之外的文件。

## 3. LaTeX

首发理解范围：文档结构、常见文本命令、列表、数学定界符与常见环境、label/ref/cite、figure、
table、bibliography、input/include、newcommand 定义位置。宏调用默认不展开。

解析采用容错 token/CST：控制序列、组、可选参数、环境、注释、数学切换和 error node。verbatim 类
环境由白名单和项目声明识别。未知命令保留参数的词法结构，不推测语义。

项目解析器建立 include graph，检测循环、缺失资源和逃逸项目根的路径。构建日志通过文件/行列映射
为诊断；SyncTeX 或等价映射只用于预览定位，不成为编辑真相。

## 4. Markdown

基线语法和扩展集写入项目配置。首发建议固定 CommonMark 基线，并显式启用表格、脚注、任务列表、
数学和 citation 扩展。不同方言不能靠自动猜测后静默保存。

解析器必须保留原始标记选择，例如 `*`/`_`、围栏长度、列表编号和空白。格式化是显式命令，普通
保存不规范化这些差异。

## 5. Typst

首发理解 markup、math、heading、list、figure、table、label/ref/cite 和由 Scholium generator 产生的函数调用。
任意 Typst code、动态循环、状态与自定义函数调用可直接源码编辑，但默认投影为 RawTypst。
RawTypst 在 Typst 中保留原文并由受控引擎构建；跨方言降级或阻止导出，不反推任意程序语义。

StructuredAuthority 的 Typst generator 必须产生稳定源码与 NodeId SourceMap；生成源码的受支持修改可
reconcile 回语义图。SourceAuthority Typst 项目以 lossless CST/source 为真，结构视图只编辑可逆区域。

## 6. BibTeX

保留 entry 类型、字段顺序、大小写、注释、string macro、concatenation 与原始 value token。
规范化值只进入索引。citation key 重命名必须跨所有已知源文件生成 WorkspaceTransaction。

重复 key、循环 crossref、缺失必填字段和无法解析日期产生诊断，不阻止保存。CSL-JSON 转换保留
无法映射字段到 extension map，并在报告中列出。

## 7. 导入

导入是分析流程，不是立即转换：

1. 探测编码、入口和格式，固定项目语言为 Typst 或 LaTeX。
2. 构建依赖图，列出缺失/根外/远程资源。
3. 解析并统计 understood、raw、error 节点。
4. 检查构建命令和潜在外部执行。
5. 展示报告，让用户选择原地打开或复制到新目录。

原地打开不产生正文变更。复制导入保留目录结构，冲突文件使用明确映射并写入报告。

## 8. 导出

导出管线为 `SDG 或 source/CST → conversion IR → target generator → optional builder`。原格式导出优先
直接复制或最小 patch，不绕 IR。跨格式输出包含 artifact、资源目录、`report.json` 和 UI 摘要。

每个 IR 节点结果为 `Exact`、`Equivalent`、`Degraded`、`Dropped` 或 `RawPreserved`。首发门禁禁止
未确认的 `Dropped`；用户可配置将部分降级提升为错误。

LaTeX ↔ Typst 仅经一次性转换或导出，生成目标源码、资源与逐节点报告，目标工具链在沙箱中验证。
原生 SDG 项目导出 LaTeX 是导出能力，不在 Typst 项目内编辑 LaTeX 目标。
报告覆盖未投影节点、宏、模板与资源，另列编辑性、可移植性与依赖；保留原文不代表成功转换。
Raw 同方言无损保留，跨方言降级或阻止导出。混合组件、跨引擎桥接、混合重建包及标准目标包中的
外语预生成资源已按 [ADR 0041](adr/ADR-0041-separate-latex-typst-kernels.md) 推迟，见
[项目语言、跨格式转换与文件操作](MIXED_SOURCE_EDITING.md)。

### 保存与另存为

普通保存按当前权威模式写回，不调用跨格式生成；源码有语法错误仍可保存。生成源码的未提交草稿单独持久化。
同格式另存为创建独立项目身份；「转换为…」生成目标入口、资源依赖与兼容报告，建立新项目并保留原项目。
导出只生成副本，不改变工作项目、权威模型或项目语言。输入快照明确包含哪些草稿，不能默认忽略新输入。
打开普通 `.tex`/`.typ` 不要求 Scholium 元数据；可重建源码交付包含目标工具链所需的资源及依赖说明。

## 9. Copy As

复制时锁定 selection 对应 revision，投影最小完整语义范围，并生成多 MIME 剪贴板：

- 始终提供 `text/plain`。
- HTML 可提供 `text/html`。
- 目标源码提供产品内部 MIME 与纯文本表示。
- 文献可提供 BibTeX、CSL-JSON 和格式化文本。

超过阈值的降级先弹出摘要。复制不修改文档，也不把临时生成文件留在项目目录。

## 10. 格式矩阵

| 格式 | edit | understand/project | reconcile | generate/build |
|---|---:|---:|---:|---:|
| LaTeX | 首发 | 常用论文子集 | 不适用（源码权威） | 同引擎预览/PDF/源码 |
| Typst | 首发 | generator + 常用子集 | generator 子集 | 预览/PDF/源码 |
| Markdown | 首发 | 配置方言 | 支持子集 | HTML/PDF/源码 |
| BibTeX | 首发 | 首发 | 表单字段 | BibTeX/CSL-JSON |
| DOCX | 否 | 导入后续 | 否 | 候选 |
| EPUB | 否 | 否 | 否 | 候选 |

具体第三方解析器和转换后端由技术验证确定，选择结果写 ADR，不把库名写进公共数据模型。
