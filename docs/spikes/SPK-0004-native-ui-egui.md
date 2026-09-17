# Spike 0004：原生 UI 候选 — egui

> **有效性（2026-09-17 登记）**：选定候选的原始验收证据；选型由 [ADR 0006](../adr/ADR-0006-native-ui-framework.md) 固定，后续能力进展见 [报告 0016](SPK-0016-working-tree-status.md) 与 [报告 0021](SPK-0021-native-edit-acceptance.md)。阶段 0 当前判定见 [报告 0012](SPK-0012-exit-criteria.md)。

> 历史记录：以下保留该轮验证结果。2026-09-16 工作区后续实现与复测见
> [报告 0016](SPK-0016-working-tree-status.md)，当前出口判定见[报告 0012](SPK-0012-exit-criteria.md)。

- 结论：**通过共同验收的可验证部分**（可访问性与输入法均实测可用；选区与增量布局未做，见文末「第 1 项结论」）
- 对应验证项：[路线图阶段 0 第 1 项](../plan/ROADMAP.md)
- 日期：2026-09-16
- 执行者：ation_ciger
- 关联 ADR：[0002 原生技术栈与 UI 验证顺序](../adr/ADR-0002-native-ui-validation-order.md)

> **这是四个候选里第一个没有硬性 Fail 的。** Iced 与 GPUI 是因为**框架不具备**能力而 Fail；
> egui 具备能力，只是候选实现尚未接完，因此记 Blocked。这个区分对选型很关键。

## 为什么验证 egui（而不是先做 Slint）

按验证计划第 1 节，末组"根据前三者的缺口决定先验证哪个"。前三者（含预筛的 EUI-NEO）在
**同一处**缺口上一致：都不具备可访问性。按计划自身的规则，末组优先验证具备该能力的 egui。
预筛依据见 [报告 0003](SPK-0003-native-ui-prescreen.md)。

## 环境与方法

| 项 | 值 |
|---|---|
| 版本 | `eframe` / `egui` = `0.36.2`（`accesskit` 是默认 feature） |
| 结构渲染 | 用 `egui::Painter` 绘制同一份 `core::layout`（`text` + `rect_filled`） |
| CJK | 运行时把 `SourceHanSerifCN-Regular.otf` 装进 `FontDefinitions` |
| 脚本 | `scripts/smoke.sh`、`scripts/a11y-test.sh`、`scripts/input-test.sh` |

## 一次必须记录的测试方法错误：无障碍开关

**我最初三次无障碍测试都是在会话无障碍关闭的状态下做的**，因此结论全部无效：

```text
$ gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
    --method org.freedesktop.DBus.Properties.Get org.a11y.Status IsEnabled
(<false>,)
```

**AccessKit 只在 `IsEnabled` 为真时才向 AT-SPI 注册**。关着的时候，即使框架完全支持无障碍，
桌面上也看不到它的任何对象——这是**假阴性**。启用后重测，egui 立刻出现在 AT-SPI 应用列表里：

```text
启用前：桌面上的应用（13 个）… quickshell                       ← 没有 egui
启用后：桌面上的应用（14 个）… quickshell, scholium-spike-egui   ← egui 已注册
```

`a11y-probe.py` 已修正两处：① 同时按**应用名**匹配（AccessKit 应用常以 crate 名注册、窗口名为空），
原先只按窗口标题匹配会漏掉它；② 先读并打印会话无障碍开关，关闭时明确提示是假阴性而不是"没有对象"。

Iced 与 GPUI 也已在**启用状态下**重测，结论不变：仍不出现在 AT-SPI 中（它们连 accesskit 都没有，
静态证据见报告 0001/0002）。因此前两个候选的 Fail 结论成立，只是当时的**测试前提**记录有误。

## 结果

冒烟：窗口创建成功、0 条 error、SIGTERM 干净退出。截图
[artifacts/egui-window.png](../../spikes/native-ui/candidate-egui/artifacts/egui-window.png)、
输入法实验 [artifacts/egui-ime-test.png](../../spikes/native-ui/candidate-egui/artifacts/egui-ime-test.png)。

### 可访问性：通过（首个发布对象树的候选）

AT-SPI 中 `scholium-spike-egui` 的真实对象树：

