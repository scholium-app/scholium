# scholium-app

## 职责

提供桌面壳、页面式结构编辑器、Source Studio、焦点/模式工具栏、双后端预览、项目树、大纲、引用面板、
历史时间线、冲突解决、协作 presence、命令系统、设置和 core session 编排。

## 非职责

不直接写项目文件，不实现 CRDT/解析/历史算法，不拼接构建命令，不在前端保存永久 token。

## 内部结构

```text
app-shell
├── native_ui/      原生 editor、panels、preview、state projections
├── core/           ProjectSession actors、command handlers
├── bridge/         Rust 消息通道、平台坐标转换、可选原生 FFI
└── platform/       window、clipboard、file dialog、keychain、updates
```

桌面框架严格按 Iced → GPUI → C++ EUI-NEO → Slint 或 egui 依次验证，已选定 egui / eframe 0.36.2（ADR 0006）。
窗口、输入法、文本塑形、布局、GPU/软件绘制与可访问树均采用原生接口；不使用浏览器编辑器或 WebView。
Rust 负责权威文档和业务；EUI-NEO 候选由 C++ 实现原生 UI，通过受控 ABI 连接 Rust。必要 C/C++/Zig 依赖单独审查。
原生框架提供基础控件不代表现成支持数学结构编辑。结构视图与源码视图分别验证编辑适配、IME、选区、撤销、
大文件、协作及无障碍；不得直接把静态排版图片当可编辑控件。详细标准见 [原生 UI 验证计划](../plan/NATIVE_UI_VALIDATION.md)。

## 应用壳与主题边界

应用壳位于 `crates/scholium-app`：`chrome` 负责文档标签、工作方式切换与状态栏，
`ribbon` 负责文件菜单、功能标签与可折叠的分组命令区，`typst_editor` 是唯一编辑面（结构会话、
Typst Content 场景、输入、选区、导航与启动迁移），`state` 只持有视图偏好与编辑器元数据，
`commands` 集中路由视图命令。`icons` 以主题描边自绘矢量图标，不引入图标字体；图标按钮必须带
悬停提示与可访问性标签。未接入的写操作保持禁用，不以本地变量伪造保存、构建或 revision 状态。

`ribbon::RibbonState` 只保存当前功能标签与折叠状态，不保存正文或修改 revision。
`ribbon/controls` 提供统一高度的分组、矢量图标大按钮与不换行的小按钮；`ribbon/groups` 编排
开始、插入、布局、视图的命令。窄窗口使用溢出时可见的水平滚动条，不把文字挤成多行或缩小点击区域。
Ctrl+F1 与收起按钮调用同一 `ToggleRibbon` 命令；点击标签展开，双击标签收起。

开始/插入标签由 `typst_editor::ribbon` 提供：剪贴板按钮经原生 viewport 请求发送
Copy/Cut/Paste 并先恢复页面焦点；文字样式、段落样式、公式与分数作为结构 intent 排入编辑会话，
与键盘输入同序执行。结构视图不启用正文写按钮；组合输入期间禁用相关写入。纸张、页边距、
文档对象等未接入能力只提供禁用入口，不以 Ribbon 布局表示能力交付。

主题使用 `theme::Palette` 的语义色槽（工作区、面板、边框、正文、次级文字、强调、选区）。
egui 控件与自绘区域从同一主题取色；配色变更不修改布局、不影响文档内容。
主题选择、外部配色文件和偏好持久化可独立接入，不能把主题常量散落到各工作区组件。

## ProjectSession

### Typst Content 编辑入口

启动即打开 Typst Content 会话（[ADR 0036](../adr/ADR-0036-typst-default-editor.md)、
[ADR 0037](../adr/ADR-0037-remove-legacy-editor.md)）；旧回显编辑器已删除，没有回退路径。未设 `SCHOLIUM_SESSION_FILE`
时文档为 XDG 数据目录 `scholium/document.v1.sqlite`；它不存在而同目录旧版会话有已保存 head 时
一次性迁移到新文件，旧文件只读、保持原样，页头显示迁移提示与转为原串的公式数。已有 v1
文件不被迁移覆盖；显式 `SCHOLIUM_SESSION_FILE` 必须是 v1。没有可写副本对拷。旧文件只读探测；
迁移或打开失败时窗口只显示原因与恢复方式，不运行编辑器。迁移只发生一次，此后不再读取旧文件。

