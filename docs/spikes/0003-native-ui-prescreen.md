# Spike 0003：原生 UI 候选预筛（EUI-NEO、Slint、egui）

> **有效性（2026-09-17 登记）**：预筛结论，不是候选验收结论；框架选型已由 [ADR 0006](../adr/0006-native-ui-framework.md) 固定为 egui。阶段 0 当前判定见 [报告 0012](0012-exit-criteria.md)。

- 结论：**预筛结论**（不是候选验收结论）：EUI-NEO 不投入、Slint 被许可证政策阻断、
  **egui 通过预筛并进入实现验证**
- 对应验证项：[路线图阶段 0 第 1 项](../plan/ROADMAP.md)
- 日期：2026-09-16
- 执行者：ation_ciger

## 为什么先预筛

前两个候选各花了几小时实现适配层，最后都栽在同一件事上（框架不自带无障碍）。继续按顺序硬做，
很可能重复这个过程。因此在写适配层之前，先用**最小成本**查证三件决定成败的事：

1. **许可证**是否符合 `AGENT.md` 的许可证政策；
2. 是否有**文本输入控件**（没有的话，编辑器要从零写）；
3. 是否支持**无障碍**（前两个候选都在这里 Fail）。

只有三项都不构成硬阻断，才投入实现。预筛依据全部是可复核的命令输出，不是印象。

## 预筛结果

| 候选 | 许可证 | 文本输入控件 | 输入法 | 无障碍 | 处置 |
|---|---|---|---|---|---|
| **EUI-NEO**（C++，克隆自 `sudoevolve/EUI-NEO`） | **Apache-2.0** ✓ | 有（`components/input.h`，含 `compositionText`） | 有合成文本状态 | **无**（见下） | **不投入** |
| **Slint 1.17.1** | **GPLv3 / 商业 / royalty-free** ✗ | 有 | 有 | 有（依赖树 accesskit 14 条） | **政策阻断** |
| **egui / eframe 0.36.2** | **MIT OR Apache-2.0** ✓ | `TextEdit` ✓ | `egui-winit` 处理 `Ime::Preedit` / `Ime::Commit` ✓ | **`accesskit` 默认开启，含 `accesskit_unix`** ✓ | **投入验证** |

### 依据

**EUI-NEO**

```text
LICENSE 首行 = "Apache License, Version 2.0"；GPL 提及 0 次
components/ = input.h text.h markdown.h panel.h ...（有输入控件）
core/input/input_state.h: compositionText / compositionTextStates()  ← 有输入法合成
grep -rli 'accesskit|a11y|atspi|screen_reader' → 0 个文件
grep -rn 'accessible' 的 6 处命中全部位于 3rd/freetype 与 3rd/glfw 的文档注释里，与无障碍无关
```

顺带解决了此前的两处不确定：**EUI-NEO 许可证确认为 Apache-2.0**（原先在报告 0001 里标为"待核实"），
且上游就是 `sudoevolve/EUI-NEO`（克隆成功，最新提交 2026-09-04）。

不投入的理由：无障碍缺失（与 Iced、GPUI 同样的 Fail），而它是 C++/CMake 框架，
接入 Rust 核心需要 FFI 层，成本显著高于 Rust 候选；用高成本换一个确定 Fail 没有意义。

**Slint**

```text
依赖树 accesskit 条目 14 条（有可访问性实现）
许可证：GPLv3 / 商业许可 / Slint Royalty-free —— 均非宽松
```

按 `AGENT.md` 许可证政策，"自定义 royalty-free / 受限许可"**默认禁止**，需要 ADR 才能引入。
因此在用户明确决定改用 GPLv3 或购买商业许可之前，Slint 不具备可验证性。
这一条在报告 0001 的 3.1 节已预告，现在有依赖树证据。

**egui / eframe**

```text
eframe/Cargo.toml:
    accesskit = ["egui-winit/accesskit"]
    default   = ["accesskit", ...]        ← 默认开启
Cargo.lock 中 accesskit 条目 9 条，含 accesskit_unix（Linux → AT-SPI 适配器）
egui/src/widgets/text_edit/               ← 自带多行文本编辑
egui-winit/src/lib.rs:722  winit::event::Ime::Preedit → egui::Event::Ime(ImeEvent::Preedit)
egui-winit/src/lib.rs:758  winit::event::Ime::Commit  → egui::Event::Ime(ImeEvent::Commit)
egui-winit/src/lib.rs:1153 window.set_ime_allowed(allow_ime)
```

三项全部通过，且是 Rust crate、宽松许可，接入成本与 Iced 同量级。

## 对候选顺序的影响

验证计划规定的顺序是 Iced → GPUI → C++ EUI-NEO → Slint 或 egui，并写明末组"根据前三者的缺口
决定先验证哪个"。前三者（含预筛的 EUI-NEO）在同一处的缺口是一致的：**都不发布可访问对象树**。
因此按计划自身的规则，末组优先验证**具备无障碍能力**的 egui，而不是先做成本更高的 Slint。

## 不确定性

- 预筛只查静态能力，不证明 egui 的 accesskit 在本机真的能跑通 AT-SPI——那需要实测，
  正是接下来要做的事。
- Slint 的阻断是**政策**而非技术：若决定调整项目许可或购买商业许可，应另立 ADR 后重新预筛。
- EUI-NEO 的输入法只是"有合成文本状态"，未验证其与 GLFW 预编辑回调的接线质量；
  若将来把无障碍问题解决掉，需要重新评估。

## 复现步骤

```bash
# EUI-NEO：许可证与能力核查
git clone --depth 1 https://github.com/sudoevolve/EUI-NEO.git spikes/native-ui/.vendor/eui-neo
head -3 spikes/native-ui/.vendor/eui-neo/LICENSE
grep -rn 'compositionText' spikes/native-ui/.vendor/eui-neo/core/input/input_state.h

# Slint / egui：依赖树与默认 feature 核查
cd spikes/native-ui/.vendor/prescreen
CARGO_HOME=../../.cargo-home cargo add eframe slint
grep -c 'name = "accesskit' Cargo.lock
grep -n 'accesskit' "$CARGO_HOME"/registry/src/*/eframe-0.36.2/Cargo.toml
```