```text
application name='scholium-spike-egui'
  frame name=''
    label name='正文（结构编辑 · 结构渲染）'
    label name='焦点 Text { node: NodeId(2), byte: 0 }'
    label name='源码 Source Studio [只读]'
    label name='方言 .tex / 4 行'
    label name='\\documentclass{article}\n\\begin{document}\n正文与 $a/b$ 公式\n\\end{document}\n'
    label name='预览（Typst 快速预览占位）'
    label name='占位，不冒充最终排版'
    label name='结构渲染夹具：$a/bx_1^2[123]sqrt(y)$\n'
    label name='revision 18 / 核心动作 17 / 预编辑事件 0 / IME 提交 0 / CJK 字体 已加载 / 最近：启动完成'
```

**重要限制**：树里只有 egui **控件**（`label`/`heading`）的内容。
正文区的数学结构是**自绘**的（`Painter`），**不在**对象树里——屏幕阅读器读不到分数、上下标、矩阵。
这不是 egui 的缺陷：任何框架的自绘内容都需要应用显式补充可访问描述。egui 提供了这条路径
（`Response::widget_info` / accesskit 节点构造），但候选当前没有做。
因此"结构可被辅助技术读取"这一项**仍未通过**，只是从"框架不支持"变成了"应用待实现"。

| 验收项 | 结果 | 证据 / 缺口 |
|---|---|---|
| 可访问性 | **部分 Pass** | 框架发布对象树，控件文本可读（上树证据）；缺口：自绘结构未附可访问描述。 |
| 数学结构 | **部分 Pass** | 自绘同一份 `core::layout`，截图可见分数线/上下标/矩阵/根号。缺口：未接键盘导航（已实现方向键处理，未人工验收）。 |
| 中文与 Unicode | **Pass** | 用户实机操作：fcitx5 预编辑持续到达、提交计数增长、中文进入正文，且预编辑期间核心动作不增长（预编辑不进历史、提交只产生一个动作）。缺口：长组合串下的删除与光标移动未单独验收。 |
| 源码视图 | **部分 Pass** | `TextEdit` 就绪且只在可写时出现；只读时是 `label`，与其它候选同一约束。缺口：语法高亮、大文件未测。 |
| 核心集成 | **部分 Pass** | revision / 动作数来自 core，夹具由远端 actor 注入。 |
| 团队规则 / 预览 / 大源码 / 文件恢复 | **Blocked** | 与其它候选同一原因（分别属本项未接、或阶段 0 第 2/5 项）。 |
| 构建与依赖 | **Pass** | 编译通过，无 Node/npm/WebView；`eframe`/`egui` 为 MIT OR Apache-2.0；依赖树 accesskit 含 `accesskit_unix`（Linux AT-SPI 适配器）。 |

### 输入法：**可用**（用户实测），候选侧完成接线与定位修正

**结论（经用户实机操作确认）**：egui 候选的输入法**可以正常工作**——预编辑事件持续到达、
`IME 提交` 计数增长、中文进入正文，而预编辑期间核心动作不增长（即"预编辑不进历史、提交只产生一个动作"
这条验收成立）。可输入的是**最左侧的正文区**，因为只有它被显式授予了输入焦点并请求了输入法；
源码区（只读时是 `label`）与预览区不请求输入法。

**必须记录的两次错误结论**，都是"拿一次自动测试当定论"造成的：

1. 第一轮：候选**根本没有编辑位置**——没有光标，交给输入法的 `cursor_rect` 是硬编码假矩形
   （固定 `(8,8)` 处 2×20）。这一轮不算接线。
2. 第二轮：我据此用 `ydotool` 注入拼音，只看到 `Event::Text`、`预编辑事件 0`，
   于是写下"本栈下不可用"，并声称"egui 自带 `TextEdit` 也收不到合成"。
   **该结论是错的**：用户的实机操作证明输入法可用。我的自动注入在这个共享桌面会话里**不稳定**
   （焦点会被其他窗口抢走，某次注入连按键都没到应用），一次不可靠的自动运行不能作为否定证据。
   脚本已改为：首次未观察到合成时**等待并重试一次**，最终仍无合成时只输出 `WARN`
   并说明"不能据此断定框架不支持"，不再断言失败。

**已修的两处候选侧问题**：

- **编辑位置**：`core::layout` 为文本图元记录来源 `SourceSpan { node, start_byte }`，
  新增 `Layout::caret(node, byte_offset)` 把字节偏移算成光标几何；候选据此绘制可见光标，
  并把真实矩形交给输入法。
