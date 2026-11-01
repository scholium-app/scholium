# scholium-app

## 职责

提供桌面壳、页面式结构编辑器、Source Studio、焦点/模式工具栏、项目引擎预览、项目树、大纲、引用面板、
历史时间线、冲突解决、协作 presence、命令系统、设置和 core session 的 UI 适配。

## 非职责

不直接写项目文件，不实现 CRDT/解析/历史算法，不拼接构建命令，不在前端保存永久 token。

## 内部结构

```text
app-shell
├── native_ui/      原生 editor、panels、preview、state projections
├── typst_editor/   EditorCore 适配、事件翻译、纹理/绘制与平台交互
├── bridge/         Rust 消息通道、平台坐标转换、可选原生 FFI
└── platform/       window、clipboard、file dialog、keychain、updates
```

桌面框架严格按 Iced → GPUI → C++ EUI-NEO → Slint 或 egui 依次验证，已选定 egui / eframe 0.36.2（ADR 0006）。
窗口、输入法、文本塑形、布局、GPU/软件绘制与可访问树均采用原生接口；不使用浏览器编辑器或 WebView。
Rust 负责权威文档和业务；EUI-NEO 候选由 C++ 实现原生 UI，通过受控 ABI 连接 Rust。必要 C/C++/Zig 依赖单独审查。
原生框架提供基础控件不代表现成支持数学结构编辑。结构视图与源码视图分别验证编辑适配、IME、选区、撤销、
大文件、协作及无障碍；不得直接把静态排版图片当可编辑控件。详细标准见 [原生 UI 验证计划](../plan/NATIVE_UI_VALIDATION.md)。

## 应用壳与主题边界

应用壳位于 `crates/scholium-app`，窗口外壳由上到下是菜单栏 28、主工具栏 34、焦点工具栏 30、文档标签栏 30、
编辑区、状态栏 24（逻辑像素，高度来自令牌；应用不自绘窗口标题与窗口按钮，`with_decorations(true)`）：

- `chrome/` 排布各栏：`menubar` 由 `registry::menu_items` / `submenus` 生成文件、编辑、插入、格式、文档、视图、工具、帮助
  及子菜单；`toolbar` 按 `ToolbarPlacement` 分组画图标按钮并以竖分隔线隔开，段落样式组合成一个“¶ ▾”下拉，
  命令搜索固定在最右（暂时禁用，原因写在悬停提示里），放不下的整组按注册表顺序收进“»”溢出列表
  （菜单样式列表，图标 + 标题 + 快捷键），菜单栏、模式切换与命令搜索始终可见；`focusbar` 在正文中显示文档属性
  （样式、纸张、文档语言、字体、字号，取值读自 `scholium_typst` 的样式常量，是禁用的文字下拉框，悬停说明尚未开放编辑），
  在公式中显示“公式”标记与 `FocusKind::Math` 的命令（分数、根式、上标、下标、定界符、矩阵、离开公式），
  最右固定是“页面编辑 / 源码预览”分段切换（Ctrl+Shift+M，左右方向键切换）；`tabbar` 为当前文档显示一个标签
  （显式选择的文件名或“未命名”、未保存圆点、关闭按钮走既有的关闭与丢弃确认流程）和“+”（既有的新建文档流程），
  单文档应用，布局已按标签列表排；`statusbar` 见下。`layout` 是不含 egui 的纯宽度决策（溢出分组、焦点栏字段收窄），
  单元测试覆盖。
- `widgets/` 是“组件规格”的实现：图标按钮、文字下拉框、分隔线、标签页、模式分段切换、状态栏片段与警告、
  带右对齐快捷键与勾选的菜单项、“名称 + 快捷键”提示；全部状态（默认、悬停、按下、激活、禁用、键盘焦点环 2px 外扩 1px）
  由令牌绘制，焦点环只在键盘操作后出现。菜单宽度按最长一行测量，标签不写“（未实现）”；分段切换每段按文字测量且不小于 96。
  每个控件用 catalog 标题作无障碍名称，只有带 `checked` 的命令才暴露为开关。
- `icons` 把 Lucide 图标（ISC，许可证在 `assets/icons/LICENSE`）以内嵌 SVG 经 resvg 按设备像素比光栅化并用令牌着色，
  纹理按（图标、尺寸、线宽、颜色、ppp）缓存；工具栏 18/1.5、菜单与溢出 14/1.25、状态栏与标签 12/1.25、箭头 8/1.25。
  `IconId` 来自注册表；不引入图标字体或新的光栅库。
