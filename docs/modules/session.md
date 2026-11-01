# scholium-session

## 职责

提供不依赖 UI 的本地编辑会话，公共入口为 `scholium_session::typst::EditorCore`。
Typst 模块持有唯一 `LocalSession<StructuredDocument>`、不可变 `Arc` 投影、`SessionStore`、保存基线、
光标/选区、撤销/重做与输入分组时钟、待输入样式、Attach 占位生命周期，以及纯数据诊断来源。
默认文件的旧 head 迁移调用和显式文件的 restore/open/save 都在此层执行。
边界见 [ADR 0042](../adr/ADR-0042-session-editing-core.md)。

## 非职责

不处理 egui 事件、IME 预编辑、屏幕 placement、IME 锚点、绘制和 AccessKit；这些仍由 app 翻译或呈现。
抽象 `Input` 处理提交文字、工具包无关的按键、页面 pt 指针事件、视口/栅格请求，并以 `Outcome` 发布语义光标、
选区、跟随意图与 overlay 数据。指针状态、视觉上下导航（含目标 x）和点击序列属于核心事务状态，失败时与文档、历史一并回滚。
排版 worker、完整 wanted SceneStamp、已采纳 EditorScene、可见 ViewRange 与 RasterSpec 由 core 持有。
`Input::Viewport` 更新可见 pt 范围与栅格请求；core 按 40 ms 节流和 120 ms 稳定延迟请求条带。
Scene 只在完整 stamp 与 wanted 相等时采纳；旧 Scene 可继续显示，但不提供指针或视觉行导航权限。
抽象 deferred 记录由 core 持有并决定顺序、上限、指针拒绝和布局失败处理；匹配的 egui 事件 payload 队列仍在 app，
只按 core 的 Execute/Queue/Wait/拒绝结果存取。IME 预编辑、栅格纹理、绘制、屏幕 placement、IME 锚点、AccessKit、
源码视图、确认框、Fluent 诊断呈现和 AUDIT 打印由 [app](app.md) 持有。
不实现协作 undo、项目权限协议、Pi/MCP/插件协议或 LaTeX 公共内核 trait。

## 内部结构

```text
src/typst.rs                  EditorCore、纯语义诊断位置、只读 authority 视图
src/typst/command.rs          CommandId/CommandArgs/Outcome、原子 run 入口
src/typst/command/checkpoint.rs  执行前 checkpoint、错误/panic 回滚
src/typst/edit.rs             StructuralRequest、存活光标、历史与投影发布
src/typst/persistence.rs      open/restore/save、空白文档、只读旧 head 迁移
src/typst/history.rs          本地撤销/重做、输入/删除分组、可注入单调时钟
src/typst/input.rs            抽象输入分发，以及字面文字、公式进出、分数、删除与拆段
src/typst/events.rs           工具包无关的 Input、Key、Pointer 词汇与原子分发测试
src/typst/key.rs              工具包无关的按键映射与 caret-follow 意图
src/typst/runtime.rs          worker、完整 Scene identity 采纳与 raster/strip 交付
src/typst/deferred.rs         抽象事件的等待、顺序、拒绝与上限
src/typst/raster.rs           编辑与预览适配共用的栅格稳定策略
src/typst/pointer.rs          页面 pt 命中、点击选区与拖选
src/typst/vertical.rs         视觉上下导航、Home/End 与目标 x
src/typst/selection.rs        正文端点顺序、纯文本复制、范围替换
src/typst/style.rs            待输入样式与选区样式、active styles
src/typst/attach.rs           上下标、槽位出口和临时 Hole 生命周期
src/typst/math_enter.rs       公式后拆段的一步撤销
src/typst/indicator.rs        调用方证明当前的 Scene → Frame pt 指示框
src/typst/diagnose.rs         段落文字与公式溢出目标
```

依赖仅向下：model、document、storage、typst（`editor`）、thiserror，以及从 app 搬来的同版本
unicode-segmentation（保留组合字符、ZWJ、区域指示符的字素边界行为）。不得直接或传递依赖
egui、eframe、winit、accesskit，测试也相同；由 `scripts/check-layers.py` 强制。

## 公共接口

