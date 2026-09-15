# scholium-typst

## 职责

提供 Typst lossless source/CST adapter、常用静态语义投影、SDG → Typst 稳定生成器、生成源码 reconcile、
Typst World/编译桥接、诊断和 Frame/文档位置映射。

## 非职责

不把任意 Typst 程序求值后反推语义，不保存文档历史，不绘制应用 UI，不决定 LaTeX 转换政策。

## 内部结构

```text
src/
├── syntax.rs       parser 封装、lossless range 与 error node
├── semantics.rs    markup/math/静态函数 capability
├── projection.rs   Typst CST → SDG/IR
├── generate.rs     SDG → 稳定 Typst source
├── source_map.rs   NodeId ↔ source range ↔ layout span
├── reconcile.rs    generated CST diff → SemanticPatch
├── world.rs        字体、资源、package policy、缓存
├── compile.rs      revision-aware compile
└── diagnostics.rs  Typst errors → NodeId/SourceRange
```

## 支持边界

markup、heading、list、emphasis、link、label/ref/cite、静态 figure/table、标准 math 和 Scholium 生成函数为
可理解子集。任意 code block、循环、状态、动态内容和未知函数是 RawTypst；可编译不代表可 reconcile。

## 生成器

每种 SDG node 有确定 renderer，输入为不可变 StyleContext。生成时记录所有节点范围，避免依赖在源码中
插入可见标记。模板/helper 定义与正文分离，generator schema 版本写入 generation metadata。

## Reconcile

只接受同 generator schema 的 base。比较新旧 CST，将受支持属性/结构变化转为 SemanticEdit；新增未知合法
语法可包装 RawTypst；删除生成边界、破坏 helper 契约或跨多个节点的动态代码进入冲突预览。

## 不变量

- 生成相同 SDG 和 profile 得到相同源码。
- 编译结果绑定输入 revision，过期结果不可发布。
- RawTypst 不在其他方言生成时静默丢失。
- source span 找不到 NodeId 时标记 derived，不猜测最近节点。
- package/network 访问遵守 build/security policy。

## 测试门禁

所有 SDG 节点生成、数学/中文/字体、SourceMap、生成源码局部编辑 reconcile、Raw 动态代码、编译诊断、
缓存失效和旧 revision 淘汰。与锁定 Typst 版本的 golden 结果分开保存。

## 团队源码语言与混合项目合约

新增 Typst ForeignSource 的作用域、参数/结果、模板及符号桥接描述。动态代码可在受控 Typst 组件中执行，
执行结果不反推可逆语义图；跨到 LaTeX 由 build 选择已验证载体与装配策略。
Typst 主入口引用 LaTeX 组件使用显式绑定/预生成资源，不把 preamble 当 Typst 代码。编译 API 由 build 的沙箱
worker 调用，不能在 core 中绕过取消、内存和网络策略。新增正文/模板/引用及载体能力兼容测试。

共同要求见 [混合源码与团队编辑](../MIXED_SOURCE_EDITING.md) 与 [ADR 0001](../adr/0001-mixed-source-team-editing.md)。

## WASM 验证边界

官方 Rust syntax 与编译引擎作为候选；纯语法和投影优先移植。浏览器编译另测 Worker、版本 features、字体、包资源、缓存、内存与取消；World 通过宿主注入声明资源，不能直接依赖本机文件系统，也不能因为引擎使用 Rust 就宣称 WASM 已可用。见[全栈候选](../TECH_STACK.md)及[WASM](../WASM.md)。
