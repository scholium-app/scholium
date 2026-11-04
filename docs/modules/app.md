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

应用壳位于 `crates/scholium-app`，窗口外壳由上到下是菜单栏 28、主工具栏 36、焦点工具栏 32、文档标签栏 30、
编辑区、状态栏 24（逻辑像素，高度来自令牌；应用不自绘窗口标题与窗口按钮，`with_decorations(true)`）：

- `chrome/` 的菜单、主工具栏、焦点栏与溢出列表只显示命令注册表中适用于当前 `Kernel` 的已实现命令；
  空菜单和子菜单隐藏。只有已实现但当前不可用的操作灰显，悬停解释原因。
  Typst 原生项目保留段落样式下拉、文字格式、公式结构与历史；`.typ` / LaTeX 源码项目保留已接通的撤销重做。
  源码历史读原生 TextEdit 的 undoer；工具栏历史恢复源码焦点，向原生编辑器发送对应按键，不接入 Typst 历史。
  未实现的命令搜索不占工具栏位置。放不下的整组按注册表顺序收进“»”；删除占位项后 480 px 窗口能容纳现有主工具栏。
  Typst 正文焦点栏留空（行内命令打开时可显示 Tab 操作提示），公式焦点栏保留现有结构操作；源码项目左侧留空。
  右侧固定“页面编辑 / 源码预览”分段切换，Ctrl+Shift+M 与左右方向键仍可切换，源码项目切换不改变权威模型。
  标签显示项目名；默认数据目录的项目显示本地化“未命名 / Untitled”；未保存圆点和关闭确认保留。
  导航显示同一项目名及入口文件。`layout` 只负责无 egui 的整组宽度决策。
- `widgets/` 统一绘制按钮、输入框、下拉框、分段切换、弹出层、对话框、图标按钮、菜单、标签、状态片段与提示。
  按钮高 28、圆角 4，主按钮为强调色实心、其余为输入底色加描边；输入框高 28、有可见边框与焦点外圈，字段错误在下方。
  所有文件、丢弃确认、冲突、错误、迁移提示和快捷键帮助共用 `dialog`：标题右侧关闭，内容内边距 18，
  底部靠右、主按钮最右；Enter 只触发启用的主按钮，Esc / 关闭取消，宽度不超过窗口减 32，短窗口内容滚动；长路径按钮在可用宽度内截断，不撑宽弹窗。
  弹出层圆角 6，对话框圆角 8；只有弹出层和对话框有浅阴影，栏、纸面、辅助面板无阴影。
  菜单项高 26、快捷键靠右；模式段宽度按文字测量且不小于 96。状态来自令牌；键盘焦点环 2px 外扩 1px。
  可访问名称来自 catalog；仅有 `checked` 的命令暴露开关。提示提供操作说明、快捷键或禁用原因，不单纯重复标签。
- `icons` 把 Lucide 图标（ISC，许可证在 `assets/icons/LICENSE`）以内嵌 SVG 经 resvg 按设备像素比光栅化并用令牌着色，
  纹理按（图标、尺寸、线宽、颜色、ppp）缓存；工具栏 18/1.5、菜单与溢出 14/1.25、状态栏与标签 12/1.25、箭头 8/1.25。
  `IconId` 来自注册表；不引入图标字体或新的光栅库。
- `typst_editor` 是原生 Typst 编辑面（持有 session 的 EditorCore、Typst Content 场景、事件翻译、导航与迁移提示），`state` 只持有视图偏好
  （工作方式、面板、缩放、主题、语言、工具栏显示）与编辑器元数据，`command` 是命令注册表，`commands` 保留视图命令的处理函数。
  未接入的写操作不注册为可执行入口，不以本地变量伪造保存、构建或 revision 状态。

剪贴板命令经原生 viewport 请求发送 Copy/Cut/Paste 并先恢复页面焦点（`typst_editor/clipboard.rs`）；文字样式、段落样式、
公式与分数作为结构 intent 排入编辑会话，与键盘输入同序执行。结构视图禁用正文写命令；组合输入期间禁用相关写入。
Ctrl+F1 与“视图 → 显示工具栏”调用同一命令，同时显示或隐藏主工具栏与焦点工具栏。

