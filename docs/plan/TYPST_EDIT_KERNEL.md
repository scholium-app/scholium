# Typst 编辑内核改造计划

目标是用同一套 Typst 算法完成输入时的排版与后续页面排版：字符、分数、空槽、
光标和点击几何共同更新，取消“egui 先画文字，Typst 随后替换”的交接。
方向由用户于 2026-09-30 指定；边界提案见
[ADR 0032](../adr/ADR-0032-typst-edit-kernel.md)。
本页定义将来的实现与验收，不表示内核已完成。入口的限定实验证据见
[报告 0049](../spikes/SPK-0049-typst-content-layout.md)。
主程序接入的实施批次、模型迁移和默认替换条件见
[主程序接入计划](TYPST_MAIN_APP_INTEGRATION.md)。

## 1. 修改哪一层

保留 Typst 的字体选择、shaping、数学字体 MATH 常量、公式布局、段落断行和分页算法。
增加结构输入、编辑身份、布局几何和可恢复的增量会话；不另外写近似数学排版器。

```text
语义操作 → LocalSession 接受 → 带稳定身份的 Content 增量
                                    ↓
                      Typst EditorLayoutSession
                       realization / math / inline / flow
                                    ↓
                   SceneDelta { Frame, EditGeometry, placements }
                                    ↓
                        原生页面绘制、光标、选区、IME
```

生成的 `.typ` 仍用于 source lens、reconcile 和标准源码输出；原生结构输入不再必须绕过
“生成全文源码 → eval → 全文档布局”才得到反馈。真正的 Typst 源码项目继续经过 parser/eval，
增量能力按实际依赖决定，不能因此宣称任意程序都可以逐键局部求值。

## 2. 源码依据与可复用入口

研究基线固定 Typst **0.15.1**，实际读取本机 registry 的对应源码；升级另做兼容验证。
以下路径相对于上游 `crates/`，对应同版本的源码位置，不引用浮动 main 作为实现依据。

| 部位 | 已有实现 | 改造方向 |
|---|---|---|
| `typst/src/lib.rs::compile_impl` | eval 后创建文档，重复布局直到 introspection 稳定，最多五轮 | 保留完整编译；编辑路径接收 Content 与明确的依赖上下文 |
| `typst-layout/src/lib.rs`、`flow/mod.rs` | 对外暴露 `layout_frame` / `layout_fragment`；内部 fragment memoize | 作为结构输入切入点，封装有生命周期的编辑布局会话 |
| `typst-layout/src/inline/mod.rs::layout_par_impl` | 段落 memoize；realize → collect → prepare → linebreak → finalize | 保留段落算法，输出行、cluster 与编辑端点几何 |
| `typst-library/src/math/ir/` | `MathItem`、`MathProperties`、`resolve_equation` | 传播编辑 origin，表达 Hole；IR 借用 arenas/styles，不可当持久化树 |
| `typst-layout/src/math/` | 分数、根式、上下标、定界符、矩阵等 | 在真实布局位置输出槽位、节点框和装饰归属 |
| `typst-library/src/text/item.rs` | Glyph range、span、advance、offset | 保留 cluster 关系，补字素与视觉方向上的 caret stops |
| `typst-library/src/foundations/content/raw.rs` | Content hash 包含 element、metadata、span | 新 origin 必须参与缓存正确性设计，避免身份串用 |
| `typst-layout/src/flow/{mod,collect,compose}.rs`、`pages/` | 工作状态借用本次收集出的 children；按 region 顺序合成 | 改成可保存的准备结果及重排检查点，不能把临时 Work 跨请求保存 |

Typst 已有增量缓存；调用完整 compile 不等于每项计算都从零开始。
需要改的是每次修改的工作边界、身份与输出合约，不能只在 memoize 外再套一层缓存。

## 3. 第一批内核补丁：身份和空槽

### 编辑来源

在隔离 fork 中添加 feature-gated 编辑 origin，例如 opaque `EditNodeId`、`EditSlotId`、
叶子 UTF-8 范围和 affinity。Typst 不依赖 scholium-model；adapter 负责把 opaque ID
映射为本项目 TreeAnchor。ID 不是源码偏移、节点序号或 Typst introspection Location。

origin 随 Content → realization → MathItem / inline segments → Frame 传播：

- 文本保存 origin 与叶子范围；行内拼接与切分保留映射，不按 glyph 序号猜字符。
- fraction bar、radical、delimiter 等保存拥有者 origin，点击可以选择结构。
- show rule 展开可能是一对多或 derived。仅在能保留来源时允许原位编辑；不能给所有
  派生字形猜一个可写文本位置，也不能为了保留 origin 而阻止正常 show 变换。