- `typst_editor` 是唯一编辑面（持有 session 的 EditorCore、Typst Content 场景、事件翻译、导航与迁移提示），`state` 只持有视图偏好
  （工作方式、面板、缩放、主题、语言、工具栏显示）与编辑器元数据，`command` 是命令注册表，`commands` 保留视图命令的处理函数。
  未接入的写操作保持禁用，不以本地变量伪造保存、构建或 revision 状态。

剪贴板命令经原生 viewport 请求发送 Copy/Cut/Paste 并先恢复页面焦点（`typst_editor/clipboard.rs`）；文字样式、段落样式、
公式与分数作为结构 intent 排入编辑会话，与键盘输入同序执行。结构视图禁用正文写命令；组合输入期间禁用相关写入。
Ctrl+F1 与“视图 → 显示工具栏”调用同一命令，同时显示或隐藏主工具栏与焦点工具栏。

`theme::Tokens` 是仅有的颜色来源：P1_UI 令牌表的全部语义色（浅色、深色两套 HEX）、栏高、控件高、字号与行高、间距刻度、
圆角与图标尺寸；控件一律读令牌，`theme` 之外没有颜色字面量（测试检查）。浅色是默认主题，“视图 → 主题”在运行时切换浅色/深色
（偏好暂不持久化）；纸面在两套主题中都是白色，画在纸面上的光标、选区与公式底色因此使用浅色主题的强调色与 `math_bg`。
配色变更不修改布局、不影响文档内容。界面字体在启动时从系统字体库（fontdb，与 Typst 内核扫描同一来源）按序选择：
HarmonyOS Sans SC → MiSans → 苹方 → 微软雅黑 UI → 微软雅黑；西文优先 Inter，等宽优先 JetBrains Mono；随程序打包的
Noto Sans CJK 只作最后回退。所选字体写入 AUDIT 的 `fonts`。
Typst 编辑页与源码预览目前另会按正文的 Times New Roman/SimSun 字体族请求系统字体，并扫描宿主字体库；
因此同一文档可能因机器安装的字体及字宽不同而采用不同断行。场景与窗口验收固定使用应用打包字体，
这项测试隔离不改变普通产品启动策略。

## 诊断

`diagnostics/` 持有诊断列表的数据模型与面板，不持有文档内容。一条诊断是 `{ 严重度 (Error | Warning | Info)、catalog key 与参数、
可选位置、可选段落摘录 }`；文字在绘制时按界面语言经 Fluent catalog 渲染，所以切换语言立即生效。位置是文本/数学叶的
稳定 `NodeId` 加字节偏移，不含屏幕坐标。列表按严重度排序（同级保持收集顺序）：错误（编辑、保存、排版失败，无位置）→ 警告（被拒绝输入、
选区几何、公式超出行宽、缺少光标几何；超出与缺光标带位置）→ 提示（启动迁移说明）。session 提供布局错误、被拒输入、段落文字与溢出目标，`typst_editor/diagnose.rs` 负责呈现列表：
公式超出在采纳 Scene 时按快照一次算出（几何只在 Scene 当前时可信），其余随元数据逐帧收集；`EditorView.problems` 把结果交给状态栏与面板。

跳转：诊断（或面板行）写 `WorkspaceState.jump_request`，编辑会话在无排队输入时按顺序执行：把光标移到目标叶的字节处、清除选区/拖动/输入样式、
请求页面焦点与跟随光标。目标叶已不存在（文字已被改掉）时忽略。点击页面矩形之外的按下事件（页面纸面高于视口，状态栏和面板落在其矩形内）
不进入页面的指针处理，因此点击状态栏或面板不会移动光标。

诊断面板由“视图 → 诊断”（Ctrl+J）或状态栏的“共 N 条”开关（`WorkspaceState.diagnostics`），停靠在编辑区底部、状态栏之上，
默认高 160 逻辑像素，顶边分隔条可拖动调整，不覆盖任何栏。左为“问题”列表（严重度图标、消息、所在段落摘录，点击或 Enter 跳转，空时显示“没有问题”），
右为“排版”（后端、正文 r、画面 r、待更新标记与上一次布局+栅格的统计；窗口窄于 640 时在列表之下）。面板打开且有问题时键盘焦点落在第一行；
上下键在行间移动，Esc 把焦点还给页面；Enter/空格激活后按键被控件消费，不会再被页面当作输入。行是普通控件，
无障碍名称为“严重度：消息”。AUDIT 的 `diagnostics` 列出诊断及位置，`controls` 里 `ui.diagnostics.row` 按序给出各行矩形。