`theme::Tokens` 是仅有的颜色来源：P1_UI 令牌表的全部语义色（浅色、深色两套 HEX）、栏高、控件高、字号与行高、间距刻度、
圆角与图标尺寸；控件一律读令牌，`theme` 之外不定义颜色字面量（测试检查）。浅色是默认主题，“视图 → 主题”在运行时切换浅色/深色
（偏好暂不持久化）；纸面在两套主题中都是白色，画在纸面上的光标、选区与公式底色因此使用浅色主题的强调色与 `math_bg`。
配色变更不修改布局、不影响文档内容。界面字体在启动时从系统字体库（fontdb，与 Typst 内核扫描同一来源）按序选择：
HarmonyOS Sans SC → MiSans → 苹方 → 微软雅黑 UI → 微软雅黑；西文优先 Inter，等宽优先 JetBrains Mono；随程序打包的
Noto Sans CJK 只作最后回退。所选字体写入 AUDIT 的 `fonts`。
Typst 编辑页与源码预览使用同一字体策略（见 [typst.md](typst.md)）：正文为 Times New Roman → Liberation Serif →
SimSun → Noto Sans CJK SC，标题将 SimSun 换为 SimHei。System 始终载入打包的四种 Liberation Serif 与 Noto，
并扫描宿主字体，使已安装的 TNR/宋体/黑体优先；Bundled 完全跳过宿主扫描，场景与 parity 验收使用它。
打包字体以 Brotli 无损字节嵌入，纯 Rust 解压器在进程内首次加载时解压一次；界面 Noto 回退经
`scholium_typst::bundled_cjk_font_bytes()` 复用同一缓存，避免再嵌入未压缩的 Noto。首次离线启动不下载字体。
界面字体偏好与文档字体列表相互独立；不同字体的外观可不同，混排断行与原始字节哈希由字体回归测试约束。

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
默认高 180 逻辑像素，顶边分隔条可拖动调整，不覆盖任何栏。左为“问题”列表（严重度图标、消息、所在段落摘录，点击或 Enter 跳转，空时显示“没有问题”），
开发字段（后端、revision、缓存计数、布局耗时）不绘制，只在 `SCHOLIUM_EDITOR_AUDIT` 保留。
面板打开且有问题时键盘焦点落在第一行；
上下键在行间移动，Esc 把焦点还给页面；Enter/空格激活后按键被控件消费，不会再被页面当作输入。行是普通控件，
无障碍名称为“严重度：消息”。AUDIT 的 `diagnostics` 列出诊断及位置，`controls` 里 `ui.diagnostics.row` 按序给出各行矩形。

## Session 的 UI 适配

### Typst Content 编辑入口

默认启动打开数据目录 `scholium/default-project`；显式路径由 `SCHOLIUM_PROJECT_PATH` 指定。
正式项目通过 `ProjectHandle` 决定语言与 authority，由 `project_workspace` 装配：原生 Typst 接现有页面 EditorCore，
普通 `.typ` 接最小源码输入与 SourceProject；LaTeX 通过 `open_latex_workspace` 将 ProjectHandle 与 egui context
装配为真实的源码/预览工作台，以项目声明的入口为准，不创建 Typst authority/worker。`source_view` 仍仅服务原生项目生成源码。
默认项目不存在时，只读迁移旧 structured/legacy 会话当前 head，旧库保留，显示迁移提示与报告位置；
失败或未完成迁移目标不能当空白文档打开。`SCHOLIUM_SESSION_FILE` 已退役，不再被主程序读取。
项目声明与迁移边界见 [ADR 0051](../adr/ADR-0051-project-files-and-session-retirement.md)。

### 文件命令与工作区装配

