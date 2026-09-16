# 阶段 0 验证报告

本目录保存阶段 0 否决性验证的报告。设计中的"待验证"只有在这里留下可复现证据后才能关闭，
不能用在实现里补写生产脚手架的方式替代。

## 规则

- 报告按 `NNNN-short-title.md` 命名，模板见 [0000-template.md](0000-template.md)。
- 每份报告对应 [路线图](../ROADMAP.md) 阶段 0 的一个验证项，必须在 `docs/ROADMAP.md` 对应条目上链接。
- 结论只能是 `Pass`、`Fail`、`Blocked`。缺系统环境、缺工具链或缺夹具记 `Blocked`，不能记 `Pass`。
- 报告必须包含锁定版本、真实平台、可复现命令和证据位置；只给结论不给证据视为未完成。
- 结论需要固化选型时另立 ADR，并在报告和 ADR 之间双向链接。ADR 只记录决策与替换条件，
  原始数据留在本目录。
- 原生 UI 各候选的报告还必须满足 [原生 UI 验证计划](../NATIVE_UI_VALIDATION.md) 第 5 节的字段要求。
- 可执行验证代码放在仓库根 `spikes/<name>/`，使用 Rust 或必要原生代码构建。

## 语言与工具约束

验证代码遵守 `AGENT.md` 的硬性约束：不使用 npm/Node.js、JavaScript/TypeScript 编辑器或
WebView/Electron/Tauri 作为应用 UI 或验证依赖。旧 ProseMirror/浏览器编辑器实验已于
2026-09-16 从仓库删除，不能作为任何结论的证据，也不能据此重新引入 Web 编辑器路线。

## 当前状态

| 验证项 | 报告 | 结论 |
|---|---|---|
| 第 1 项 原生 UI（候选 1） | [0001-native-ui-iced.md](0001-native-ui-iced.md) | **Iced 结论 Fail**：核心 26 个测试全过、渲染后端与真实输入法均有可复现证据；可访问性不通过（框架无 accesskit）。 |
| 第 1 项 原生 UI（候选 2） | [0002-native-ui-gpui.md](0002-native-ui-gpui.md) | **GPUI 结论 Fail**：结构渲染可行且与 Iced 可比，但核心**没有文本输入控件**，且同样无 accesskit。 |
| 第 1 项 原生 UI（候选预筛） | [0003-native-ui-prescreen.md](0003-native-ui-prescreen.md) | EUI-NEO 许可证 Apache-2.0 但无无障碍（不投入）；Slint 有无障碍但许可证被政策阻断；**egui 三项全过，进入实现**。 |
| 第 1 项 原生 UI（候选 3） | [0004-native-ui-egui.md](0004-native-ui-egui.md) | **egui 结论 Blocked（首个无硬性 Fail）**：可访问性通过（发布对象树）、输入法可用（用户实机确认）、结构渲染通过；结构编辑交互、性能与预览定位未验收。**目前唯一同时具备输入法与可访问性的候选。** |
| 第 2 项 Typst 映射 | [0005-typst-mapping.md](0005-typst-mapping.md) | **结论 Pass（机制）**：20 页 637 ms 编译成功；NodeId ↔ 预览位置 257/257 双向解析；增量编译 183 ms（冷的 29%）。附带结论：预览必须异步去抖，不能按帧预算设计。 |
| 第 3–7 项 | 尚无 | 待执行 |

**测试前提**：无障碍测试必须在会话 `org.a11y.Status IsEnabled = true` 下进行。
关闭时 AccessKit 类框架不会注册，结果是**假阴性**——本目录早期记录曾因此出错，已在报告 0001/0002 更正。

阶段 0 七项清单与出口条件见 [路线图](../ROADMAP.md)。