## Session 的 UI 适配

### Typst Content 编辑入口

启动即打开 Typst Content 会话（[ADR 0036](../adr/ADR-0036-typst-default-editor.md)、
[ADR 0037](../adr/ADR-0037-remove-legacy-editor.md)）；旧回显编辑器已删除，没有回退路径。未设 `SCHOLIUM_SESSION_FILE`
时文档为 XDG 数据目录 `scholium/document.v1.sqlite`；它不存在而同目录旧版会话有已保存 head 时
一次性迁移到新文件，旧文件只读、保持原样，页头显示迁移提示与转为原串的公式数。已有 v1
文件不被迁移覆盖；显式 `SCHOLIUM_SESSION_FILE` 必须是 v1。没有可写副本对拷。旧文件只读探测；
迁移或打开失败时窗口只显示原因与恢复方式，不运行编辑器。迁移只发生一次，此后不再读取旧文件。

编辑器持有 [session](session.md) 的 `EditorCore`，将 egui 文字、可迁移按键、页面坐标和视口转换为 `Input`，由核心
按事件顺序经唯一输入入口提交文字与命令；布局仅持有
完整不可变投影。输入接受和提交先于页面绘制，后台完成唤醒 UI，不去抖源码求值。界面
工具栏与页面之间没有任何信息行。状态栏左侧是字数（正文里每个 CJK 字符与每个拉丁词各计一，随快照修订更新）、
页码（只在源码模式且预览已编译时显示，连续的页面编辑视图不显示）与上下文（正文 / 公式 / 源码 · 只读）；
中间是当前最严重的一条诊断（可截断，悬停看全文），多于一条时紧随其后显示“共 N 条”（英文 “N issues”）；右侧是缩放（缩小、百分比、放大、适合宽度）。
点击或按 Enter：诊断有位置时把光标移到该位置并聚焦页面，由跟随光标滚动到可见；没有位置时打开诊断面板；“共 N 条”开关面板（见“诊断”）。
左侧上下文之后是常驻的紧凑后端标签（“Typst 预览”；排版画面落后于正文时改为“更新中”，两种文字同宽，不造成布局跳动），
悬停提示给出后端、正文/画面 revision 与待更新标记；布局统计在诊断面板里，不再占状态栏；
保存状态由标签上的未保存圆点与窗口标题表示。
旧图停用命中与光标绘制，不回显已提交文字。页面在视口内水平居中，视口更窄时从左边距起水平滚动。
快捷键提示在“行内公式”“分数”按钮的悬停提示中；文字组的“清除格式”恢复纯文本样式，“正文”只指段落样式。
纹理、caret、命中与 IME 共用一套 Frame-pt 到窗口逻辑像素 placement。
页面栅格分辨率跟随显示：页宽量化为整数物理像素（`typst_editor/raster.rs`），页面原点吸附到物理像素网格，
placement 的 scale 取量化后的值，故命中/caret 与绘制一致。页面是两层纹理（`raster/layers.rs`）：整页底图
（≤ 1 px/pt，拉伸、LINEAR）与其上 1:1 的显示分辨率条带（512 行，只覆盖可见范围，边吸附后相接无缝）。
应用把可见范围（pt）和 RasterSpec 翻译为 `Input::Viewport`；session core 持有 worker/Scene，以约 40 ms 节流请求滚动后的条带；
条带按 Scene stamp 归属，采纳新 Scene 即整体替换，旧 Scene 的条带不会画在新底图上；窗口外（三个视口）的条带纹理被释放。
缩放、窗口拖拽或缩放因子变化只改目标分辨率：旧条带拉伸显示，目标稳定约 120 ms 后 worker 以新倍率补渲染可见条带，
新倍率的条带覆盖可见范围后才替换旧集合；键入提交的排版直接按当前目标倍率渲染底图和可见条带，不等待。
源码模式 A4 预览按其显示物理宽请求页面栅格，规则相同（A4 页面恒小于单张纹理上限，不分条带）。
测试专用环境变量 `SCHOLIUM_RASTER_PX_PER_PT` 强制旧的固定栅格倍率，仅供窗口检查对比锐度。
页面持有焦点期间每帧都发布 IME 区域：旧图或当前叶无 caret 几何时沿用最近一次已证 caret
位置，因为 egui-winit 在缺少 IME 输出的帧关闭平台输入法，反复开关会让 fcitx5 停止送字。
同帧 Text 仅在与 IME Commit 文字相同时视为重复丢弃；输入法透传的英文 Text 照常输入。
Escape 分层取消，不释放页面焦点：先取消选区、IME 组合与拖动；三者都没有且光标在行内公式内时，离开该公式，
光标落在公式之后的正文（与 Ctrl+M 离开同位；空占位槽由占位生命周期移除）；公式外无事可做。
视图跟随光标由 session core 返回 page-pt 矩形意图，app 的 `typst_editor/follow.rs` 只负责将其转换为滚动区动作：键入/粘贴/提交、Enter、删除、方向键、Home/End、撤销重做与移动光标的工具栏命令
引起光标移动后，滚动区最小幅度滚动，使光标矩形上下（放宽时左右）各留 2 行正文（`FOLLOW_MARGIN_LINES`）；
Ctrl+Home/End 另外露出页面顶部（含外边距）与底部。只有输入引起的光标变化才触发；滚轮、滚动条与点击不触发，
指针按下或滚轮会丢弃尚未执行的跟随。Scene 不是当前版本时保持“待跟随”，等新 Scene 采纳后用其 caret 几何滚动，
绝不对准旧位置；滚动改变的可见范围照常经 `set_view` 请求新区域条带。AUDIT `follow` 报告 caret 与可见范围（pt）。