- 缓存命中必须连同对应 origin/geometry 一起返回。禁止外部全局回调记录几何，因为
  memoize 命中会跳过函数体。先让带身份产物参与缓存 key，再根据实测分离纯几何缓存
  与身份重绑定；两者都要通过“相同文字、不同节点 ID”的负对照。

### 空槽与未完成输入

原生数学使用带稳定 ID 的 Row、Symbol、Fraction、Root、Script、Delimited 和 Hole。
对应模型与 document 设计已有方向，需新增版本化迁移，不能把现有 Math(String)
静默解析成猜测的数学树。旧公式保留原串，按明确 capability 转换或作为 RawMath。

第一版将 `Fraction(x, Hole)` 映射到 Typst 原生分数 + editor-only Hole 内容，
由数学布局按当前 script style 和字体度量决定槽位的宽、高、基线与交互矩形。
空槽不是往源码里插入可见方框字符；提示样式由 UI 根据几何绘制。

必须区分两类输入：

- 结构命令插入的分数具有分子/分母槽位，从创建起就是可排版结构。
- 命令拼写、IME preedit 或 raw source 的未完成 token 保留输入身份和诊断；
  不把 `al` 自动认作 alpha。真正的结构公式不整体退化成源码文字。

编辑模式允许 Hole；严格输出发现必填 Hole 就失败并定位节点。编辑提示与 preedit
不写入生成源码、保存的正文或 PDF。合法的空内容仍须与必填 Hole 区分。
Hole 本身是可保存恢复的语义结构，不能因为隐藏提示图元而在保存时丢掉空槽身份。

## 4. 第二批内核补丁：独立且完整的交互几何

新增 `EditGeometry`，与 Frame 在一个布局产物内返回，至少含：

```text
NodeBounds { origin, parent, local_bounds, transform }
SlotBounds { owner, slot, baseline, local_bounds }
TextCluster { leaf, utf8_range, bidi_level, advance_bounds }
CaretStop { structural_position, affinity, line, rect }
Decoration { owner, bounds }
```

这是接口草图，不是已经公开的产品 API。坐标单位固定为局部 pt，页面 placement
由同一个场景的变换应用；文本偏移固定为叶内 UTF-8 byte range，字素导航另有索引。

在 `inline/shaping`、`inline/finalize` 和 math 组合 frame 时生成几何，不能仅用最终墨迹框
补救：空格、空行、合字内部位置和空槽都可能没有单独的墨迹。
合字 caret 优先采用字体可用数据，缺失时定义可复现且经实机验证的插入点策略；
不把等宽插值叫作精确字体几何。还需覆盖 RTL 的视觉方向与文本逻辑方向。

同一次布局产生的 Frame、文字 cluster、行几何、选区和 IME 锚点共同采纳。
数学空槽、结构端点与段落空行均可命中；无法归属的 derived 内容提供结构选择或源码跳转。

## 5. 第三批内核补丁：持久会话与增量重排

### Content 与依赖

adapter 保存按节点局部版本索引的不可变 Content，编辑只重建叶子与必要祖先。
缓存 key 包含内容、实际 StyleChain、可用宽高、字体/资源世代、布局 profile 和相关
introspection 约束；不能仅按 NodeId 或整篇 revision 判断可复用性。

正文改字先重排受影响段落。公式子树的独立缓存另做测量；初版允许重排整条公式，
不能宣称已经有逐子式增量。字体、模板或宽度变化应使相应段落失效。

### Flow 与分页

在 Typst flow/pages 内部增加拥有其数据的 prepared items 和分页检查点。
先支持正文段落与行内数学，保存每页入口的 region、carry、段落延续、样式、编号和相关
上下文 fingerprint。修改后从最早受影响检查点重放，直到输出和后续入口状态均一致。

“高度没变”不足以证明分页可复用：文字/链接/几何、widow/orphan、页码、脚注、浮动体
及计数状态都可能改变。新片段的图像和几何即使外框相同，也必须替换。
不能按段各自冷编译后在 UI 拼接，否则会失去 Typst 的上下文与分页语义。

全局 `query` / `state` / `counter`、show/context 与页面信息可能跨越局部边界。
先保守记录依赖并使相关范围失效；遇到未知动态依赖升级为完整求值/重排，保留真实诊断。
静态原生子集是第一轮性能门禁；复杂任意程序不纳入同样的逐键时延承诺。

### 会话协议与调度

```text
EditorLayoutSession.apply(accepted_revision, structural_delta)
EditorLayoutSession.layout(viewport, resource_generation, work_budget)
    -> SceneDelta {
         base_scene, scene_revision, node_versions,
         frames, placements, geometry, resources, diagnostics, pending_regions
       }
```

几何 delta 与绘制资源 delta 同时验证和提交；base 不匹配请求完整场景。
未变块可复用已经验证局部版本和依赖 fingerprint 的产物，在新场景内更新 placement，
不是把旧 revision 的几何拿当前源码重新解释。