`project_files` 处理新建、打开、最近项目、旧会话导入、另存为与关闭，命令统一来自注册表；未实现的导出不进入界面。
无项目时，起始页显示应用定位与“开始 / 最近 / 环境”三组平面列表，不创建卡片。
开始操作与菜单共用命令及键位表；最近项目显示名称、路径、固定语言，可点击或 Enter 打开、移除记录，
上下键在开始、最近、移除、重试与刷新之间循环，控件发布可访问名称。
最近项目在用户配置目录 `scholium/recent-projects.json`，按成功打开的规范根路径去重，最多 12 个；
`SCHOLIUM_RECENT_FILE` 隔离验收配置。记录错误可见，不撤销已打开项目。

新建表单默认 LaTeX，语言单选附说明；模板为 article、ctexart 和 Typst 空白笔记。
名称、已有且可写的位置、浏览及目标文件预览各占独立字段；空名称、不安全名称、已有目标和不可写位置就地报错。
Enter 创建，Esc 取消；引擎在项目设置中选择。创建不覆盖已有目录，ctexart 模板经 SourceProject 保存。

打开经 `FilePicker` 接口：当前为批次一组件绘制的路径与最近目录回退，系统 portal 实现预留在 `picker.rs`，不增加系统依赖。
所有实现的返回值都经同一校验边界规范化，只接受已有目录或 `.tex` / `.typ` 普通文件；文件夹选择模式拒绝文件，
选择过程不创建项目元数据、不写源码。回退实现和模拟系统返回值共用契约测试。
目录只有一个入口时直接打开，多入口随后选择主文件；已有声明按其入口打开。
“从旧版本导入”位于起始页与文件菜单，打开对话框不混入迁移操作。

文档菜单为 LaTeX 与 `.typ` 源码项目提供项目设置，状态栏引擎项打开同一对话框。
表单显示主文件、LaTeX 引擎与实时/整编说明、参考文献工具，以及只读项目语言；Typst 隐藏不适用的构建选项。
`SourceProject::update_settings` 在现有项目锁下同步并原子替换 `.scholium/project.toml`，外部声明改变时拒绝覆盖。
设置将 v1 声明提升为 v2，保留项目身份、固定语言与 authority；入口资源映射保留每个文件的私人草稿身份。
切换主文件前须保存或丢弃当前草稿，并拒绝未完成物化意图；目标草稿与打开源码共用同一崩溃恢复基线判定。
引擎变更重建派生预览，旧 worker 的输出通道关闭，旧页面清空；不改磁盘源码或私人草稿。
参考文献选择保存为项目偏好，实际 bibliography 构建流水线另行接入。

项目切换与文件关闭有保存/丢弃/取消确认；关闭释放项目锁，返回起始页。原生窗口关闭沿用编辑器确认。
文件对话框打开时页面不接收键盘输入。另存为创建同语言新项目/资源身份并复制普通资源和当前内容，
保留构建偏好，不复制私人历史或协作身份；已有目标或原目录内部目标拒绝。成功后切换至副本，原项目保留。
源码输入写独立 draft，提交失败仍保留新输入并可见报错，保存由 materialize 比对磁盘 hash。
冲突对话框标题显示文件名与 UTC 修改时间，正文说明拒绝保存及操作后果；查看差异显示磁盘与草稿的统一差异，
取消保留双方、重新载入显式丢弃草稿、主操作另存为保留原文件。存在未完成物化意图时不允许重新载入清除证据。
强杀后重开恢复 draft；LaTeX 工作台使用相同 SourceProject，未保存文本也是预览初始输入。
锁冲突在起始页顶部或当前项目上的统一对话框显示项目路径与重试，不回退临时正文。
迁移提示使用相同模态框与平实文案，正文不显示内部元数据路径，原始旧库保留。

环境组直接消费 `scholium_latex::toolchain::ToolchainDetector`，显示 TeX 发行版、可用引擎、rtex 版本及完整性状态。
问题与建议操作按工具链提供的消息 ID 和参数在中英 catalog 本地化。初次检测及重新检测在 detector 的工作线程运行，
UI 只读取缓存；缺工具链不会禁用新建、打开或编辑。