- `EditorCore::open(path)` 打开/创建 v1 store；`prepare(target, legacy)` 只在目标不存在且旧文件有 head 时迁移。
  路径选择、迁移提示格式与启动失败页面由 app 提供。已有目标与旧文件从不覆盖。
- `run(CommandId, CommandArgs) -> Result<Outcome, CommandError>` 是编辑命令的唯一执行入口。
  支持 app 注册表现有的公式、分数、上下标、文字样式、段落类型、undo/redo，以及 SelectAll、Delete 和
  `Text`（Typed、Commit、Paste）；语义替换、拆段、槽位离开/移除/收尾、新建和显式 StructuralEdit 同样经此入口。
  不匹配的参数形状返回错误。`Outcome` 告知投影是否变化、条件操作是否应用、旧 Scene 是否需要丢弃，
  以及 app 是否应清除旧编辑错误或拖选；不返回屏幕类型。
- `save()` 只写已接受快照及请求日志，成功后更新保存基线；命令本身不自动保存，`dirty()` 比较完整保存内容。
  app 在 IME 组合和排队输入结束后才请求保存。
- `session` 是不提供 `DerefMut` 和构造器的只读 authority 视图；`session()` 提供同一不可变引用。
  app 不能从此调用可变 document 方法。`restore_document` 在验证后恢复本地文档，供恢复入口和 fixture 使用。
- app 只把 egui 文字、按键和页面指针翻译成 `Input`；光标/anchor、输入分组、输入样式、视觉导航、点击选区和
  follow 意图均由 core 更新。`record`、`range`、`selected`、`active_styles`、`in_math` 读取同一权威投影。
- `Input::Pointer` 的双击选词、三击选段由 core 处理；`Release` 携带点击数时也独立完成该手势，
  不要求宿主重造此前的按下事件。app 只判定像素距离/时间所对应的点击数，不在绘制后另触发选择。
- `semantic_projection()` 返回稳定快照摘要、cursor/anchor 和 undo_depth，供无界面与 AUDIT 共用。
  摘要仅去除随机 node/document 身份和 revision，保留整个语义树、内容与样式；端点为结构路径、UTF-8 byte 与 affinity。
- 场景 worker 可固定使用应用打包 Noto CJK 与 Typst embedded 字体；该 profile 在 worker 创建时固定，避免排版测试读取宿主字体库。
- `math_indicator(scene)`、`find_overflows(scene)` 要求调用方先证明 Scene 当前；返回 padded Frame pt 或叶 ID，
  worker/Scene 由 core 持有。layout error、rejected input 与 overflow targets 是纯数据，语言目录和状态栏在 app。

- `open_with_wake(path, wake)` 创建真实 worker；`submit_layout` 启动首次/显式重排，成功编辑输入自动提交当前投影。
  `poll_layout` 经 `adopt_scene` 比较完整 stamp 后更新当前 Scene、几何和溢出目标；`take_scene_rasters` 仅移动位图条带，
  Scene 几何与身份仍归 core。`poll_strips` 过滤其他 Scene 的结果，app 只上传纹理。
- `admit(DeferredEvent)` 与 `next_deferred()` 决定 egui 事件的匹配记录何时执行；`deferred_len` 是待处理输入的权威计数。
  文字/粘贴/普通键最多等待 256 条，IME 生命周期和 Escape 取消不受此上限；被拒文字保留为可复制草稿。
  布局失败只跳过依赖视觉行几何的键，后续文字仍依次执行。`clear_deferred` 用于用户确认丢弃后清空输入。

## 不变量

- 只有一个权威文档；排版、源码与界面读取同一已接受投影。输入文字不会作为 Typst 代码执行。
- 每个 `run` 在执行前保存快照、完整请求身份、光标/anchor、undo/redo、open run 与 clock、pending style、placeholder
  和保存/诊断基线。多次结构编辑（如插入后设置样式）同属一次命令事务。
