# Spike 0004：原生 UI 候选 — egui

- 结论：**Blocked**（可访问性首次**通过**，结构渲染通过；输入法与结构编辑待接线）
- 对应验证项：[路线图阶段 0 第 1 项](../ROADMAP.md)
- 日期：2026-09-16
- 执行者：ation_ciger
- 关联 ADR：[0002 原生技术栈与 UI 验证顺序](../adr/0002-native-ui-validation-order.md)

> **这是四个候选里第一个没有硬性 Fail 的。** Iced 与 GPUI 是因为**框架不具备**能力而 Fail；
> egui 具备能力，只是候选实现尚未接完，因此记 Blocked。这个区分对选型很关键。

## 为什么验证 egui（而不是先做 Slint）

按验证计划第 1 节，末组"根据前三者的缺口决定先验证哪个"。前三者（含预筛的 EUI-NEO）在
**同一处**缺口上一致：都不具备可访问性。按计划自身的规则，末组优先验证具备该能力的 egui。
预筛依据见 [报告 0003](0003-native-ui-prescreen.md)。

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
| 中文与 Unicode | **Blocked** | 输入法未接线：注入拼音后 `预编辑事件 0 / IME 提交 0`，文本以 ASCII 经 `Event::Text` 进入（见下）。 |
| 源码视图 | **部分 Pass** | `TextEdit` 就绪且只在可写时出现；只读时是 `label`，与其它候选同一约束。缺口：语法高亮、大文件未测。 |
| 核心集成 | **部分 Pass** | revision / 动作数来自 core，夹具由远端 actor 注入。 |
| 团队规则 / 预览 / 大源码 / 文件恢复 | **Blocked** | 与其它候选同一原因（分别属本项未接、或阶段 0 第 2/5 项）。 |
| 构建与依赖 | **Pass** | 编译通过，无 Node/npm/WebView；`eframe`/`egui` 为 MIT OR Apache-2.0；依赖树 accesskit 含 `accesskit_unix`（Linux AT-SPI 适配器）。 |

### 输入法：框架路径存在，候选未接线

注入 "nihao" 后，文本以**普通 ASCII** 落进正文，预编辑与提交计数都是 0 —— 说明
**输入法根本没有开启**，按键走的是 `Event::Text`。原因与 Iced 候选当初的问题同源：
egui 只在**有控件请求时**才启用输入法。`TextEdit` 在获得焦点后这样请求：

```rust
// egui-0.36.2/src/widgets/text_edit/builder.rs:952
if ui.memory(|mem| mem.owns_ime_events(id)) {
    ui.output_mut(|o| o.ime = Some(crate::output::IMEOutput {
        purpose: IMEPurpose::Normal,
        rect: /* 编辑区 */, cursor_rect: /* 光标 */, should_interrupt_composition: false,
    }));
}
```

`Memory::owns_ime_events(Id)` 决定哪些事件归属该 id（`egui-0.36.2/src/memory/mod.rs:1042`）。
**自绘结构编辑器必须自己申请 IME 归属并设置 `o.ime`**，否则收不到 `Event::Ime(ImeEvent::Preedit/Commit)`。
这是一段有界工作，尚未实施。

## 与其它候选的对比

| 维度 | Iced 0.14 | GPUI 0.2.2 | EUI-NEO（仅预筛） | **egui 0.36** |
|---|---|---|---|---|
| 许可证 | MIT OR Apache | Apache-2.0 | Apache-2.0 | MIT OR Apache |
| 文本输入控件 | 有 | **无** | 有 | 有（`TextEdit`） |
| 输入法 | 需自绘区自行请求 | 需自行实现 `InputHandler` | 有合成文本 | 需自绘区自行请求（路径已知） |
| 可访问性 | **无 accesskit** | **无 accesskit** | **无** | **默认开启，含 `accesskit_unix`** |
| 结构自绘 | canvas | canvas / 绝对定位 | 原生绘制 | `Painter` |
| 结论 | Fail | Fail | 未投入（预筛） | **Blocked（首个无硬性 Fail）** |

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