编辑器按事件顺序向 `LocalSession<StructuredDocument>` 提交字素文字/结构请求，布局仅持有
完整不可变投影。输入接受和提交先于页面绘制，后台完成唤醒 UI，不去抖源码求值。界面
持续显示 backend、正文/画面 revision 与 pending；旧图停用命中/光标/IME，不回显已提交文字。
纹理、caret、命中与 IME 共用一套 Frame-pt 到窗口逻辑像素 placement。

Ribbon 的开发命令发结构 intent；文字样式作用于整叶，标题作用于段落，源视图禁用正文
写按钮。单机 undo/redo 每步保留快照及其完整请求日志，最多 512 步，restore 换 epoch。
保存读取唯一 LocalSession 投影，dirty 比较完整保存内容。RawMath/容量/布局错误显示诊断，
被拒绝文字保留可复制草稿；不能在同一编辑面自动回到旧回显。

源码工作区（Ctrl+2）左侧是 `generate_source` 的只读 Typst 文本，光标所在块高亮；右侧是同一快照的
A4 快速预览，首次进入才启动预览 worker，快照变化即以新 generation 提交、只显示当前 generation 的页，
编译后翻到光标块所在页；点击预览把光标移到该块开头。空槽数与原串公式数在源码上方说明。
这是只读派生视图：不 reconcile、不写回，也不代替最终 LaTeX/Typst 构建。

文字插入/删除接受后，编辑器必须在新叶内容中重新校验光标的 UTF-8 字素边界。ZWJ、区域
指示符或组合字符可能把原先独立的邻接字素合并；此时光标移动到原目标 byte 之后的首个
合法边界，不能把下一次输入提交到字素内部。正文与数学 Text/Hole 的左右方向键在文档
逻辑叶顺序中跨边界移动，文档首尾停止，不循环；导航不产生语义动作。Tab 仍用于显式
槽位切换。

上下方向键与 Home/End 只读当前 Scene 的精确正文 caret（padded Frame pt）推出视觉行：
行内公式的槽位与正文行纵向重叠，不参与划行。上下键取纵向最近的相邻行、再取最接近目标
x 的停靠点；连续上下在光标和期望 Scene 都未变时保留目标 x。软换行同一字节的 Upstream
停靠点属于上一行末、Downstream 属于下一行首，等 x 时取逻辑较后者。无相邻行时停在本行首/末。
数学槽内上下先在最近的分子/分母间切换，否则以整公式 bounds 代表当前行离开公式；
槽内 Home/End 保持槽内首末。Ctrl+Home/End 按逻辑顺序到正文首末，不需要几何。
排版未就绪时几何键及其后的输入按到达顺序排队（上限 256，超限文字保留可复制草稿；
IME 与 Escape 不受上限丢弃），Ribbon/撤销/保存也等队列清空，关闭时排队输入按未保存处理；
排队期间的鼠标按下被拒绝，布局失败只放弃排队的几何键。目标 x 只在连续上下间保留。
视觉 bidi 不从逻辑顺序猜几何；仅有公式而无正文 caret 的行不作为上下停靠行。

选区用正文 Text 叶的稳定身份、UTF-8 字素边界保存 anchor/focus。Shift 左右键按正文
逻辑顺序跨叶/块，完整覆盖的结构公式作为对象选中；Shift 上下/Home/End 按视觉行扩展。
普通左右键折叠已有选区。鼠标按输入事件顺序命中同一当前 Scene 的精确 caret；拖选绑定
完整 SceneStamp 和 placement，epoch、布局请求或页面位置/缩放改变即停止旧拖选。
比例合字内部 caret 不作鼠标端点；未知身份和 pending Scene 不能恢复命中能力。

选框来自同一 Frame 的 cluster stops 和数学根 bounds，绘制非打印多边形，不画已提交
文字。缺几何的当前选区拒绝替换；pending 时可按有效语义端点继续键盘编辑，禁止使用旧
几何判断位置。范围内 RawMath 明确拒绝，失败保留正文、端点和可复制输入草稿。
输入、Backspace/Delete、Enter 和多行 Paste 通过一个 ReplaceBodyRange 提交；核心返回
存活叶和合法字素位置，UI 不猜拆段后的新 ID。撤销恢复两端并换 epoch。数学内部槽位
选区、选区格式和结构命令另行接入。复制/剪切只输出纯文本：段落以换行连接，范围含公式或
原串时拒绝且不改正文。见 [ADR 0035](../adr/ADR-0035-typst-page-selection.md)。
与其他行内内容相邻的空 Text 叶在 Typst 中不成 cluster、没有可绘范围；选框跳过它，
语义范围仍包含它。视觉行导航见[报告 0058](../spikes/SPK-0058-typst-vertical-navigation.md)。

