# scholium-bib

## 职责

实现 BibTeX/BibLaTeX lossless CST、条目索引、字段值表达式、crossref/xdata 解析、验证、citation key
重命名、文献搜索、IR/CSL-JSON 投影与 BibTeX 生成。

## 非职责

不联网补全文献，不决定引用样式排版，不在保存时排序字段，不丢弃未知 entry/field。

## 内部结构

```text
src/
├── lexer.rs       at/type/key/value/comment token
├── parser.rs      entry/string/preamble/comment/error CST
├── value.rs       literal、braced、quoted、macro、concatenation
├── index.rs       key → definitions/usages
├── validate.rs    duplicate、required field、cycle、date
├── resolve.rs     string macro/crossref 的非破坏性规范值
├── edit.rs        insert/update/rename 的最小 patch
├── projection.rs  bibliography IR/CSL-JSON + extension
└── generate.rs    新条目和跨格式输出
```

## 数据语义

索引同时保存 raw value 与 resolved value。resolved value 带来源字段链，循环或未知宏返回部分结果和诊断。
key 区分规则按项目配置；默认精确区分大小写，避免跨工具行为不一致。

## 编辑

表单编辑不能直接重写整条 entry。每个字段变更生成精确 patch，并按相邻条目风格选择缩进、逗号和
quote/braces。新增条目使用项目 formatter profile；显式“整理文献库”才允许排序或规范化。

key rename 是跨格式 WorkspaceTransaction：先查询 latex/markdown usages，再一次验证和提交。未知格式
中的文本匹配只作为候选，不自动替换。

## 公共接口

- `parse(resource, revision, text) -> BibCst`；`reparse(previous, edits) -> BibCst`。
- `index(cst) -> BibIndex`，同时暴露 raw value、resolved value 和定义来源链。
- `validate(cst) -> Vec<Diagnostic>`；`search(index, query) -> Vec<EntryRef>`。
- `rename_key(cst, old, new) -> TextPatch`；跨文件 rename 由 `scholium-format` 组织为 `WorkspaceTransaction`。
- `project(cst, selection) -> BibliographyIr`；`generate(entries, style) -> GeneratedArtifact`。

## 不变量

- CST token 拼接等于输入。
- 未知 entry type/field 始终保留。
- CSL 无对应字段进入 extension map 并生成 Equivalent/Degraded 报告。
- 重复 key 不用 last-one-wins 隐藏；索引返回多定义。
- 网络元数据抓取未来必须是外部 provider，不能进入 parser。

## 失败处理

- 未知 entry type/field、重复 key、crossref 循环和无法解析日期产生诊断，但保留原文并允许保存。
- 未定义或递归的 string macro 返回部分 resolved value 加诊断，不猜测取值。
- rename 目标 key 已存在、引用在验证后变化或 precondition 失败时拒绝提交，不自动替换未知格式中的文本匹配。
- CSL 无对应字段进入 extension map，并在报告中记为 Equivalent/Degraded。
- 随机损坏输入必须不 panic，且 token 仍覆盖全部输入。

## 测试门禁

BibTeX/BibLaTeX 夹具、string concatenation、nested braces、LaTeX accents、duplicate key、crossref cycle、
字段表单最小 patch、跨文件 rename、CSL 往返降级和随机损坏输入 fuzz。