工具栏、菜单、Typed/Paste/IME Commit、删除/拆段与 undo/redo 都翻译为 `Input` 后经 `EditorCore::input` 执行；
app 保留组合输入、只读模式和当前选区几何的前置检查，按 Outcome 清除拖选、提交新 Scene 或刷新元数据。
唯一 LocalSession、store/保存基线、撤销分组与时钟、输入样式和占位生命周期属于 session（[规则](session.md)）。
app 不直接应用 StructuralRequest，也不持有第二份可写文档。确认框决定是否丢弃，然后发送 New 命令。

编辑面是连续的 A4 版心纸面（宽 210 mm、页边距同 A4 预览、不分页），字体、字号、行距、缩进与标题
样式与预览同源（见 [typst.md](typst.md)）。

源码工作区（Ctrl+Shift+M 切换）左侧是 `generate_source` 的只读 Typst 文本，光标所在块高亮；右侧是同一快照的
A4 快速预览，首次进入才启动预览 worker，快照变化即以新 generation 提交、只显示当前 generation 的页，
编译后翻到光标块所在页；点击预览把光标移到该块开头。空槽数与原串公式数在源码上方说明。
这是只读派生视图：不 reconcile、不写回，也不代替最终 LaTeX/Typst 构建。

左右方向键与 Tab 的事件翻译、逻辑导航仍在 app；语义光标与 anchor 存在 core 中。
app 的导航、点击、Esc 与选区变化解除待输入样式，并在导航后通过命令结算 Attach 占位；
占位创建、撤销及字素重锚由 session 执行。数学朗读为 `上下标（底数：…；下标：…；上标：…）`，
槽路径名为 底数/下标/上标。

上下方向键与 Home/End 只读当前 Scene 的精确正文 caret（padded Frame pt）推出视觉行：
行内公式的槽位与正文行纵向重叠，不参与划行。上下键取纵向最近的相邻行、再取最接近目标
x 的停靠点；连续上下在光标和期望 Scene 都未变时保留目标 x。软换行同一字节的 Upstream
停靠点属于上一行末、Downstream 属于下一行首，等 x 时取逻辑较后者。无相邻行时停在本行首/末。
数学槽内上下先在最近的分子/分母间切换，否则以整公式 bounds 代表当前行离开公式；
槽内 Home/End 保持槽内首末。Ctrl+Home/End 按逻辑顺序到正文首末，不需要几何。
排版未就绪时几何键及其后的输入按到达顺序排队（上限 256，超限文字保留可复制草稿；
IME 与 Escape 不受上限丢弃），工具栏/菜单/撤销/保存也等队列清空，关闭时排队输入按未保存处理；
排队期间的鼠标按下被拒绝，布局失败只放弃排队的几何键。目标 x 只在连续上下间保留。
视觉 bidi 不从逻辑顺序猜几何；仅有公式而无正文 caret 的行不作为上下停靠行。

