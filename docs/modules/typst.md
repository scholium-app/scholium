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

`generate_source(&StructuredDocument, SourceMode)` 从结构文档派生 Typst 文本，不回读：
正文按字面转义，粗体/强调为 `*…*`/`_…_`，一二级标题为 `=`/`==`；数学文字作字面量（单个字母/数字
裸写，其余加引号），分数为 `(a)/(b)`，必填空槽显示为 `□` 并计入 `holes`（严格输出须拒绝）。
`View` 模式给用户看，原串公式逐字显示；`Preview` 模式加预览 preamble 与 `blkstart{i}`/`blk{i}`
块锚点，原串公式换成可见占位、**从不在进程内编译**，其 ID 计入 `raw`。`blocks` 给出每块的源码区间。

`PreviewCompiler::submit(document, generation, source, block_count)` 编译预览模式源码，
`request_page(document, generation, page, px_per_pt)` 按需栅格，`poll() -> PreviewEvent` 非阻塞。
`generation` 是调用方单调递增的提交序号而非 revision（撤销会在新 epoch 回到旧 revision）；
请求槽只保留最新一份。`CompileOutcome` 含页数、耗时、错误/Typst 警告与块锚点，
`block_at_click(anchors, page, y_pt)` 把预览点击映射到块序号。旧 `DocumentSnapshot` 的投影、
字形几何与输入期公式反馈随旧编辑器删除（[ADR 0037](../adr/ADR-0037-remove-legacy-editor.md)）。
这一窄接口不是上面的正式 `compile(revision, inputs)` 沙箱构建接口。

## 直接 Content 开发接口

`editor` feature 由 `scholium-app` 启用，并打开 `typst-layout/editor`；它要求根 workspace
统一 patch 的锁定 fork 13 个库（[ADR 0036](../adr/ADR-0036-typst-default-editor.md)）。
`typst-main-fork-check` 示例按同一 fixture 输出 fork 像素，供与独立 stock 参照逐像素比较。
`EditorWorker::spawn(wake)` 在 CPU 线程驻留 World/字体和 Content cache；
`submit(Arc<StructuredDocument>, SceneStamp)` 传递完整已接受快照，`poll()` 非阻塞返回
带 stamp 的结果。队列和结果分别只保留最新一份派生工作；关闭不在 UI join。
节点/子树未变则保留 Content，变动的祖先重建；传输与快照复制仍有全文成本。

结构直接构造正文/样式、Par、Equation、Frac、Attach 与 EditHole；字串不作为 source 执行。
Attach 为 `AttachElem`（t=sup、b=sub），槽编号 base 1、sub 2、sup 3。数学 Text 逐字素成原子：
每个字素一个 `ClassElem`（Typst 默认数学类）包一个带原子 origin 的 `TextElem`，ASCII 字母用数学
斜体码位显示；原子 origin 编码 叶/字节范围，几何把原子两端 caret 映回叶内字节，故每个字素边界
都有精确 caret。`sin` 等多字母词暂按独立斜体字母显示（已知限制）。
生成源码（`structured_source`）对数学文字同样逐原子输出：ASCII 字母与数字串裸写并以空格分隔，
白名单运算符裸写，其余为转义引号串；Attach 写为 `base^(sup)_(sub)`，多原子 base 加括号。
World 的 source/file 均拒绝，today 无环境求值。RawMath 返回明确节点错误，既不改变模型
也不走旧输入期文字替代。受控 flow 宽为 A4 版心宽（210 mm − 2×2.5 cm，约 453.5 pt）、
高度预算 2400 pt（约 3.4 个 A4 版心高），超过预算拒绝而非截断；画面四周补 A4 页边距
（左右 2.5 cm、上下 2.54 cm），高度连续、不分页（"连续视图"）。

文档样式只有一处定义：`style.rs` 的常量（字体、字号、行距、首行缩进、语言、页边距）同时生成
预览 preamble 和编辑面的 Content 样式，二者的断行与垂直节奏必须一致（`editor/parity.rs` 以
同一夹具逐行比对）。标题在编辑面使用真正的 `HeadingElem`，与预览同套默认字号、间距与标题字体。
标题编号**不在** preamble 中：编辑面以空 introspector 布局，无法运行 counter；编号随文档模板回归。
空块不设首行缩进：fork 以行文本锚定空行 caret，缩进占位字符会使该行失去 caret。
预览源码的块起点锚点对标题单独成行放在 `=` 之前，因为标题正文前的元素会改变标题块间距。

`EditorScene` 的 raster 和 `EditGeometry` 都取同一个 padded Frame；caret 位置是 typed
NodeId、叶内 UTF-8 byte 与 affinity，top/bottom 为该 Frame 的 pt。opaque ID 只在 adapter
注册/反查，不进入模型或存储。剪裁/未知 origin 拒绝；跨叶 derived cluster 不猜端点；合字
内部按比例的 caret 保留 `exact=false`。调用方只用当前全 stamp 场景做命中/IME 锚点。
`selection_text(leaf, range)` 从 shaped cluster 的变换后 stops 切分非打印 SelectionQuad，
保留逐行/双向 cluster 的独立四角，验证范围完整覆盖；缺边界或映射即报 Geometry。
空范围返回 1 pt 的边界标识，不冒充字形宽度。比例合字边界保留 exact=false。
`selection_node(math_root)` 使用真正数学根的结构 bounds，包含分数横线和各槽位；
inline 包装身份没有对应 bounds 时不得猜矩形。所有四角与 raster 共用 padded Frame pt。
端点选择与编辑请求仍由 app/core 负责，adapter 不修改权威或从 markup 偏移猜选框。
SceneStats 记录 built/reused/source_reads/elapsed，不能作为物理刷新或分页性能证明。

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

`spawn_with_wake` 的回调在编译/页面事件入队后通知宿主重绘，不能阻塞工作线程。
输入队列通过通道唤醒线程，无输入时阻塞等待；丢弃句柄关闭通道，不在 UI 线程 join。
应用启动时提前创建常驻 worker，使字体扫描尽量不落在首次键入之后。
决策与测量边界见 [ADR 0030](../adr/ADR-0030-incomplete-formula-feedback.md)。