编辑器持有 [session](session.md) 的 `EditorCore`，将 egui 文字、可迁移按键、页面坐标和视口转换为 `Input`，由核心
按事件顺序经唯一输入入口提交文字与命令；布局仅持有
完整不可变投影。输入接受和提交先于页面绘制，后台完成唤醒 UI，不去抖源码求值。界面
工具栏与页面之间没有任何信息行。状态栏按内核装配：

- Typst 原生：页码（连续编辑面为 1/1；生成预览可用时显示其页码）、正文词数、正文/公式/只读上下文、常驻问题计数、
  “Typst · 已同步 / 更新中”、缩放。CJK 每字与拉丁词各计一，按快照修订更新；有空间时保留最严重诊断与被拒输入复制入口。
- `.typ`：TextEdit 实际行列、现有问题计数、Typst 状态、缩放。源码预览尚未装配时明确显示“尚未生成预览”，不冒称已同步。
- LaTeX：实际行列、源码词数、现有工作台错误计数、规范拼写的 LuaLaTeX / XeLaTeX / pdfLaTeX 与实时毫秒或整编秒数、缩放。
  工具链缺失显示本地化状态项；工作台不显示原始 `missing SCHOLIUM_*`、调试标题或权威/未保存说明，原始细节仍写 AUDIT。

问题计数始终带最严重问题对应的图标，点击开关面板；宽窗口显示零条，窄窗口零问题时隐藏计数。
空间不足时按上下文、字数、页码的顺序整段隐藏左侧标签，不裁切字形；右侧引擎、计数和缩放按实际宽度保留。
当前面板数据仍用应用已有诊断结构，统一内核诊断来源另行接入。
点击有位置的诊断跳转页面，无位置则打开面板。窄窗口非零问题计数缩为图标加数字、适合宽度缩为图标，避免状态操作重叠。
后端项提示说明预览引擎；源码项目点击该项打开项目设置。revision 与布局统计只在 AUDIT，保存状态由标签圆点与窗口标题表示。
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
位置，并把窗口逻辑像素矩形限制在可见页面范围内。`IMEOutput.rect` 与 `cursor_rect` 都是 caret 矩形，
因为 egui-winit 0.36 的候选窗定位读取前者；不能发布整页矩形。egui-winit 在缺少 IME 输出的帧关闭平台输入法，反复开关会让 fcitx5 停止送字。
同帧 Text 仅在与 IME Commit 文字相同时视为重复丢弃；输入法透传的英文 Text 照常输入。
Escape 分层取消，不释放页面焦点：先取消选区、IME 组合与拖动；三者都没有且光标在行内公式内时，离开该公式，
光标落在公式之后的正文（与公式切换命令离开同位；空占位槽由占位生命周期移除）；公式外无事可做。
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
编译后翻到光标块所在页；点击预览把光标移到该块开头。标题为“由文档生成 · 只读”；空槽数与原串公式数在源码上方说明。
这是只读派生视图：不 reconcile、不写回，也不代替最终 LaTeX/Typst 构建。

左右方向键与 Tab 的事件翻译在 app，逻辑导航与语义光标/anchor 由 session core 持有。
app 的导航、点击、Esc 与选区变化解除待输入样式，并在导航后通过命令结算 Attach 占位；
占位创建、撤销及字素重锚由 session 执行。数学朗读为 `上下标（底数：…；下标：…；上标：…）`，
槽路径名为 底数/下标/上标。普通左右键按 base、sub、sup 顺序进入槽，再经过 Attach 边缘，不创建
临时 Hole；显式 LeaveSlot API 保留兼容生命周期。结构边缘继续显示公式指示底色，AccessKit 状态
播报「公式内，分数/上下标/根式/括号前/后」，不增加正文 Text 偏移。根式的槽路径为根指数、被开方数，
定界符播报为括号及其内容；中英文均提供对应文案。

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