选区用正文 Text 叶的稳定身份、UTF-8 字素边界保存 anchor/focus。Shift 左右键按正文
逻辑顺序跨叶/块，完整覆盖的结构公式作为对象选中；Shift 上下/Home/End 按视觉行扩展。
普通左右键折叠已有选区。鼠标按输入事件顺序命中同一当前 Scene 的精确 caret；拖选绑定
core 的不透明 Scene generation 和 placement，布局请求或页面位置/缩放改变即停止旧拖选。
比例合字内部 caret 不作鼠标端点；未知身份和 pending Scene 不能恢复命中能力。
app 按时间和屏幕像素距离给原生事件标记点击数；双击选词、三击选段只经 core 的抽象指针入口执行，
不根据绘制 Response 再发送第二次选择。完成抬起时同样交付点击数。

页面点击的命中规则：点在某个行内公式的 bounds 内才进入公式（取最近精确 caret）；其余位置只在正文
精确 caret 上定位——按与上下键相同的视觉行划分，取与点纵向距离最小的行，再取该行 x 最近的停靠点
（行右侧空白到行末）；第一行之上、最后一行之下分别取与 Ctrl+Home/End 相同的逻辑位置（不依赖几何，文档以公式结尾时是公式后的空正文叶）；
点在以公式结尾的行右侧时取公式之后的正文位置，而不是公式之前的停靠点。整公式 bounds 是公式内所有节点（原子、分数线、Hole）bounds 的并集——Row 与 Attach 自身没有 Frame bounds。
双击选中指针下的 UAX #29 词段（空白、标点选该段），三击选中该块全部正文 Text，均写入既有
anchor/cursor 选区，复制、剪切、输入替换沿用同一路径。点击次数取 egui 的 `double_clicked`/`triple_clicked`
与会话自记的“位置相近（≤6 px）且间隔 <300 ms 的连续按下数”中较小者（egui 只比时间），在释放时生效
（按下时已按单击放好光标）；数学槽内双击保持光标并提示“数学槽位选区尚未接入”。

光标在行内公式内时，页面在公式整体 bounds 后画淡色圆角矩形、在当前 Fraction/Attach 槽外画更淡的描边
（均为形状，不画文字，几何取自当前 Scene），状态栏显示“公式”。Esc 在无选区、无组合时离开公式；末尾右键/开头左键同样可以离开。
公式内 Enter 的语义拆段由 session 执行。无运算符长串的溢出由 session 按当前 Scene 推导目标，
app 显示每个公式的“公式超出行宽”诊断，不改写内容。AUDIT 的 `formula_overflow` 只表示光标所在公式。

选框来自同一 Frame 的 cluster stops 和数学根 bounds，绘制非打印多边形，不画已提交
文字。缺几何的当前选区拒绝替换；pending 时可按有效语义端点继续键盘编辑，禁止使用旧
几何判断位置。范围内 RawMath 明确拒绝，失败保留正文、端点和可复制输入草稿。
范围替换、存活叶/字素位置、撤销端点与纯文本复制/剪切的拒绝规则由 [session](session.md) 定义；
app 只检查当前选区几何和交付原生剪贴板。数学内部槽位选区与选区结构命令尚未接入。
见 [ADR 0035](../adr/ADR-0035-typst-page-selection.md)。
与其他行内内容相邻的空 Text 叶在 Typst 中不成 cluster、没有可绘范围；选框跳过它，
语义范围仍包含它。视觉行导航见[开发日志《Typst 主程序候选的视觉行导航与几何键有序排队》](../log/2026-10-10-typst-vertical-navigation.md)。

worker、wanted SceneStamp、已采纳 Scene、viewport/raster 请求策略和 deferred-input 语义均由 session core 持有。
app 只保留匹配的 egui 事件队列：事件分类后交 core 决定执行、入队、等待或拒绝；不自行判断几何等待、上限或布局失败。
core 交付已采纳 Scene 的位图与已验证身份的条带，app 上传为纹理；几何、统计与 AUDIT identity 只读取 core 投影。
源码视图的 A4 派生预览及其纹理是只读展示适配，不承担正文编辑输入。
见 [ADR 0042](../adr/ADR-0042-session-editing-core.md)。

## UI 状态

前端只保存投影：当前视觉树、source buffer mirror、selection、focus、viewport、panel layout 和 pending request。
正文权威状态在 core；前端对输入做乐观显示，core 拒绝 base revision 时返回 semantic/text rebase delta。

## 命令系统

所有用户动作注册为 command（`crates/scholium-app/src/command/`）。菜单、主工具栏、焦点工具栏、溢出列表、状态栏按钮、
快捷键（以及之后的命令搜索）读同一份注册表，不得各自维护命令、标题或快捷键。

