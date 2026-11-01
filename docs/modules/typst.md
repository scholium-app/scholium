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
`EditorWorker::spawn(wake)` 在 CPU 线程驻留 World/字体和 Content cache；默认使用 `FontProfile::System`：始终加载打包的 Noto Sans CJK SC、Liberation Serif 2.1.5
（Regular/Italic/Bold/BoldItalic）及 Typst embedded，并额外扫描宿主字体。
`FontProfile::Bundled` 使用相同字体列表与打包集合，但完全跳过宿主扫描；共享场景 headless worker 显式选择它，
parity 窗口子进程通过 `SCHOLIUM_TEST_FONTS=bundled` 选择它。World 创建后字体集不再改变。
五个打包字体以 Brotli-11 无损字节嵌入；纯 Rust 解压器在进程内首次加载时解压一次并缓存，
`bundled_cjk_font_bytes()` 让界面复用同一 Noto 原始字节，无文件读取或网络下载。
来源、版本、原始 SHA-256、许可与压缩再生成命令见[字体资产说明](../../crates/scholium-app/assets/fonts/README.md)。
`submit(Arc<StructuredDocument>, SceneStamp, RasterSpec, ViewRange)` 传递完整已接受快照、栅格分辨率与页面可见范围（pt），`poll()` 非阻塞返回
带 stamp 的结果。队列和结果分别只保留最新一份派生工作；关闭不在 UI join。
Scene 有两层：`base` 是整页、倍率不超过 1 px/pt（受页宽 ≤ `max_side` 与 `MAX_RASTER_PIXELS` = 64 MP 限制，仅作缺条带时的底图）；
`sharp` 是显示分辨率（`RasterSpec.px_per_pt`，由调用方按“页面物理像素宽 / 页宽 pt”给出，只会为页宽 ≤ `max_side` 而压低）下
固定 `SHARP_STRIP_PX` = 512 行的水平条带，稀疏，只含与可见范围相交的条带，因此逐键成本不随文档长度增长。
`strip_window(view, px_per_pt, page_height_px)` 给出可见条带与保留窗口（可见范围上下各加一个视口高度，共三个视口）。
`set_view(RasterSpec, ViewRange)` 对 worker 保留的最新已排版页面补渲染缺失条带（可见的先渲染，再由近及远），不重新排版；
结果由 `poll_strips()` 以 `StripUpdate`（含 Scene stamp、倍率、整页位图尺寸）逐条返回，只对同 stamp 的 Scene 有效。
新排版或新 `set_view` 到达时进行中的补渲染立即让路；窗口外的已交付条带从 worker 记录中移除；倍率变化清空记录。
单条带只渲染一个条带大小的像素图：按条带包围盒裁剪帧项（文字/形状/图片按边界，恒等变换的 Group 递归，带变换的 Group 整体保留），
再平移到条带位置渲染；与整页渲染对应行逐字节比较，最多相差 1 个最低有效位（实测与独立 stock 整页渲染逐字节相等）。
未裁剪时条带耗时随页长增长（typst-render 不按边界跳过项目），裁剪后与页长基本无关。`EditorStats.raster_ms` 为底图加可见条带耗时。
节点/子树未变则保留 Content，变动的祖先重建；传输与快照复制仍有全文成本。

结构直接构造正文/样式、Par、Equation、Frac、Attach 与 EditHole；字串不作为 source 执行。
Attach 为 `AttachElem`（t=sup、b=sub），槽编号 base 1、sub 2、sup 3。数学 Text 逐字素成原子：
每个字素一个 `ClassElem`（Typst 默认数学类）包一个带原子 origin 的 `TextElem`，ASCII 字母用数学
斜体码位显示；原子 origin 编码 叶/字节范围，几何把原子两端 caret 映回叶内字节，故每个字素边界
都有精确 caret。正文 Text 的 origin 必须落在最内层 `TextElem`：Strong/Emph 在布局前被 show 规则展开，
只挂在包装元素上的 origin 会丢失，样式叶就没有任何 caret 几何。`sin` 等多字母词暂按独立斜体字母显示（已知限制）。
键入的空格只分隔原子（生成源码丢弃它），故编辑面里空格原子保留 caret 但字号为 0、不占宽，并按 Normal 类
（默认无类会被当作字母而吃掉二元/关系符后的断点）；这样编辑面与预览在二元/关系运算符之后断行位置一致
（见[开发日志《公式内回车与长行内公式》](../log/2026-10-13-math-enter-wrap.md)）。无运算符的长串（含上标内）在 Typst 与 TeX 中都不可断。
生成源码（`structured_source`）对数学文字同样逐原子输出：ASCII 字母与数字串裸写并以空格分隔，
白名单运算符裸写，其余为转义引号串；Attach 写为 `base^(sup)_(sub)`，多原子 base 加括号。
World 的 source/file 均拒绝，today 无环境求值。RawMath 返回明确节点错误，既不改变模型
也不走旧输入期文字替代。受控 flow 宽为 A4 版心宽（210 mm − 2×2.5 cm，约 453.5 pt）、
高度预算 2400 pt（约 3.4 个 A4 版心高），超过预算拒绝而非截断；画面四周补 A4 页边距
（左右 2.5 cm、上下 2.54 cm），高度连续、不分页（"连续视图"）。