- **候选窗定位**：egui-winit 在 Wayland 下是拿 `IMEOutput.rect` 去调 `set_ime_cursor_area` 的，
  **不是** `cursor_rect`（见 `egui-winit-0.36.2/src/lib.rs` 的 `handle_platform_output_inner`）。
  我原先把整块面板传成 `rect`，候选窗因此锚在面板左上角而不跟光标走；现在传**光标处的小矩形**。
  实测日志：`[ime] 已向平台声明输入法归属：anchor=(8,75,2,24)`。

`TextEdit` 内部的做法可作参照（`egui-0.36.2/src/widgets/text_edit/builder.rs:952`；
`Memory::owns_ime_events` 即"是否持有焦点"，`memory/mod.rs:1042`）。

### 确定性测试：无窗口驱动（不再依赖注入）

**为什么加这个**：`ydotool` 注入依赖窗口焦点，在共享桌面会话里会被抢焦点——我已经被它误导过两次。
egui 提供 `Context::run_ui(RawInput, impl FnMut(&mut Ui))`，可以在**没有窗口、没有焦点、没有输入法进程**
的条件下，用合成事件驱动**同一段输入处理代码**。应用逻辑因此从 eframe 里拆出来成为库
（`src/lib.rs`），`src/main.rs` 只留入口。

`tests/input.rs` 的 8 个测试（全部通过）：

| 测试 | 断言的核心契约 |
|---|---|
| `startup_focuses_the_document_area` | 启动后正文区持有输入焦点 |
| `preedit_does_not_touch_history_and_commit_adds_exactly_one_action` | 3 次预编辑产生 **0** 个核心动作；一次提交恰好 **1** 个 |
| `typing_inserts_text_and_advances_the_caret` | 输入进入正文且插入点前进 |
| `backspace_deletes_one_grapheme` | 退格只删一个字素 |
| `typing_zwj_emoji_then_backspace_does_not_split_grapheme` | 退格不拆开 ZWJ 序列 |
| `arrow_keys_do_not_corrupt_the_document` | 纯导航不改内容 |
| `undo_reverts_the_local_typing` | Ctrl+Z 撤销本地输入，且历史只追加（产生补偿动作） |
| `clicking_in_the_document_moves_the_caret` | 点击把插入点移到命中位置 |

**点击定位**依赖共享核心新增的命中测试：`Layout::hit_test(x, y) -> Option<(NodeId, usize)>`
（`core/src/layout.rs`），把布局坐标映射回节点与字节偏移；核心另有 3 个测试覆盖边界、
分数槽位内部与"远离文本不命中"。

两个测试写法上的坑，记下来免得再踩：① 无窗口运行时 `FullOutput.textures_delta` 必须显式 `clear()`，
否则 epaint 会在 drop 时断言；② egui 0.36 的 `Modifiers::CTRL` 里 `command` 为 `false`
（真机由 egui-winit 同时置位），且 `InputState.modifiers` 只由 `ModifiersChanged` 事件更新。

### 无障碍开关：两个都要开

复测发现**只开 `IsEnabled` 不够**，`ScreenReaderEnabled` 也必须为真，AccessKit 才会注册：

```text
只开 IsEnabled：桌面 13 个应用，无 scholium-spike-egui
两个都开：    桌面 14 个应用，含 scholium-spike-egui
```

`a11y-probe.py` 已按此更新提示。自绘结构通过 `Response::widget_info` 补的可访问描述也出现在树中
（与预览面板的文本同形，未能单独区分）。

### 性能：每帧重算布局是 O(文档规模)，这是当前实现的主要风险

候选在**每一帧**都调用 `layout_document` 并重画全部图元，因此文档规模直接决定帧开销。
用共享核心的 `fixture::build_large` 构造文档后实测（`cargo run --example bench_layout`，
以及 `tests/perf.rs` 的无窗口整帧测量）：

**布局本身**（`examples/bench_layout.rs`）：

| 段落数 | debug 稳态 | release 稳态 | 图元数 |
|---|---|---|---|
| 1 | 0.02 ms | 0.00 ms | 6 |
| 100 | 1.29 ms | 0.25 ms | 600 |
| 500 | 4.46 ms | 1.00 ms | 3 000 |
| 2000 | **14.96 ms** | **4.00 ms** | 12 000 |

**整帧**（`tests/perf.rs`，无窗口，仅逻辑侧，**不含 GPU 光栅化**）：

| 段落数 | debug 中位 | debug p95 | release 中位 | release p95 |
|---|---|---|---|---|
| 0（标准夹具） | 4.68 ms | 9.85 ms | 0.26 ms | 0.53 ms |
| 100 | 6.63 ms | 8.76 ms | 0.99 ms | 1.24 ms |
| 500 | 16.43 ms | 17.86 ms | 2.92 ms | 3.98 ms |
| 1000 | 29.20 ms | 30.52 ms | 5.84 ms | 6.09 ms |