- 核心不包含 UI 或存储写入；注入时钟只读单调时间；`AssertUnwindSafe` 只包围可完整恢复的内存执行。存储不在 unwind 边界内。
- 历史条目的 `HistorySnapshot` 包装 `Arc<StructuredDocument>`，请求日志为 `Arc<[RequestId]>`；
  克隆 undo/redo 只做 O(栈长) 的指针复制，占位持有的历史条目同样共享。`HistorySnapshot` 保留与 owned 文档的值比较。
  checkpoint 的权威直接保留 LocalSession 已验证的共享快照和 action journal，投影与 saved 基线也复用 Arc，
  不逐命令复制全文或重建/验证请求日志。接受结构编辑仍在独立规划副本上验证，提交时 action journal 写时复制。
- 错误与 panic 恢复 checkpoint；已接受权威的回滚副本无需再次验证，恢复不执行可失败校验。失败前没有文档修改时保留 epoch；
  已部分修改的 authority 用 restore 换新 epoch，再请求 Scene，旧几何不得恢复命中权限。
- 正文/数学位置是稳定叶 ID、叶内 UTF-8 字素边界与 affinity。插入/删除后按已接受内容重新验证字素边界。
- 本地历史最多 512 个分组；同叶、连续位置、间隔 <1 秒的键入与 IME 提交按词合并（含词尾空白）；
  连续删除单独合并，粘贴单独一步，保存不打断分组。app 将导航和命令键的分组中断传给 core。
- 选区替换保持范围外身份；多行文字走 `ReplaceBodyRange`。RawMath 范围明确拒绝；纯文本复制拒绝公式与原串。
  当前 Scene 的选区几何完整性由 app 检查；pending 时语义端点有效即可编辑，不使用旧几何。
- 选区文字样式切换保持同一文字范围；无选区仅武装下一次输入样式。插入与其样式同一步撤销。
  单叶只有一种样式，粗体与强调不能叠加；公式内拒绝文字样式。
- `^` / `_` 仅对 Typed 数学文字创建槽位，Paste/Commit 保留字面。LeaveSlot 追加的临时 Hole 在未编辑而离开时
  恢复之前快照、丢弃多余撤销项；已经输入则成为内容。公式内 Enter 到公式后的正文拆段，作为一步撤销。

## 失败处理

命令返回结构化错误和纯语义拒绝原因，由 app 转为既有诊断文本。返回的 panic 诊断不携带 payload。
被拒 Text/范围替换输入在回滚后另存为可复制草稿，不进入文档。错误/panic 不写 store，也不移动保存基线。
保存失败保留 dirty 和内存内容；迁移/恢复失败不猜测另一权威。RawMath 不自动执行、降级或回显。

## 测试门禁

无界面测试驱动同一 `run`，覆盖正文、公式进出、分数、上下标与临时槽位、粗体/强调、选区替换、
输入分组与 undo/redo、保存重开，以及参数拒绝和修改后的 Err/panic 完整回滚（含磁盘 saved head）。
既有 app 单元测试、双语言真实窗口和独立 stock Typst 像素对照保持原断言/阈值。
工作区 fmt、clippy、test、rustdoc、规模/链接/UI 字串门禁与 layer gate 共同验收；layer gate 有真实 egui 负对照。
共享回归测试检查满历史、redo、保存基线与权威的 Arc 身份，以及修改后 Err/panic 的隔离和 epoch 语义。
`python3 scripts/benchmark-session-checkpoint.py --label <name> --out <json>` 运行显式 ignored 计时测试：
Typst 确认的 26 页夹具、512 步历史、8 次预热、100 次实际 Text 中的 checkpoint 采样，输出原始纳秒和 p50/p95。
此计时不包含命令执行或 checkpoint 析构；检测到机器休眠则废弃本轮，不设置依赖机器速度的测试阈值。

## 临时组合排版

`set_preedit(Option<Preedit>)` 只向派生 runtime 保存正文显示输入，分配新的完整 SceneStamp 并请求
既有 worker；不改 authority、快照、history、请求日志或保存基线。平台 IME 生命周期和字符索引转换
仍由 app 持有。`scene_is_current` 表示显示场景最新；`document_scene_is_current` 额外要求不含 preedit。
组合 Scene 采纳时不安装文档 geometry，`sync_scene_geometry` 也不安装 split-run 几何；取消或提交的
正常场景到达后恢复。持久化只读取 authority，即使显式保存也不包含临时串。见
[ADR 0044](../adr/ADR-0044-transient-ime-layout.md)。