- **数据模型不含 egui 类型**（以后可整体移入与界面无关的编辑核心）：`spec.rs` 定义 `CommandSpec`，
  `registry.rs` 汇总，`file.rs`、`edit.rs`、`insert.rs`、`format.rs`、`document.rs`、`view.rs`、`tools.rs`、
  `help.rs` 各出一个 `specs()`；新增命令只改所属领域文件，新增领域只在 `registry::DOMAINS` 加一行。
  `keys.rs`、`shortcuts.rs`、`ui.rs` 是界面边缘，负责 `KeyChord` 与 egui 按键互转、消费快捷键，
  以及把一条命令的标题、提示、可用性与勾选状态解析成控件描述（`ui::Resolved::meta`）；控件本身由 `widgets/` 绘制。
- **`CommandSpec`**：稳定点号 `id`（`format.bold`、`view.mode.source`）；`title`、`tooltip` 为目录 key
  （`command.<id>.title|tooltip`，由 `command!` 宏从 id 派生）；`icon`（`IconId`，无图像）；默认快捷键
  `KeyChord`；`routing`（`App` 由应用先消费，`Editor` 留在页面事件流里与键入保序执行，`Platform` 是剪贴板事件、
  只用于显示）；菜单位置（菜单、可选子菜单、组、序）、可选主工具栏位置与焦点工具栏类型（文本/数学）；
  `writes`；`availability: fn(&CommandContext) -> Availability`，取 `Enabled`、`Disabled(原因 key)` 或
  `Unimplemented(原因 key)`；`checked`；`action`（复用 `ViewCommand`、编辑器 `Command`、撤销/重做与剪贴板请求）。
  `CommandContext` 是纯事实结构（模式、是否有文档、组合输入、选区、能否撤销重做、粗体/强调、面板与缩放状态等），
  由 `WorkspaceState` 与 `EditorView` 构建。