页面点击的命中规则：点在某个行内公式的 bounds 内才进入公式（含左右各 0.5 pt 的边缘命中余量，取最近精确 caret；距结构边缘不超过 0.5 pt 时优先该边缘）；其余位置只在正文
精确 caret 上定位——按与上下键相同的视觉行划分，取与点纵向距离最小的行，再取该行 x 最近的停靠点
（行右侧空白到行末）；第一行之上、最后一行之下分别取与 Ctrl+Home/End 相同的逻辑位置（不依赖几何，文档以公式结尾时是公式后的空正文叶）；
点在以公式结尾的行右侧时取公式之后的正文位置，而不是公式之前的停靠点。整公式 bounds 是公式内所有节点（原子、分数线、Hole）bounds 的并集——Row 与 Attach 自身没有 Frame bounds。
双击选中指针下的 UAX #29 词段（空白、标点选该段），三击选中该块全部正文 Text，均写入既有
anchor/cursor 选区，复制、剪切、输入替换沿用同一路径。点击次数取 egui 的 `double_clicked`/`triple_clicked`
与会话自记的“位置相近（≤6 px）且间隔 <300 ms 的连续按下数”中较小者（egui 只比时间），在释放时生效
（按下时已按单击放好光标）；数学槽内双击保持光标并提示“数学槽位选区尚未接入”。

光标在行内公式内时，页面在公式整体 bounds 后画淡色圆角矩形、在当前 Fraction/Attach/Root/Delimited 槽外画更淡的描边
（均为形状，不画文字，几何取自当前 Scene），状态栏显示“公式”。Esc 在无选区、无组合时离开公式；末尾右键/开头左键同样可以离开。
公式内 Enter 的语义拆段由 session 执行。无运算符长串的溢出由 session 按当前 Scene 推导目标，
app 显示每个公式的“公式超出行宽”诊断，不改写内容。AUDIT 的 `formula_overflow` 只表示光标所在公式。

选框来自同一 Frame 的 cluster stops 和数学根 bounds，绘制非打印多边形，不画已提交
文字。缺几何的当前选区拒绝替换；pending 时可按有效语义端点继续键盘编辑，禁止使用旧
几何判断位置。范围内 RawMath 明确拒绝，失败保留正文、端点和可复制输入草稿。
范围替换、存活叶/字素位置、撤销端点与纯文本复制/剪切的拒绝规则由 [session](session.md) 定义；
app 只检查当前选区几何和交付原生剪贴板。数学行选区支持分数、根式和定界符包装；跨槽选区尚未接入。
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
  （`command.<id>.title|tooltip`，由 `command!` 宏从 id 派生）；`icon`（`IconId`，无图像）；`routing`（`App` 由应用先消费，`Editor` 留在页面事件流里与键入保序执行，`Platform` 是剪贴板事件、
  只用于显示）；菜单位置（菜单、可选子菜单、组、序）、可选主工具栏位置与焦点工具栏类型（文本/数学）；
  `writes`；`availability: fn(&CommandContext) -> Availability`，取 `Enabled`、`Disabled(原因 key)` 或
  `Unimplemented(原因 key)`；`checked`；`action`（复用 `ViewCommand`、编辑器 `Command`、撤销/重做与剪贴板请求）。
  `CommandContext` 是纯事实结构（模式、是否有文档、组合输入、选区、能否撤销重做、粗体/强调、面板与缩放状态等），
  由 `WorkspaceState` 与 `EditorView` 构建。