所有已接受结构修改按顺序进入会话，丢弃中间显示结果不能丢掉修改或仍被引用的资源。
工作预算耗尽时在 Typst 内部检查点让出，优先活跃位置和可见受影响区；不能在 UI
绘制近似正文来冒充新布局。尚未完成布局的区域明确 pending，旧图只代表旧内容，
禁止用旧几何定位新字。后台完成的是同一会话的后续布局，不切换另一套文字/数学算法。

原生静态内容可沿现有受信常驻 worker 边界实验；任意项目代码、用户宏与外部资源必须
经过既有沙箱入口。结构 Content 也可能触发 show rule/上下文执行，不能把直接布局
本身当成安全证明。UI 线程不运行 Typst 求值与布局，也不等待 worker join。

## 6. 分步交付与门禁

| 步骤 | 范围 | 必须验收 |
|---|---|---|
| K0 入口探针 | 未修改上游的公开 Content 布局接口 | 不读源码、分母空/填入均布局、指纹与重复布局正确；只证明入口 |
| K1 origin 与 Hole | 隔离 fork，少量 library/layout 补丁，结构 adapter 夹具 | 分子/分母空槽可命中；装饰归属；跨 edit 缓存没有节点身份串用；完整公式与原 Typst 同字体参考图比较 |
| K2 统一几何 | 文本 cluster、行、caret、数学节点/槽位 | 中英混排、长行、空段、合字、组合字符、RTL、缩放/变换、IME 位置；几何缓存命中与真实计算一致 |
| K3 会话与分页 | 增量 Content、flow 检查点、依赖失效 | 1/64/256 段首中尾修改；跨页增删、换宽/换字体、脚注/计数/全局 query 对照；与完整编译的支持子集结果一致 |
| K4 应用集成 | worker SceneDelta → 页面；结构数学模型版本化迁移 | 保存重开/撤销/选区回归；当前画面命中当前位置；编译期间连续输入；无源码回显交接 |

K1/K2 应在 `spikes/typst-edit-session` 的独立 fork/fixture 中验证，不能先替换产品后补证据。
只修改确有需要的 Typst crate；以固定 0.15.1 基线和可审阅 patch series 管理，不修改全局
Cargo registry。产品接入时统一同版本组的 Cargo patch，避免多个不同来源的 Typst 类型。
fork 的完整源码、上游许可、补丁与版本需可复现；不把整套 fork 作为无法回顾的大提交。

性能目标在 K3/K4 动手前固定：暖态活跃段落布局 + 当前几何 p95 ≤ 8 ms；
受控 60 Hz 原生窗口“事件接受 → 当前画面与可操作几何呈现”p95 ≤ 32 ms。
这些是拟验收目标，不能用 K0 的约 1 ms 后端数据声称已达标。
连续输入按 10 次事件/秒至少 30 秒测量；局部输入耗时应与全文段数基本解耦。

测试分别报告输入确认、局部求值/布局、后续分页、栅格化、纹理上传和屏幕呈现。
release、debug、冷字体、超长单段与复杂公式单列；同一字体和内容比较 stock 全量与 fork
增量路径，缓存命中率、失效范围、内存和排队长度均需记录。

K4 移除页面编辑的 `echo_blocks` 和依赖拼接 markup 偏移的交互几何桥，恢复现有撤销与
本地会话契约。保留独立模式的源码预览/最终构建，不在输入后自动替换编辑画面。
回退以功能开关或独立提交切回旧后端；未过门禁不默认开启，也不删除原始源码与输出路径。

K4 的开发候选可在结构身份/迁移和必要同源几何通过后，进入主程序的显式验证入口，与
K3 分页/性能改造交替验证；该入口复用真正的 session/storage，不另起可写模型。
候选仅处理报告声明的支持范围和隔离测试会话，不改变默认发行行为；默认替换仍需完整
K1–K4、兼容及安全门禁。每批报告区分探针、主程序候选和普通启动的交付范围。

## 7. 内核起步验证与主程序实施入口

内核路线从 K1 起步：固定上游 fork，在 Content/math IR 上贯穿 origin，并在 math resolver/layout 中
加入 editor-only Hole，返回分数两个槽位与拥有者几何。用“同一公式文本、不同稳定 ID”
和“修改分母后复用分子”的夹具检查缓存身份；以 stock Typst 完整公式图作为排版对照。
这一任务能直接验证输入、结构排版和几何是否同源，再进入段落与分页的更大改造。
主程序的实施批次按[接入计划](TYPST_MAIN_APP_INTEGRATION.md) A–E 推进，已有探针结论按报告
的支持范围复用；每批以主程序实际行为与对应兼容/性能证据验收。
