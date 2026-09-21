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

## ProjectSession

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
[P1 排版工作区规划与参考图](../plan/P1_UI.md)：标准菜单与紧凑工具栏、单一所见即所得编辑面、
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

## 阶段 0 窗口证据

[报告 0044](../spikes/SPK-0044-stage0-scope-review.md)以显式开关注入受信远端语义动作，验证结构窗口
本地包裹/文字撤销保留远端内容。入口仅用于原生验收，不是网络或共享 SDG adapter；阶段边界见 ADR 0023。

[报告 0019](../spikes/SPK-0019-native-window-review.md) 记录真实源码/剪贴板/预览闭环，
并修复候选窄窗口面板溢出和正文失焦后从无障碍树消失。
[报告 0020](../spikes/SPK-0020-body-accessibility-viewport.md) 补正文 Text/选区操作和双轴视口；
文本叶子按模型前序发布，系统字符偏移转 UTF-8 字节偏移，核心校验字素边界后接受选区。
真实 AT-SPI 跨节点选区、原生剪切/撤销已通过；报告 0020 时尚无数学角色/槽位朗读、自动光标跟随，后续见下方 0021；完整屏幕阅读器验收仍缺。
这些接口仅位于阶段 0 候选，不代表生产 app 合约已落地。

实验会话 UI/IO 边界见 [ADR 0015](../adr/ADR-0015-ui-session-recovery-spike.md)：
恢复语义树与独立草稿，加载结果须通过编辑变化门禁，重开重置渲染会话；不含历史/身份恢复。

团队窗口模拟接口见 [ADR 0016](../adr/ADR-0016-team-window-gate-spike.md)：
当前语言投影选择不授予共享写权限，源码候选必须在协调检查与 reconcile 都成功后发布。
三成员模拟不替代认证/网络，实验正文只读，不序列化为单人恢复文件。

[报告 0021](../spikes/SPK-0021-native-edit-acceptance.md) 补真实 Rime 合成/取消/编辑区焦点切换、
双源码草稿、鼠标拖选/滚动、Unicode 剪贴板和数学描述取证。正文组合失焦请求平台中断，
选区替换是单个核心动作，退格/撤销/解除结构后修复光标，拖选按实际 press origin 锚定。
Math 角色以独立只读语义投影暴露，TextRun 保持可编辑文本位置；禁用状态与屏幕阅读器验收仍有缺口。
完整共同验收尚未通过，未接入的团队、预览定位和文件恢复不得用独立模型测试替代。
