# Spike 0002：原生 UI 候选 — GPUI

> **有效性（2026-09-17 登记）**：候选层结论，只记录 GPUI 的 Fail 与原因（无文本输入控件且无 accesskit）；框架选型已由 [ADR 0006](../adr/0006-native-ui-framework.md) 固定为 egui。阶段 0 当前判定见 [报告 0012](0012-exit-criteria.md)。

- 结论：**Fail**（可访问性不通过；且 GPUI 核心**没有文本输入控件**，源码视图与输入法验收无法在候选层完成）
- 对应验证项：[路线图阶段 0 第 1 项](../ROADMAP.md)
- 日期：2026-09-16
- 执行者：ation_ciger
- 关联 ADR：[0002 原生技术栈与 UI 验证顺序](../adr/0002-native-ui-validation-order.md)；最终选型 ADR 待完成

> 按 [原生 UI 验证计划](../NATIVE_UI_VALIDATION.md) 第 1 节的顺序验证第二个候选。结论是 Fail，
> 但**失败原因与 Iced 不同**：Iced 缺的是无障碍，GPUI 缺的是无障碍**加**文本编辑基础设施。

## 问题与判据

与 [报告 0001](0001-native-ui-iced.md) 相同：判据是验证计划第 4 节的验收表，候选使用同一最小工程、
**同一份夹具与同一份布局**（`scholium-spike-core`）。

## 环境

| 项 | 值 |
|---|---|
| rustc / cargo | 1.98.0-nightly（bd08c9e71 / a595d0da6），edition 2024 |
| 平台 | Linux，Wayland，合成器 niri |
| GPU | 与报告 0001 同一台机器（Intel UHD + NVIDIA RTX 4060，Vulkan 可用） |
| gpui 版本 | `=0.2.2`（crates.io 发布版，上游仓库 zed-industries/zed） |
| 依赖规模 | **711 个包**（Iced 候选为 389 个） |
| 构建时间 | 全树 `cargo build` 3 分 29 秒 |

## 方法与夹具

- 复用报告 0001 建立的共用核心与夹具；GPUI 候选通过 path 依赖引用同一个 `core/`。
- 结构渲染绘制**同一份** `core::layout` 结果，因此与 Iced 的结构渲染可直接比较。
- 渲染方式：`Item::Text` → 绝对定位文本元素，`Item::Rule` → 绝对定位实心矩形。
  Iced 用的是 `canvas`；机制不同，但输入相同、输出可比。
- 冒烟脚本：`candidate-gpui/scripts/smoke.sh`（按 **PID 匹配**窗口——GPUI 在 Wayland 下窗口标题为空，
  标题匹配不可靠）。
- 可访问性脚本：`candidate-gpui/scripts/a11y-test.sh`，复用报告 0001 的 `a11y-probe.py`。

## 结果

模型层 26 个测试（20 验收 + 6 布局，来自共用核心）：全部通过。
候选冒烟：**窗口创建成功、合成器已列出、0 条 error、SIGTERM 干净退出**。
证据截图：[artifacts/gpui-window.png](../../spikes/native-ui/candidate-gpui/artifacts/gpui-window.png)。

| 验收项（计划第 4 节） | 结果 | 证据 / 缺口 |
|---|---|---|
| 中文与 Unicode | **Blocked** | 未实现文本输入，无从验证预编辑/提交。见"核心没有文本输入控件"。 |
| 数学结构 | **部分 Pass** | 绘制同一份共享布局，截图可见分数线、上下标错位、矩阵网格、根号上横线，与 Iced 输出一致。缺口：结构导航未接键盘事件。 |
| 源码视图 | **Fail（未实现）** | GPUI 核心无文本输入控件，只读展示可用，编辑必须自行实现。 |
| 团队规则 | **Blocked** | 依赖文本输入。 |
| 核心集成 | **部分 Pass** | 界面 revision 与动作数来自 core，夹具由远端 actor 注入。 |
| 预览 | **Blocked** | 仅占位。 |
| 大源码 | **Blocked** | 未测。 |
| 可访问性 | **Fail** | 候选不在 AT-SPI 桌面对象树中；`gpui 0.2.2` 的 `Cargo.toml` 没有 accesskit 依赖、没有相关 feature，源码 0 处引用，**整棵依赖树 accesskit 计数为 0**。同期其他应用（Avalonia、Electron、Thunar、WPS）均可正常发布对象。 |
| 文件恢复 | **Blocked** | 核心不含持久化；属阶段 0 第 5 项。 |
| 构建与依赖 | **Pass** | 全树编译通过，无 Node/npm/WebView。`gpui` 自身许可 Apache-2.0；依赖树许可分布为 MIT OR Apache-2.0（320）、MIT（155）、Apache-2.0 OR MIT（70）、Apache-2.0（21）、Unicode-3.0（18）、BSD-3-Clause（10）等，**未见 GPL/AGPL 类**，符合 `AGENT.md` 许可证政策。 |