- **菜单**：文件、编辑、插入、格式、文档、视图、工具、帮助，内容见 [P1 UI](../plan/P1_UI.md#菜单工具栏与焦点工具栏组织)；
  未交付的命令照样注册，可用性为 `Unimplemented`，菜单与按钮置灰，原因写在悬停提示里。
  `chrome/menubar` 按组插分隔线、按子菜单分层生成菜单树；“视图”含工作方式、导航与诊断、缩放、显示工具栏，
  以及“主题”（浅色/深色）与“语言”（跟随系统/简体中文/English）子菜单，后两者同样是注册表里的命令。
- **快捷键**：`shortcuts::shortcuts` 按生效键位与作用域选择命令，修饰键精确匹配，标点允许布局所需 Shift。
  `Unimplemented` 不消费按键；页面命令留在编辑事件流保序执行。默认绑定与用户覆盖规则见下文“数据驱动键位”。
- **文字目录**：命令标题、提示、菜单名与不可用原因经 `i18n` 的 Fluent 目录（`i18n/zh-Hans.ftl`、`en.ftl`，
  见 [ADR 0040](../adr/ADR-0040-ui-string-catalog.md)）渲染；界面语言 `WorkspaceState.locale`（`auto` 按 `SCHOLIUM_LOCALE`、`LC_ALL`、
  `LC_MESSAGES`、`LANG` 的第一个非空值，否则英语；窗口检查按 `--locale` 在 zh-Hans 与 en 下各跑一遍，控件按命令 id 经 AUDIT 的 `controls` 映射点击，不依赖标签文字或坐标）可在“视图 → 语言”里运行时切换，下一帧生效，不影响文档与历史。窗口外壳的全部可见文字（菜单、工具栏提示、焦点栏、标签、状态栏、
  窗口标题、丢弃确认框）都走目录；源码窗格标题、预览状态和源码警告也使用带命名参数的目录 key。数学结构、槽位与边缘播报使用 `a11y.math.*` 目录；其余既有编辑面状态消息仍列在
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
- 命令注册表（`command::registry`），`CommandSpec { id, title, tooltip, icon, routing, menu, toolbar, focus, writes, availability, checked, action }`；
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

## 共享源码编辑器与 LaTeX 项目工作台

`source_editor` 提供本地 TextEdit、双向滚动、选区、原生撤销/重做、Ctrl+F 查找、诊断与高亮 byte-range 接口。
语言内核提供高亮内容，编辑器仅提供样式与标记；IME 使用独立预编辑缓冲，提交后才更新权威源码，整次提交为独立撤销点。
共享 `ReadOnlySource` 提供布局缓存与跟随滚动，Typst source_view 保持生成源码只读、块高亮、主题更新和预览定位行为。
LaTeX 源码键入反斜线启动 scholium-math-input 的同一候选引擎，Tab/Enter 接受、方向键选择、Esc 取消；
插入源码符号/结构片段，经 TextEdit 原生输入路径保证选区与撤销。粘贴与 IME 提交保持字面文本，不触发补全。

File > New 选择 LaTeX，或 File > Open 打开 `.tex` / 项目目录，均进入左右源码/预览工作台；
`SCHOLIUM_PROJECT_PATH` 使用同一路径。入口来自 ProjectHandle 声明，不固定为 main.tex；独立的 LaTeX 启动变量已移除。
SourceProject 持有唯一可写正文：编辑经 `edit` / `save_draft` 持久化私人草稿，Save 经 `materialize` 发布到入口文件。
外部冲突沿用文件层的重新载入/另存为/取消 UI；重新载入及另存为同步重建预览和编辑器状态。
SourceSession/LiveSession 只持有派生的预览快照，恢复 draft 后先覆盖快照入口，再启动预览；缺工具链显示明确不可用状态，源码仍可编辑。
文件模态框禁止工作台输入；源码项目的撤销/重做快捷键交给 TextEdit 原生历史，不被 Typst 历史命令截获。
IME 预编辑不写草稿，并阻止保存/切换，提交后才进入持久化路径。
工具链准备、协议、编译、字体与栅格工作全部离开 UI 线程，输入 watch 保留最新版本、队列有界。
LuaLaTeX 经 LiveSession 转发最小 UTF-8 patch；显示段落更新。非 Lua 引擎使用一次性整编并丢弃旧 revision。
UI 始终标明项目引擎、实时/整编来源、耗时及更新中状态。审计 submitted_ms 只代表 UI 提交阶段；
真实窗口 E6 通过私有 Xvfb 的 XGetImage 检测预览像素变化，不把协议回调或 UI 提交当作呈现。

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

### 行内 IME 组合

正文组合经独立 `Preedit` 排版输入在 caret 处交给 Typst，继承文档字体并加下划线，后文正常重排。
caret 使用 egui 给出的 active range 末端（Unicode scalar 转 UTF-8 byte），否则在组合串末尾。
新组合场景等待期间保留最后一幅 Typst 图，不叠加 egui 文字；取消或提交也保留旧图直到新图到达。
组合不进入快照、请求日志、撤销分组、生成源码、保存文件或 AccessKit 文本文字。
组合时暂停文档几何查询：指针、导航、选区 overlay、caret-follow 与 AccessKit 的位置几何不读取
split-run offset；辅助技术继续读取权威正文。正常 Scene 到达后恢复。Commit 先清除临时排版输入，
再走普通 `Origin::Commit`，不保留第二份可写文档。数学内暂不显示临时组合串，但 Commit 和 Escape
仍沿原有数学输入路径；不执行用户文字为 Typst 源码。

窗口测试可同时设置 `SCHOLIUM_EDITOR_AUDIT=1` 与 `SCHOLIUM_IME_CHECK_FILE`，通过原子写入
JSON `preedit`/`commit`（可选 `cursor_chars`）给同一 egui 输入路径送事件；正常应用不读取此文件。
该私有 Xvfb 测试验证 X11 窗口中排版与 IME 输出，不替代 fcitx5/Wayland 协议验收。

## 数学分数选区呈现

分数按钮、插入菜单与键位表分派经同一 session 命令，在光标处插入空分数或包装同一数学行选区。
数学行的 Shift+左右选择和折叠由 core 决定；app 的 `selection_quads` 从当前 Scene 读取 core 的数学选区几何。
正文跨公式选择使用 `selection_math`，因此包含 Text 与 Fraction 的 Row 不依赖不存在的 wrapper bounds。

根式和定界符的菜单、焦点工具栏入口共享 session 命令。光标在根式内时焦点栏提供“添加根指数”，
已存在时进入该槽；在定界符内时提供循环配对的命令。两项针对最内层包含光标的对应结构。


## 数据驱动键位

命令注册表只声明稳定 id、可用性与动作；默认绑定来自随应用嵌入的
[`default-keymap.toml`](../../crates/scholium-app/src/command/default-keymap.toml)。Ctrl 按平台映射到 Cmd，
Alt、Shift、Meta 独立匹配。作用域由宽到窄为 global、page/source、math、focus:结构类型；最窄匹配优先，
同作用域的重复组合与未知命令 id 拒绝。编辑键在页面事件流中按顺序执行，会话不再持有产品快捷键。

启动时从用户配置目录 `scholium/keymap.toml` 叠加覆盖（Linux 为 XDG_CONFIG_HOME 或 ~/.config，
macOS 为 Library/Application Support，Windows 为 APPDATA；测试可指定 SCHOLIUM_KEYMAP_FILE）。
每条 `(command, scope)` 替换该项全部 keys，空数组解除绑定；修改后重启生效。整份文件先验证再安装，
解析、引用、冲突或读取失败保留默认表，诊断输出到 stderr 并显示在帮助→快捷键窗口。
菜单右侧、工具提示、快捷键帮助均读取同一生效表。


页面中的反斜线命令输入由共享引擎维护 query 与候选，app 在当前 Typst caret 下方绘制独立浮层；
上下键选择，Enter/Tab 接受，Esc 取消。浮层只显示未提交 query 与候选，不回显已接受正文。
焦点栏读取 `EditorCore::tab_action` 显示补全/循环/槽位提示，窄窗口把放不下的数学命令收进“»”菜单，保留模式切换。Linux Alt 快捷键同时产生的文字事件被消费，
避免结构后夹入字母。原生剪贴板事件也重新经过生效键位表，解除或改键不会保留默认别名；
菜单触发的原生剪贴板请求带一次性标记，避免被误当物理默认键重复过滤。