读数与结论：

- 布局是**线性**的（约 0.18 µs/节点，release）；debug 下 2000 段单是布局就吃掉整个 16 ms 预算。
- 整帧还包含逐图元绘制，因此比布局更快撞线；debug 下 500 段已越过 16 ms。
- 上面都是**逻辑侧**的数字：debug 与 release 差约 5 倍，两者都**不含 GPU 光栅化**，
  因此不能当作达标或不达标的结论——它说明的是**架构风险**：每帧全量重算 + 全量重绘不可扩展，
  正式实现需要增量布局、按 revision 缓存、视口虚拟化与脏区重绘。
- 20 页量级（几百段）在 release 下约 1 ms 量级，勉强可接受；但这是"勉强"，不是余量。

## 第 1 项结论：候选锁定与验收结果

### 锁定版本与平台

| 项 | 值 |
|---|---|
| 候选 | **egui / eframe 0.36.2**（`egui-winit 0.36.2`，`winit 0.30.13`） |
| 可访问性 | `accesskit 0.24.1` + `accesskit_unix 0.21.1`（默认开启） |
| 对照候选 | Iced 0.14.0（含 `cosmic-text 0.15.0`）、GPUI 0.2.2、EUI-NEO（仅预筛）、Slint（被许可证阻断） |
| 平台 | Arch Linux，内核 7.2.4-arch1-2，niri 26.04（Wayland） |
| 输入法 | fcitx5 5.1.22 |
| 字体 | 系统 Source Han Serif（7 个字重） |
| 无障碍会话 | AT-SPI；测试时必须 `IsEnabled` 与 `ScreenReaderEnabled` **两个开关都开** |

### 验收结果

| 验收项 | 结果 | 证据 |
|---|---|---|
| 中文 IME | **Pass** | 用户实机操作：预编辑到达、提交计数增长、中文进正文；预编辑期间核心动作不增长 |
| 嵌套数学光标 / 编辑位置 | **Pass** | 共享核心 `Layout::caret` + 可见光标 + 输入法锚点；`tests/input.rs` 覆盖输入与退格 |
| 结构选区 / 结构编辑 | **部分 Pass** | 候选有**结构编辑命令**（包裹为分数/根式、解除、循环变体，界面有按钮），headless 测试覆盖包裹与解除；选区支持拖拽、Shift 扩展、同一叶子内删除；**跨节点选区删除未实现**，明确拒绝而不是猜 |
| 源码工作台 | **部分 Pass** | 只读 `TextEdit` + 方言与行数显示；可写模式与 reconcile 尚未接入界面 |
| 无障碍 | **Pass** | 发布对象树（`frame` + `label`，含真实文本）；自绘结构用 `widget_info` 补描述 |
| 结构渲染 | **部分 Pass** | 同一份共享布局在三个候选上可渲染并截图比对；渲染本身是**临时实现**，等 AST 后重做 |
| 点击定位 | **Pass** | `Layout::hit_test` + `tests/input.rs` 的点击用例 |
| 确定性输入契约 | **Pass** | 10 个无窗口测试（egui，含选区 2 个）与 8 个（iced）覆盖同一组契约 |
| 性能 | **部分 Pass** | 已量化：每帧全量重算，20 页量级 release 约 1 ms；**无余量**，需增量与虚拟化 |

### 结构编辑补充（本轮新增）

原候选**完全没有结构编辑命令**（Iced 候选有，被选定的 egui 反而没有）——这是验收项"结构编辑"的真实缺口。
本轮补上：

- 候选新增 `wrap(NodeKind)` / `unwrap()` / `cycle_variant()`，对应核心的
  `SemanticEdit::Wrap` / `Unwrap` / `CycleVariant`，界面提供四个按钮。
- **包裹后焦点跟随新结构**：`Wrap` 把新结构放在被包裹节点的原位置，因此从被包裹节点的父节点即可取到；
  这让"包裹 → 解除 / 循环变体"成为连贯操作（第一版焦点仍停在文本叶子上，`unwrap` 因此被拒）。
- headless 测试 2 个（包裹产生结构且不丢原文、解除移除结构且保留内容），egui 测试 10 → 12。
  测试里踩到一个前提错误：夹具**本身已含根式**，所以改用夹具中不存在的 `Delimited` 并按出现次数断言。

### 选区补充（本轮新增）

