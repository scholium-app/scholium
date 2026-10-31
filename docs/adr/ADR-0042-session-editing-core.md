# ADR 0042：编辑交互核心提取为不依赖 UI 的 scholium-session

- 状态：Fully implemented（[phase-1 日志](../log/2026-10-30-session-core.md)、[phase-2a 日志](../log/2026-10-30-session-core-input.md)、[phase-2b 日志](../log/2026-10-31-session-core-scene.md)）
- 日期：2026-10-30
- 决策者：协调者按 [ArtCraft 借鉴计划](../plan/ARTCRAFT_REFERENCE.md)采纳项 3 与用户确认的执行顺序
- 影响模块：app、document、typst、storage；新增 session
- 相关：[ADR 0025](ADR-0025-extension-boundaries.md)、[ADR 0041](ADR-0041-separate-latex-typst-kernels.md)、
  [会话与扩展组织计划](../plan/EXTENSION_ORGANIZATION.md)

## 背景

`scholium-app` 的 `EditorSession` 同时持有三类状态：权威 `LocalSession`、存储与撤销栈（会话层）；光标、选区、
垂直目标、输入样式、占位生命周期、点击序列与视图跟随意图（交互层）；egui 事件队列、纹理、屏幕放置与 IME 锚点
（界面层）。交互规则因此只能经 egui 测试，命令失败时无法整体回滚，无界面调用入口（离屏截图脚本、控制通道、
将来的 Pi/MCP/插件）无法复用同一套行为。ADR 0041 又要求工作区按项目语言装配内核，外壳不能与 Typst 编辑交互
硬绑定。

组织计划已为“不依赖 UI 的 session”预留 `scholium-session`，并规定第二个调用入口出现时提取。无界面场景驱动
就是第二个入口。

## 候选方案

1. 继续留在 app，只把纯函数拆成子模块：不能去掉 egui 依赖，验收无法在无界面中重放。
2. 新建只服务编辑交互的 `scholium-editor-core`，session 以后另建：与组织计划形成两套平行结构，违反采纳项 3。
3. 现在提取 `scholium-session`，Typst 内核的会话与交互核心作为其 `typst` 模块；项目级编排在需要时加入同一 crate。

## 决策

采用方案 3。

1. 新增 `crates/scholium-session`，依赖 model、document、storage、typst（`editor` 特性），**不依赖** egui、eframe、
   winit、accesskit。公共入口为 `scholium_session::typst::EditorCore`。
2. `EditorCore` 持有：权威 `LocalSession` 与快照、`SessionStore` 与已保存基线、撤销/重做栈与输入分组、光标与锚点、
   垂直目标、点击序列、输入样式、占位生命周期、视图跟随意图、排版 worker 与当前 Scene 的采纳（页面 pt 坐标）、
   诊断来源（布局错误、被拒输入、公式溢出）。
3. 输入是抽象事件：键（含修饰键）、已提交文字、指针（页面 pt 坐标 + 按下/移动/抬起 + 点击次数）、命令调用、
   视口（可见 pt 范围、栅格规格）。输出是编辑结果、光标/选区/overlay 几何（pt）、元数据视图与诊断。
   输入法预编辑、屏幕坐标换算、纹理、绘制、AccessKit 发布、对话框与 `AUDIT` 打印留在 app。
4. 所有命令经 `run(CommandId, args)` 执行，**失败原子**：执行前记录文档快照、选区、撤销/重做栈与输入分组；
   命令返回错误或 panic（`catch_unwind`）时全部恢复，并产生一条诊断。持久化写入只在成功后发生。
5. 分层门禁：`scripts/check-layers.py` 解析 `cargo metadata`，维护分层表（model → document → typst/storage →
   session → app）；app 以下任何 crate 直接或传递依赖 egui/eframe/winit 即失败，未登记的 crate 直接失败。接入 CI lint 作业。
6. LaTeX 内核将来以 `scholium_session::latex` 加入；在它出现前不抽象两个内核的公共 trait。

## 验证方法

- `scholium-session` 的测试不链接 egui：用抽象事件重放现有窗口检查覆盖的编辑场景（输入、公式进出、上下标、
  选区替换、撤销/重做、垂直导航、点击/双击/三击），断言快照、光标与撤销结果。
- 同一场景在无界面测试与真实窗口检查中得到相同的快照、光标与撤销结果。
- 注入失败（返回错误、panic）的命令不改变文档、选区、撤销/重做栈与输入分组，也不写存储。
- 既有单元测试、窗口检查与 stock 对照全部不改阈值通过；`check-layers.py` 对加入 egui 依赖的负对照失败。

## 后果

正面：交互规则可在无界面中测试；命令失败可整体回滚；控制通道、离屏截图与扩展入口复用同一核心；外壳可按项目语言装配内核。

负面：一次跨模块搬迁，app 的 `typst_editor` 大幅收缩，需要分两期合入以控制评审规模；`EditorCore` 的事件类型
成为 app 与 session 之间的内部契约，变化需同步两侧测试。

## 替换方案

`EditorCore` 的事件与输出类型是 crate 内部契约，不是长期 ABI；外部入口（Pi、MCP、插件）通过各自适配层调用，
不直接暴露这些类型。若将来项目级编排需要独立 crate，可从 `scholium-session` 拆出，内核模块保持不变。
