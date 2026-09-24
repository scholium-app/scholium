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

## 公共接口

- `parse(resource, revision, text) -> TypstCst`；`reparse(previous, edits) -> TypstCst`。
- `project(cst, selection) -> Projection { ir, coverage, diagnostics }`。
- `generate(sdg, profile) -> GeneratedArtifact + SourceMap`，generation metadata 记录 generator schema 版本。
- `reconcile(generation, edited_source) -> ReconcilePlan`，只接受同 generator schema 的 base。
- `compile(revision, inputs) -> CompileResult`，由 `scholium-build` 的沙箱 worker 调用，core 不得绕过取消、
  内存与网络策略。
- `locate(span) -> Option<NodeId | Derived>`；`diagnostics() -> Vec<Diagnostic>`。

本地受信块文档的预览适配器提供 `PreviewCompiler::submit_snapshot(&DocumentSnapshot)`、
`request_page(document, revision, page)` 与 `poll() -> PreviewEvent`。`CompileOutcome` 包含
文档 ID/revision、页数、耗时、错误/输入期提示、块首/末锚点与 `Vec<PageGeometry>`。
`GlyphBox { block, input, rect, decoration }` 的 `input` 是块的规范标记文本 UTF-8 字节范围，
`rect` 是页面 pt 的左/上/右/下坐标。编译器遍历实际 Frame 变换、glyph span/offset 和形状，
通过生成源码区间关联稳定 NodeId；字符偏移不等于 Unicode 字符序号或 UTF-16。
数学命名符号保留完整 token，装饰与空槽显式区分，未映射内容不猜测归属。

当前页栅格结果单独交付；调用方须同时校验文档 ID、revision 和页码，再组合画面与几何。
`submit(document, revision, source, block_count)` 仅供无编辑映射的源码夹具编译，返回空字形映射。
这一窄接口不是上面的正式 `compile(revision, inputs)` 沙箱构建接口。
直接编辑边界见 [ADR 0029](../adr/ADR-0029-direct-page-editing.md)。

## 不变量

- 生成相同 SDG 和 profile 得到相同源码。
- 编译结果绑定输入 revision，过期结果不可发布。
- RawTypst 不在其他方言生成时静默丢失。
- source span 找不到 NodeId 时标记 derived，不猜测最近节点。
- package/network 访问遵守 build/security policy。
- 复用 World 和字体缓存时，源码更新必须使依赖缓存正确失效；源码 span 按当前 Source 解析，
  不复用前一 revision 的字节范围。缓存淘汰受会话资源限制约束。

## 失败处理

- 任意 code、循环、状态与未知函数进入 RawTypst；可编译不等于可 reconcile，不反推语义。
- 只接受同 generator schema 的 base；schema 不匹配拒绝 reconcile 并进入冲突预览。
- 删除生成边界、破坏 helper 契约或改动跨多个节点的动态代码进入冲突预览，不静默重写。
- package/network 访问被 build/security policy 拒绝时返回诊断，不绕过策略。
- 编译结果 revision 过期时丢弃，不发布；source span 找不到 NodeId 时标记 derived，不猜最近节点。
- 锁定 Typst 版本的 golden 差异显式失败，不自动更新基线。

## 测试门禁

所有 SDG 节点生成、数学/中文/字体、SourceMap、生成源码局部编辑 reconcile、Raw 动态代码、编译诊断、
缓存失效和旧 revision 淘汰。与锁定 Typst 版本的 golden 结果分开保存。

## 团队源码语言与混合项目合约

新增 Typst ForeignSource 的作用域、参数/结果、模板及符号桥接描述。动态代码可在受控 Typst 组件中执行，
执行结果不反推可逆语义图；跨到 LaTeX 由 build 选择已验证载体与装配策略。
Typst 主入口引用 LaTeX 组件使用显式绑定/预生成资源，不把 preamble 当 Typst 代码。编译 API 由 build 的沙箱
worker 调用，不能在 core 中绕过取消、内存和网络策略。新增正文/模板/引用及载体能力兼容测试。

共同要求见 [混合源码与团队编辑](../MIXED_SOURCE_EDITING.md) 与 [ADR 0001](../adr/ADR-0001-mixed-source-team-editing.md)。

## WASM 验证边界

官方 Rust syntax 与编译引擎作为候选；纯语法和投影优先移植。浏览器编译另测 Worker、版本 features、字体、包资源、缓存、内存与取消；World 通过宿主注入声明资源，不能直接依赖本机文件系统，也不能因为引擎使用 Rust 就宣称 WASM 已可用。见[全栈候选](../plan/TECH_STACK.md)及[WASM](../plan/WASM.md)。

当前 mixed-build spike 的 Typst 编译、内省和 PDF/SVG 导出已进入独立沙箱 worker，
见[报告 0018](../spikes/SPK-0018-typst-worker-isolation.md)与[ADR 0012](../adr/ADR-0012-typst-worker-isolation.md)。
每次新进程不保留增量缓存；临时 JSON 不是正式存储或网络协议。

## 公式输入期反馈

`submit_snapshot` 在严格编译失败且每条诊断均落在已知 Math 源范围内时，
仅把这些公式在交互投影中渲染为原位置的琥珀色文字，返回当前 revision 的可编辑字形。
`CompileOutcome.warning` 保留原始诊断，`error` 为空表示页面已生成，不表示公式通过最终检查。
其它公式照常排版；语义快照与 `generate_typst` 不变。未知来源错误不猜测范围，仍报失败。
`submit` 的严格源码编译不采用输入期替代；正式输出不能使用该交互页面冒充有效结果。

`spawn_with_wake` 的回调在编译/页面事件入队后通知宿主重绘，不能阻塞工作线程。
输入队列通过通道唤醒线程，无输入时阻塞等待；丢弃句柄关闭通道，不在 UI 线程 join。
应用启动时提前创建常驻 worker，使字体扫描尽量不落在首次键入之后。
决策与测量边界见 [ADR 0030](../adr/ADR-0030-incomplete-formula-feedback.md)。