原判定把"结构选区"列为未实现。本轮补上最小可验证部分：

- **共享核心**：新增 `Selection { anchor, focus }`，`ordered()` 按**文档顺序**（前序序号 + 字节偏移）
  返回起止，与拖拽方向无关；`text_range()` 给出"同一文本叶子内"的字节区间。
  核心 2 个测试覆盖排序与跨节点返回 `None`。
- **egui 候选**：拖拽选取（自己跟踪按下点，不依赖框架的拖拽判定）、Shift+方向键扩展、
  选区高亮、以及**同一叶子内的删除**。2 个 headless 测试覆盖"拖拽产生非折叠选区"与
  "退格恰好删除选中区间且选区折叠"。
- **仍未实现**：跨节点选区的删除（需要结构级编辑，当前明确拒绝）、选区的复制/剪切、
  整节点选中的视觉反馈。

### 第 1 项判定

**egui 通过共同验收的可验证部分**，是四个候选中唯一同时具备中文输入法与可访问性的实现；
Iced 与 GPUI 因框架缺能力判 Fail（Iced 无可访问性、GPUI 无文本输入控件且无可访问性），
EUI-NEO 预筛不投入（无无障碍）、Slint 被许可证政策阻断。

明确**不承诺**的能力：鼠标拖拽选区、结构编辑的完整界面交互、正式渲染质量、增量布局。
这些必须在阶段 1 之前解决，但不构成本项 Fail。

## 与其它候选的对比

| 维度 | Iced 0.14 | GPUI 0.2.2 | EUI-NEO（仅预筛） | **egui 0.36** |
|---|---|---|---|---|
| 许可证 | MIT OR Apache | Apache-2.0 | Apache-2.0 | MIT OR Apache |
| 文本输入控件 | 有 | **无** | 有 | 有（`TextEdit`） |
| 输入法（实测） | **可用**（ydotool 注入：预编辑 30 事件、提交 1 动作） | 需自行实现 `InputHandler`，未实现 | 有合成文本，未实测 | **可用**（用户实机操作：预编辑到达、提交计数增长、中文进正文；候选仅最左正文区可输入） |
| 可访问性（实测） | **无对象树** | **无对象树** | 未实测（源码无 accesskit） | **发布对象树**（frame + label） |
| 光标/编辑位置 | 有（自绘区） | 无 | 有 | 有（`Layout::caret` + 可见光标 + 光标锚点） |
| 确定性输入契约测试 | **8 个通过** | 未做 | 未做 | **8 个通过**（另加点击定位 1 个） |
| 点击定位（点哪定位哪） | 未接入（`Layout::hit_test` 已具备） | 未做 | 未做 | **已接入并有测试** |
| 结构自绘 | canvas | canvas / 绝对定位 | 原生绘制 | `Painter` |
| 结论 | Fail（无无障碍） | Fail（无输入控件、无无障碍） | 未投入（预筛） | **Blocked（无硬性 Fail；输入法与可访问性均已具备，余项待验）** |

**当前形态**：`iced` 具备输入法与光标但没有可访问性；`egui` 两者都具备。
egui 因此是唯一同时满足这两条硬要求的候选。

## 失败与不确定性

- 无障碍结论依赖会话开关：**任何无障碍测试都必须先确认 `org.a11y.Status IsEnabled`**，
  否则结果是假阴性。这一点已写进探针输出与本节。
- 自绘结构的可访问性需要应用侧实现，工作量未估算。
- 性能、大源码、预览定位均未测。
- 本候选未启用 `persistence` 等 feature，`eframe` 的默认集已足够本次验证。

## 剩余工作

1. **输入法接线**：自绘区申请 IME 归属并设置 `o.ime`，然后重跑 `scripts/input-test.sh`，
   目标是"预编辑事件增长而核心动作不增长、提交恰好一次"。
2. **自绘结构的可访问描述**：为每个 `core::layout` 图元或整个结构附加可访问文本/角色。
3. 键盘导航与结构编辑的人工验收。
4. 预览定位（阶段 0 第 2 项）。

## 复现步骤

```bash
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"
cargo build --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
bash spikes/native-ui/candidate-egui/scripts/smoke.sh

# 无障碍测试前必须先启用会话开关，否则是假阴性
gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
  --method org.freedesktop.DBus.Properties.Set org.a11y.Status IsEnabled '<true>'
bash spikes/native-ui/candidate-egui/scripts/a11y-test.sh
# 测完可恢复：把 IsEnabled 设回 '<false>'
```