## 关键发现

### 1. GPUI 核心没有文本输入控件

`gpui/src/elements/` 只有 `div`、`text`、`canvas`、`list`、`img`、`svg`、`surface`、`anchored` 等；
Zed 的文本输入控件在**另一个 `ui` crate**里，而它没有随 `gpui` 发布（GPUI 依赖树中不存在 `ui` crate）。

后果：在 GPUI 上做编辑器，**文本编辑、光标、选区、输入法全部要自己实现**。API 路径是存在的
（`InputHandler` / `ElementInputHandler` / `PlatformInputHandler`，`window.platform_window.update_ime_position(...)`
也已具备），但这是"从零写一个文本编辑器"，不是"接线"。相比之下 Iced 自带 `text_input` /
`text_editor`，自绘区域只需额外请求输入法。

这条直接决定：**GPUI 的工作量显著高于 Iced**，而且风险集中在最难的部分（输入法与文本编辑）。

### 2. GPUI 也没有无障碍支持（更正报告 0001 的一处引用）

`gpui 0.2.2`：`Cargo.toml` 中 `grep -i accesskit` 无结果，feature 列表无相关项，`src/` 中 0 处引用，
`Cargo.lock` 全树 accesskit 计数为 0。AT-SPI 实测同样读不到候选窗口。

**更正**：[报告 0001](0001-native-ui-iced.md) 曾把"GPUI 有基于 AccessKit 的可访问性实现"列为参考。
那是 **Zed 仓库源码**的情况，**已发布的 `gpui` crate 里没有**。因此：

- 可访问性**不能**用来区分 Iced 与 GPUI——两者都是 Fail；
- 我之前据此提出的"可访问性成为区分候选的关键判据"结论作废。

**测试前提更正（2026-09-16 补充）**：本报告最初的 AT-SPI 实测是在**会话无障碍关闭**
（`org.a11y.Status IsEnabled = false`）下做的。AccessKit 类框架此时不会注册，所以那种条件下的
"读不到"不能独立证明框架不支持无障碍。在**启用无障碍**后重测，GPUI 仍不出现在 AT-SPI 应用列表中，
与静态证据一致（`gpui` 全树无 accesskit，本就没有可注册的实现）。
详见[报告 0004](0004-native-ui-egui.md) 的"测试方法错误"一节。

### 3. 结构渲染可行且与 Iced 可比

两个候选绘制同一份 `core::layout`，输出的结构一致。这说明"结构渲染"这一项对两个框架都不构成瓶颈，
差异在文本输入、输入法与无障碍。

## 与 Iced 的对比

| 维度 | Iced 0.14 | GPUI 0.2.2 |
|---|---|---|
| 依赖包数 | 389 | 711 |
| 全树构建 | 27 s | 3 min 29 s |
| 内置文本输入控件 | 有（`text_input` / `text_editor`） | **无** |
| 输入法支持 | 控件自动请求；自绘区用自定义控件请求即可 | 需自行实现 `InputHandler` |
| 可访问性 | 无 accesskit | 无 accesskit |
| 结构自绘 | `canvas`（非默认 feature） | `canvas` 与绝对定位均可 |
| 许可证 | MIT OR Apache-2.0 | Apache-2.0，依赖树均宽松 |

## 失败与不确定性

- 结论 Fail 的两条依据（无文本输入控件、无无障碍）都已用源码与实测确认，不是推测。
- GPUI 的文本输入一旦自行实现，验收表其余各项才可能通过——但那意味着本项验证实际是在验证
  "我们自己的文本编辑器"，而不是"框架能力"，这会削弱候选比较的意义，值得在选型时明确。
- 窗口在 Wayland 下标题为空，脚本改用 PID 匹配；这一点会影响任何依赖窗口标题的自动化。

## 对设计的影响

- **两个候选都无法满足无障碍要求**，无障碍因此必须作为独立工作项（自研 AT-SPI/AccessKit 集成，
  或改用具备该能力的框架），不能指望候选框架自带。
- 候选比较的重点应从"能不能画结构"转向"文本编辑与输入法要写多少"。
- 若继续验证第三、四个候选（EUI-NEO、Slint/egui），应优先查证其**文本输入控件**与**无障碍**两项，
  其余项大概率不是瓶颈。

## 剩余工作

1. **文本输入**：若要继续 GPUI，需要实现 `InputHandler`（含光标、选区、输入法），工作量需要先估算。
2. **结构导航**：接键盘事件（本候选未接）。
3. **无障碍**：全项目级工作项，见上。

## 复现步骤

```bash
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"

# 1. 构建（首次会编译 711 个包）
cargo build --manifest-path spikes/native-ui/candidate-gpui/Cargo.toml

# 2. 冒烟 + 窗口截图（只截本应用窗口）
bash spikes/native-ui/candidate-gpui/scripts/smoke.sh

# 3. 可访问性检查
bash spikes/native-ui/candidate-gpui/scripts/a11y-test.sh
```
