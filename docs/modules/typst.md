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
`Equation` 块恰含一个 Math inline，编辑面映射为 `EquationElem(block: true)`；源码以 `$ … $` 独立成段生成，块范围包括公式分隔符与预览锚点。
`View` 模式给用户看，原串公式逐字显示；`Preview` 模式加预览 preamble 与 `blkstart{i}`/`blk{i}`
块锚点，原串公式换成可见占位、**从不在进程内编译**，其 ID 计入 `raw`。`blocks` 给出每块的源码区间。
空段落在两种模式中均显式生成 `#par[#text("")]`：空文本提供与编辑面相同的字体行高，
并保留 leading、段间距和每一个连续空段落（包括开头、结尾与标题旁）；单独的 `#par[]` 不具有相同的行高。
空段落的 `SourceBlock` 范围包含整个构造和 Preview 模式的两端块锚点，源码高亮仍对应独立的非空范围。

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
独立 stock 参照对空段落夹具编译普通 Typst 源码，以自动页高同时验证文本位置与末尾空行高度；
覆盖开头、中间一行/三行、结尾、标题旁及纯空段落，尺寸与 RGBA 字节必须完全相同。
源码参照按自身 FontBook 过滤正文/标题字体偏好中的缺失家族并保留原顺序，与 Content 的 fallback 选择一致；
stock 对照同时覆盖默认字体集和仅打包字体集，两者都要求源码编译零警告。
`EditorWorker::spawn(wake)` 在 CPU 线程驻留 World/字体和 Content cache；默认使用 `FontProfile::System`：始终加载打包的 Noto Sans CJK SC、Liberation Serif 2.1.5
（Regular/Italic/Bold/BoldItalic）及 Typst embedded，并额外扫描宿主字体。
`FontProfile::Bundled` 使用相同字体列表与打包集合，但完全跳过宿主扫描；共享场景 headless worker 显式选择它，
parity 窗口子进程通过 `SCHOLIUM_TEST_FONTS=bundled` 选择它。World 创建后字体集不再改变。
五个打包字体以 Brotli-11 无损字节嵌入；纯 Rust 解压器在进程内首次加载时解压一次并缓存，
`bundled_cjk_font_bytes()` 让界面复用同一 Noto 原始字节，无文件读取或网络下载。
`bundled_math_font_bytes()` 通过同一解压函数和进程缓存提供额外的 Noto Sans Math `.ttf.br` 原始字节；该字体只供界面回退，不加入 Typst 正文字体集。
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

结构直接构造正文/样式、Par、Equation、Frac、Attach、Root、Lr 与 EditHole；字串不作为 source 执行。
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
Root 使用 `RootElem`，index 槽 1、radicand 槽 2；`LrElem` 的 body 为槽 1，fork 为展开后的
内部组件传播该槽 origin 而不替换节点 origin。源码分别生成 `sqrt(x)`、`root(n, x)` 和
`lr(( x ))` 等配对；带逗号的根式参数加数学分组以保留字面含义。
定界符作为另一结构的槽时，用不打印的单子 Sequence 隔离父槽 metadata，保留它自身的 body 槽标记。
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
空段落与非空段落遵循相同首行缩进（`all: true`）；fork 在空行全范围内寻找文本 origin，
并将 caret 放在前置间距项之后，因此空段落 caret 与同样式首字的 x 一致；标题保持无缩进。
预览源码的块起点锚点对标题单独成行放在 `=` 之前，因为标题正文前的元素会改变标题块间距。