- **菜单**：文件、编辑、插入、格式、文档、视图、工具、帮助，内容见 [P1 UI](../plan/P1_UI.md#菜单工具栏与焦点工具栏组织)；
  未交付的命令照样注册，可用性为 `Unimplemented`，菜单与按钮置灰，原因写在悬停提示里。
  `chrome/menubar` 按组插分隔线、按子菜单分层生成菜单树；“视图”含工作方式、导航与诊断、缩放、显示工具栏，
  以及“主题”（浅色/深色）与“语言”（跟随系统/简体中文/English）子菜单，后两者同样是注册表里的命令。
- **快捷键**：`shortcuts::shortcuts` 遍历注册表中 `App` 路由的和弦，带 Shift 的先于不带的；某键存在带 Shift 的孪生和弦时，
  不带 Shift 的和弦在 Shift 按住时让位（Ctrl+Shift+Z 重做不会同时触发撤销）。`Unimplemented` 的命令不消费按键。
  页面路由的和弦（Ctrl+B/I/M、Ctrl+/、Ctrl+0/1/2）由页面按注册表解析，字母、数字、功能键精确匹配，标点另可匹配
  不带 Shift 的和弦。测试断言没有两个命令共用一个和弦。
- **默认快捷键**：Ctrl+0 / Ctrl+1 / Ctrl+2 = 正文 / 标题 1 / 标题 2；Ctrl+Shift+M 在页面编辑与源码预览之间切换
  （Ctrl+1/2 不再切换工作方式；Ctrl+0 不再是实际大小，实际大小只在菜单与按钮里）；Ctrl+B/I 粗体/斜体、Ctrl+M 公式、
  Ctrl+/ 分数、Ctrl+Z/Ctrl+Shift+Z/Ctrl+Y、Ctrl+S、Ctrl+N、Ctrl+J、Ctrl+Shift+B、Ctrl+F1、Ctrl+加号/等号/减号缩放不变。
- **文字目录**：命令标题、提示、菜单名与不可用原因经 `i18n` 的 Fluent 目录（`i18n/zh-Hans.ftl`、`en.ftl`，
  见 [ADR 0040](../adr/ADR-0040-ui-string-catalog.md)）渲染；界面语言 `WorkspaceState.locale`（`auto` 按 `SCHOLIUM_LOCALE`、`LC_ALL`、
  `LC_MESSAGES`、`LANG` 的第一个非空值，否则英语；窗口检查按 `--locale` 在 zh-Hans 与 en 下各跑一遍，控件按命令 id 经 AUDIT 的 `controls` 映射点击，不依赖标签文字或坐标）可在“视图 → 语言”里运行时切换，下一帧生效，不影响文档与历史。窗口外壳的全部可见文字（菜单、工具栏提示、焦点栏、标签、状态栏、
  窗口标题、丢弃确认框）都走目录；源码窗格标题、预览状态和源码警告也使用带命名参数的目录 key。编辑面的状态消息与无障碍朗读仍列在
  `scripts/ui-strings-allowlist.txt`，由 `scripts/check-ui-strings.py` 保证只减不增。
- 源码窗格只按快照、高亮范围和实际文字/高亮颜色缓存布局输入；`egui::Galley` 由 egui 缓存，随字体图集重建而失效。
  主题切换更新文字颜色与图集坐标，稳定帧复用有效布局，不强制刷新帧，也不因主题变化滚动源码。
  已迁移的源码视图不能再加入文字白名单；门禁同时自检格式化和拼接模板的扫描。
- 写命令仍必须经过 session：页面命令作为 intent 排入编辑会话，与键盘输入同序执行；源码视图与组合输入期间由
  可用性判定统一禁用。窗口外壳（菜单栏、图标工具栏、溢出列表、焦点工具栏）全部从同一注册表生成。
- **窗口审计**：`SCHOLIUM_EDITOR_AUDIT=1` 的 AUDIT JSON 含 `controls`（命令 id 或 `ui.*` 外壳 id 到每个可见控件的窗口像素矩形，
  菜单展开时含菜单行，同一命令在多处出现则有多项）、`chrome`（语言、主题、溢出分组、状态栏事实）与 `fonts`；
  窗口检查据此按 id 点击，`spikes/typst-main-editor/chrome_ui.py` 封装点击、悬停与等待矩形稳定。

## 主要界面

应用壳遵守[体验规范的工作区原则](../EXPERIENCE.md#工作区组织)，实现时参照
[P1 排版工作区规划与参考图](../plan/P1_UI.md)：菜单栏、主工具栏、焦点工具栏与文档标签栏；页面编辑采用单一编辑面，
源码与预览默认约等宽、辅助面板按需展开。下面的能力列表不表示所有面板应同时常驻；
字体、尺寸、分组和平台控件可以按实际可用性调整，不做像素复刻。

- Workspace：文件树、搜索、依赖与资源状态。
- Page Editor：即时排版树、文本/数学模式、槽位、焦点工具栏、远端光标。
- Source Studio：LaTeX/Typst/Markdown/BibTeX 源码、生成状态、诊断和 reconcile preview。
- Preview：项目语言对应引擎的预览与最终结果、源映射、实时/整编来源与 stale 标志。
- Outline/Citations：符号导航、引用插入、跨文件重命名。
- Timeline：Action、checkpoint、branch、diff、revert/cherry-pick。
- Merge/Recovery：三方内容、语义诊断、逐冲突决议、恢复报告。

## 公共接口

- `EditorSession` 持有 `EditorCore`，负责 `open` 的路径/窗口策略、show、事件顺序与命令结果的 Scene 发布。
- 命令注册表（`command::registry`），`CommandSpec { id, title, tooltip, icon, shortcuts, routing, menu, toolbar, focus, writes, availability, checked, action }`；
  菜单、工具栏、焦点工具栏、命令搜索和快捷键调用同一 `CommandSpec`；能力判定（capability）与参数 schema 随协作命令接入。
- UI ↔ core 只使用 [协议设计](../PROTOCOLS.md) 定义的命令/事件 DTO，不导入 core 内部类型。
- 平台端口 trait（window、clipboard、file dialog、credential store、updater）由 app 实现，core 只依赖端口声明。
- 交付物是可执行程序 `scholium-app`，不承诺稳定库 API。

## 不变量

- UI 不持久化正文的第二份权威副本。
- “已保存”只在目标持久化后端确认写入后显示；恢复或保存失败须在所有编辑视图可见，且不得清除未保存状态。
- 源码长草稿使用独立滚动视口，提交/丢弃按钮不得随全文被挤出窗口；无障碍文本和光标坐标必须随视口一致更新。
- 页面编辑必须用该画面对应的编译几何定位；重编译中仍可按语义光标键入，但不接受过期画面的指针定位。
- 后台排版期间无障碍 Text/选区保持来自当前权威正文；尚无当前几何时不发布旧字符框，也不能临时移除 Text 接口。
- 数学光标的无障碍播报由当前语义树生成，包含嵌套槽位路径、当前内容和显式空槽；仅正文获得焦点时发布。
  槽位上下文通过独立 polite live 节点变化通知，保持正文 Text 偏移与编辑操作不受播报文本影响。
- 每个写命令包含 base revision 和 request id。
- 过期事件可识别并丢弃；不能用到达顺序推断新旧。
- 源码组合输入未结束时不得提交或重建草稿、切换草稿所属成员/项目；结束事件先交还原编辑器，
  同帧不能改变归属。UI 禁用之外，提交入口也必须拒绝预编辑内容。
- viewer 在前端只读，同时 core 也拒绝写命令。
- 关闭窗口前确认 WAL durable；无需等待慢构建。

## 失败处理

- core 返回 `Rebased`/`Rejected` 时，UI 必须应用 authoritative range/snapshot，不能清空全部 pending 或自行猜测补丁。
- `PlanStale` 或 precondition 失败时重新规划；冲突进入冲突预览，不静默选一侧。
- 离线或无写权限的源码输入转本地草稿/fork，并明确提示未上传的数据量。
- 构建或导出失败保留原项目与上次成功产物；编辑不被阻塞，失败原因可定位。
- 会话崩溃或强杀后走 storage 的恢复计划，不自动"修复"正文。
- 前端不得重试写命令绕过 session；viewer 的写命令在 UI 与 core 两侧都被拒绝。

## 测试门禁

命令路由、乐观编辑 rebase、IME、多光标、10 万行、窗口重载、stale diagnostics、只读权限、时间线与
冲突流程。端到端覆盖编辑—WAL—文件、构建预览和两客户端同步。

## 项目语言合约

窗口壳、命令注册表、i18n 与诊断面板属于共享底座，按创建/导入时固定的项目语言装配内核。
保存权威正文与草稿；跨语言只经「转换为…」建立新项目并展示兼容报告，导出生成副本。
混合编辑与团队语言切换已推迟；源码错误、离线草稿和同语言多人协作沿用现有恢复与权限规则。
共同要求见 [项目语言、跨格式转换与文件操作](../MIXED_SOURCE_EDITING.md) 与 [ADR 0041](../adr/ADR-0041-separate-latex-typst-kernels.md)。

## 已删除的旧页面编辑器

2026-09 至 2026-10 的旧编辑面（`page_editor`、`SessionBridge`、`workspace`/`preview`/`paper`、
`native_text`、`undo`）经全文 markup 编译出页面，并以 egui 文字回显掩盖排版延迟；它已由
Typst Content 编辑面取代并删除（[ADR 0037](../adr/ADR-0037-remove-legacy-editor.md)）。
相关裁决和证据只作追溯，不再描述本模块：[ADR 0024](../adr/ADR-0024-local-paragraph-integration.md)、
[ADR 0026](../adr/ADR-0026-block-document-session.md)、[ADR 0029](../adr/ADR-0029-direct-page-editing.md)、
[ADR 0030](../adr/ADR-0030-incomplete-formula-feedback.md)、[ADR 0031](../adr/ADR-0031-structural-cursor.md)，
以及报告 0019–0021、0044、0046–0048。

旧编辑面已验收而新编辑面尚未具备、须在其上重新实现与验收的能力：编辑面分页与纸张设置、
未完成公式的原位诊断、受信远端动作注入的阶段 0 窗口验证（生成源码与快速预览已重建，见上文）。
不得以恢复旧代码代替这些验收。正文无障碍已在新编辑面重新实现，见下节。

## 编辑面无障碍

`typst_editor::access` 把编辑面发布为 AccessKit `MultilineTextInput`（“正文”），子节点为按当前 Scene
精确 caret 分视觉行的 TextRun，段落末尾追加 `\n`；字符几何为屏幕逻辑像素，同簇字符共享停靠点。
位置换算只在（TextRun、Unicode 标量序号）与（叶、UTF-8 字节）之间进行；没有正文 run 的位置
（公式旁空叶、数学槽、纯公式段落）读作本段最近正文或段落占位 run，每段至少一个 run，光标不会消失。
runs 以快照身份与 Scene 是否当前缓存，几何存 Frame pt，发布时才乘页面 placement，滚动不重建。pending 时发布权威文本而不带几何，不移除 TextRun。`SetTextSelection` 只按上一帧发布的 runs
解码，正文已变、字素内部、输入排队或组合期间拒绝并提示。行内公式是 `Math` 兄弟节点，不进入正文文本；
数学槽位上下文由稳定的 polite `Status` 节点按由外到内路径播报，空槽读“空”。禁用控件依赖根 workspace
的 `accesskit_atspi_common` 补丁不报 Enabled/Sensitive。裁决见 [ADR 0038](../adr/ADR-0038-editor-accessibility.md)。