文档样式只有一处定义：`style.rs` 的常量（字体、字号、行距、首行缩进、语言、页边距）同时生成
预览 preamble 和编辑面的 Content 样式，二者的断行与垂直节奏必须一致（`editor/parity.rs` 以
同一夹具逐行比对）。标题在编辑面使用真正的 `HeadingElem`，与预览同套默认字号、间距与标题字体。
正文字体顺序固定为 Times New Roman → Liberation Serif → SimSun → Noto Sans CJK SC，
标题为 Times New Roman → Liberation Serif → SimHei → Noto Sans CJK SC；两种 profile 都直接使用这两份列表。
System 下已安装的 Times New Roman/宋体/黑体优先；没有时使用打包的度量兼容 Liberation Serif 与全宽 Noto CJK。
Bundled 的两页中英混排测试固定每一行首个源 byte；外部字体度量比较是显式运行的本地测试，
不让普通测试依赖商业字体或宿主中文字体。该断行契约不保证不同字体的字形外观或任意未覆盖字符的 fallback 一致。
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

## 上游跟进

- fork 基线只钉在 Typst 正式发布版。当前是 v0.15.1（`9dfd3a0`），这个提交在上游 `0.15` 发布分支上，不在 main 上。
  `vendor/typst-edit` 只随新版本发布整体移动，并作为单独任务完成。
- 每周一 03:17 UTC，`.github/workflows/typst-upstream.yml` 运行 `scripts/typst-upstream-drift.py`（也可手动触发），
  对上游 main 依次检查三个阶段：
  1. 四份补丁能否应用：clean 表示直接应用，merged 表示 git 三方合并成功，conflict 表示需要人工解决。
  2. `scholium-typst --features editor` 能否针对打补丁的树编译。
  3. 打补丁的树与同一提交、未打补丁的上游是否逐像素一致。

  退出码 1/2/3 表示第一个失败的阶段。结果表写入 job summary，日志作为 artifact 上传。这个 job 不在 push 或 PR 上运行，
  不阻塞 PR，也不自动开 issue。上游不接受 AI 编写的代码，跟进只在本仓库内进行，不向 typst/typst 提交任何内容。
- 新版本发布时，以 drift job 最近一次绿灯运行的状态为起点做 rebase。只要结果还停在 conflict，就以结果表列出的冲突文件和上游提交为准，
  并参照最近一次试 rebase 留下的补丁序列和 adapter 改动（见下方开发日志）。
- 补丁规模预算：13 个库相对上游的净新增少于 600 行。当前净新增 592 行，删除 16 行，涉及 21 个文件。
  会超出预算的新能力，优先放在 adapter 里实现。2026-10-31 的试 rebase 见[开发日志](../log/2026-10-31-typst-upstream-tracking.md)。

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

## 项目语言合约

本模块属于 Typst 内核，支持 SDG 权威的原生结构项目与 .typ 源码项目；项目语言在创建/导入时固定。
Typst 同时负责预览与最终输出，动态代码保留 RawTypst，受控编译不反推可逆语义。
跨语言只经转换或导出及逐节点报告；混合组件、跨引擎桥接与团队语言切换已推迟。
共同要求见 [项目语言、跨格式转换与文件操作](../MIXED_SOURCE_EDITING.md) 与 [ADR 0041](../adr/ADR-0041-separate-latex-typst-kernels.md)。

## WASM 验证边界

官方 Rust syntax 与编译引擎作为候选；纯语法和投影优先移植。浏览器编译另测 Worker、版本 features、字体、包资源、缓存、内存与取消；World 通过宿主注入声明资源，不能直接依赖本机文件系统，也不能因为引擎使用 Rust 就宣称 WASM 已可用。见[全栈候选](../plan/TECH_STACK.md)及[WASM](../plan/WASM.md)。

阶段 0 独立沙箱 worker 的编译、内省与导出证据见
[报告 0018](../spikes/SPK-0018-typst-worker-isolation.md)与[ADR 0012](../adr/ADR-0012-typst-worker-isolation.md)。
其中混合构建只作追溯，已按 ADR 0041 推迟；临时 JSON 不是正式存储或网络协议。

`spawn_with_wake` 的回调在编译/页面事件入队后通知宿主重绘，不能阻塞工作线程。
输入队列通过通道唤醒线程，无输入时阻塞等待；丢弃句柄关闭通道，不在 UI 线程 join。
应用启动时提前创建常驻 worker，使字体扫描尽量不落在首次键入之后。
决策与测量边界见 [ADR 0030](../adr/ADR-0030-incomplete-formula-feedback.md)。