`EditorScene` 的 raster 和 `EditGeometry` 都取同一个 padded Frame；caret 位置是 typed
NodeId、叶内 UTF-8 byte 与 affinity；Fraction/Attach/Root/Delimited ID 上 byte 0/1 表示结构前/后。
`editor/stops.rs` 共用结构停点规则，键盘导航仅扫描当前文字叶，遍历内存按节点数增长。
worker 的 `edge_geometry` 从已有结构/后代 bounds 并集取左右界，以整公式高度生成边缘 caret；同 Row
邻接 Text/Hole 供应的同侧边界不重复生成。空正文 Text 在公式前后没有独立 shaped caret 时，由相邻公式已记录 bounds 的对应边界提供正文 caret，
与公式内边缘保留不同身份；不插入 Content 或改变栅格。边缘无 Content，也不参与打印或源码生成；
top/bottom 为该 Frame 的 pt。opaque ID 只在 adapter
注册/反查，不进入模型或存储。剪裁/未知 origin 拒绝；跨叶 derived cluster 不猜端点；合字
内部按比例的 caret 保留 `exact=false`。调用方只用当前全 stamp 场景做命中/IME 锚点。
`selection_text(leaf, range)` 从 shaped cluster 的变换后 stops 切分非打印 SelectionQuad，
保留逐行/双向 cluster 的独立四角，验证范围完整覆盖；缺边界或映射即报 Geometry。
空范围返回 1 pt 的边界标识，不冒充字形宽度。比例合字边界保留 exact=false。
`selection_node(math_root)` 使用真正数学根的结构 bounds，包含分数横线和各槽位；
`selection_math` 对无自身 bounds 的 Row/Attach 递归收集各子树已记录 bounds，供正文整公式和数学行选区使用；
inline 包装身份没有对应 bounds 时不得猜矩形。所有四角与 raster 共用 padded Frame pt。
端点选择与编辑请求仍由 app/core 负责，adapter 不修改权威或从 markup 偏移猜选框。
SceneStats 记录 built/reused/source_reads/elapsed，不能作为物理刷新或分页性能证明。

## 上游跟进

- fork 基线只钉在 Typst 正式发布版。当前是 v0.15.1（`9dfd3a0`），这个提交在上游 `0.15` 发布分支上，不在 main 上。
  `vendor/typst-edit` 只随新版本发布整体移动，并作为单独任务完成。
- 每周一 03:17 UTC，`.github/workflows/typst-upstream.yml` 运行 `scripts/typst-upstream-drift.py`（也可手动触发），
  对上游 main 依次检查三个阶段：
  1. `0001`–`0005` 五份补丁能否应用；其中 `0005-delimiter-body-slot-origin.patch` 保留定界符 body 的编辑槽 origin。
     clean 表示直接应用，merged 表示 git 三方合并成功，conflict 表示需要人工解决。
  2. `scholium-typst --features editor` 能否针对打补丁的树编译。
  3. 打补丁的树与同一提交、未打补丁的上游是否逐像素一致。

  退出码 1/2/3 表示第一个失败的阶段。结果表写入 job summary，日志作为 artifact 上传。这个 job 不在 push 或 PR 上运行，
  不阻塞 PR，也不自动开 issue。上游不接受 AI 编写的代码，跟进只在本仓库内进行，不向 typst/typst 提交任何内容。
- 新版本发布时，以 drift job 最近一次绿灯运行的状态为起点做 rebase。只要结果还停在 conflict，就以结果表列出的冲突文件和上游提交为准，
  并参照最近一次试 rebase 留下的补丁序列和 adapter 改动（见下方开发日志）。
- 补丁规模预算：13 个库相对上游的净新增少于 600 行。当前新增 593 行、删除 16 行（净增 577 行），涉及 21 个文件。
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

## 显示专用 IME run

`EditorWorker::submit_with_preedit` 接受独立的 `Preedit { at, text, cursor }`，`at` 是已接受正文叶内
UTF-8 byte，`cursor` 是 preedit 内 UTF-8 byte（缺省为末尾）。它不修改传入快照，也不生成源码。
Content cache 只使前后受影响正文叶及其段落失效；未变的段落继续复用。正文 Text 在插入点分成
前串、带 `UnderlineElem` 的字面预编辑串和后串，同一 Typst 文档样式负责字体、断行、段落重排。
无 egui 文本叠层。每次更新使用独立 layout request identity，旧结果不能覆盖新组合或已提交场景。
`EditorScene.preedit` 给出显示 caret；组合中 `geometry` 的 split-run byte 只用于显示审计，不能回答
真实文档的命中、导航、选区或 AccessKit 几何查询。取消/提交后正常 Scene 恢复该权限。
数学预编辑暂不注入 Content；数学 Commit 保持原有字面提交与撤销语义。
边界决策见 [ADR 0044](../adr/ADR-0044-transient-ime-layout.md)。

`BlockKind::Equation` 的内容严格为一个 `InlineBody::Math`，直接构造为 `EquationElem` 并开启 `block`。源码生成 `$ … $` 独立段落；SourceBlock 范围覆盖分隔符以及 Preview 锚点，使源码视图能准确高亮。Equation 结构进入 math indicator，并由 AccessKit 以本地化独立公式名称暴露。