一个打开项目对应一个串行 session actor，持有 authority mode、语义/源码共享文档 handles、history facade、
format/index service、render/build scheduler 和 sync client。所有写命令串行化，长任务只持有不可变 snapshot，完成后按
revision 交付。

## UI 状态

前端只保存投影：当前视觉树、source buffer mirror、selection、focus、viewport、panel layout 和 pending request。
正文权威状态在 core；前端对输入做乐观显示，core 拒绝 base revision 时返回 semantic/text rebase delta。

## 命令系统

所有用户动作注册为 command，声明 id、标题、快捷键、适用 capability、是否写入、intent 和参数 schema。
菜单、命令面板和快捷键调用同一 command。写命令必须经过 session；前端按钮不能绕开权限和历史。

## 主要界面

应用壳遵守[体验规范的工作区原则](../EXPERIENCE.md#工作区组织)，实现时参照
[P1 排版工作区规划与参考图](../plan/P1_UI.md)：带文件菜单的 Ribbon、单一所见即所得编辑面、
源码与预览默认约等宽、辅助面板按需展开。下面的能力列表不表示所有面板应同时常驻；
字体、尺寸、分组和平台控件可以按实际可用性调整，不做像素复刻。

- Workspace：文件树、搜索、依赖与资源状态。
- Visual Editor：即时排版树、文本/数学模式、槽位、焦点工具栏、远端光标。
- Source Studio：LaTeX/Typst/Markdown/BibTeX 源码、生成状态、诊断和 reconcile preview。
- Preview：Typst 快速预览、LaTeX/Typst 最终预览、源映射、后端与 stale 标志。
- Outline/Citations：符号导航、引用插入、跨文件重命名。
- Timeline：Action、checkpoint、branch、diff、revert/cherry-pick。
- Merge/Recovery：三方内容、语义诊断、逐冲突决议、恢复报告。

## 公共接口

- `ProjectSession::open(root, policy)` / `close()`；一个项目一个串行 actor，写命令只能经 session 提交。
- 命令注册表 `register(CommandSpec)`，`CommandSpec { id, title, keybinding, required_capability, writes, intent, params_schema }`；
  菜单、命令面板和快捷键调用同一 `CommandSpec`。
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
- 源码组合输入未结束时不得提交或重建草稿、切换草稿所属成员/语言；结束事件先交还原编辑器，
  同帧不能改变归属。UI 禁用之外，提交入口也必须拒绝预编辑内容。
- viewer 在前端只读，同时 core 也拒绝写命令。
- 关闭窗口前确认 WAL durable；无需等待慢构建。

## 失败处理

- core 返回 `Rebased`/`Rejected` 时，UI 必须应用 authoritative range/snapshot，不能清空全部 pending 或自行猜测补丁。
- `PlanStale` 或 precondition 失败时重新规划；冲突进入冲突预览，不静默选一侧。
- 许可失效、离线或无写权限的源码输入转本地草稿/fork，并明确提示未上传的数据量。
- 构建或导出失败保留原项目与上次成功产物；编辑不被阻塞，失败原因可定位。
- 会话崩溃或强杀后走 storage 的恢复计划，不自动"修复"正文。
- 前端不得重试写命令绕过 session；viewer 的写命令在 UI 与 core 两侧都被拒绝。

## 测试门禁

命令路由、乐观编辑 rebase、IME、多光标、10 万行、窗口重载、stale diagnostics、只读权限、时间线与
冲突流程。端到端覆盖编辑—WAL—文件、构建预览和两客户端同步。

## 团队源码语言与混合项目合约

ProjectSession 持有活动语言控制投影、许可、草稿与切换状态。源码可写性由 core/协调者验证，UI 只反映状态。
RequestSourceDialectSwitch 展示待冲刷成员、超时隔离和草稿处置；编辑异语言组件、宏、模板时走同一入口。
源码语言、排版 profile、单次导出目标是独立控件；导出不切换语言。保存/另存为/导出区别见专项合约。
离线源码输入标明草稿/fork，重连请求合入；语法错误照常保存。导出明确草稿政策和标准/混合包依赖。
新增三人同语言/视觉协作、切换期间 IME、模板冲突、异语言只读与双目标输出端到端测试。

共同要求见 [混合源码与团队编辑](../MIXED_SOURCE_EDITING.md) 与 [ADR 0001](../adr/ADR-0001-mixed-source-team-editing.md)。

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