## 统一编译诊断

`map_diagnostics(world, items, blocks)` 将 Typst 原生错误/警告映射到 model 的统一 `Diagnostic`，
通过当前 World 重新解析 span，取得 `.typ` 或生成源码的项目相对路径、1-based Unicode scalar 行列与 UTF-8 半开范围。
不能解析的 span 保留消息与级别，不推测文件或节点；非 main 文件不使用 main 的生成块映射。
只有诊断范围完整落在当前 `SourceBlock.range` 中才填入该块的 NodeId。

常驻 `PreviewCompiler` 的 `CompileOutcome.diagnostics` 总是绑定本次输入 generation（与既有 revision 字段同义）；
成功编译的 warning 和失败编译的 error 都进入统一列表，原 error/warning 字符串保留兼容。
`submit_generated(document, generation, GeneratedSource)` 连同当前块映射提交，供原生项目按节点定位；
原有 `submit` 没有源节点映射，仅提供文件/行列。调用方仍按文档身份与 generation 拒绝旧结果。
此接口不扩大 in-process preview 的执行权限；外部 `.typ` 编译仍须遵守既有沙箱规则。

## 源码项目的隔离预览入口

`SourceHighlighter::highlight` 用官方 CST 的增量 replace/reparse 与 highlight tag 提供不重叠 UTF-8 样式范围，
`highlight_source` 是不可变生成源码的便捷入口。两者只解析，不执行用户程序。

`compile_source_snapshot(entry, files, scale)` 仅供受监督的 OS 隔离 worker 调用；外部源码不得在 UI 进程调用此入口。
显式资源 map 是 World 的唯一 source/file 能力，路径必须为规范项目相对路径；包、网络、宿主任意路径及系统时间不可访问。
入口必须存在且为 UTF-8，资源总量最多 16 MiB；编译后最多 100 页、RGBA 总量 64 MiB、单边 8192 px，
在栅格化前检查，越界返回失败，不能截断为成功。返回页图、真实 Typst 诊断与失败文本，不跨进程传 PagedDocument。
宿主负责 OS 网络/挂载、内存、CPU、墙钟预算与取消整棵进程树；失败后保留最后成功页面且不得进程内重试。
原生生成源码的 PreviewCompiler 执行权限保持原有约束。部署边界见
[ADR 0055](../adr/ADR-0055-source-preview-worker.md)。


## 源码焦点导航

`breadcrumbs(text, caret)` 使用官方 syntax CST 返回 caret 前的标题祖先，以及包围 caret 的 Equation / Raw。
每项为源码标签或 math/raw 类别及 UTF-8 byte 起点；按标题深度替换同级与后代，不将注释或 raw/code 内的标题状文本当作文档标题。
该入口只解析，不求值、不读取资源、不改变源码，供 app 的光标面包屑与点击跳转使用。

## PDF 输出

同一 pin 的未修改 typst-pdf 与完整 fork 版本组共同链接，并受 verify-typst-fork.py 的源文件清单校验。
`export_generated_pdf` 使用与原生 A4 预览相同的 PreviewWorld、生成源码和字体策略；
RawMath 与未填槽先拒绝，不把占位图形当正式输出，不求值用户数学文字。
`export_source_pdf` 只在 OS 隔离 worker 中使用，与 compile_source_snapshot 共用显式资源 World。
两者返回完整 PDF bytes、页数、真实诊断，失败 bytes=None；PDF 专用病态输出上限为 100,000 页、2 GiB，独立于预览预算；沙箱 /work 配额仍约束构建。
PDF 选项采用 fork 默认值，不注入系统时间；PDF 编码错误也经 map_diagnostics 映射。

常规测试用相同 fixture 分别调用上述 API 与直接 typst::compile + typst_pdf::pdf，
经 hayro 在固定 300 dpi 比较页数、尺寸及全部 RGBA，容差为零。元数据差异不参与判断；
不得通过放宽容差接受以后升级的排版差异。另测试跨文件 source snapshot 与损坏语法/未填槽拒绝。
最终文件发布和共同报告属于 app，见 [ADR 0056](../adr/ADR-0056-pdf-export.md)。
