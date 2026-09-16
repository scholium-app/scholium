# 阶段 0 验证报告

> 真实窗口复核见[报告 0019](0019-native-window-review.md)：源码/剪贴板/预览闭环通过；正文无障碍接口、结构视口和实际帧性能仍未闭合。
> Typst 进程隔离和运行时白名单的后续进展见[报告 0018](0018-typst-worker-isolation.md)，条件 6 仍部分满足。
> 后续入口隔离进展见[报告 0017](0017-mixed-build-isolation.md)：普通混合构建 LaTeX 已接入统一沙箱，条件 6 仍部分满足。
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

## 一键复现

```bash
cd <repo>
bash spikes/verify-stage0.sh          # 跑全部（七项 + 出口条件）
bash spikes/verify-stage0.sh core     # 分段：core / ui / typst / reconcile / items / packages / preview / security / license / web
```

脚本逐项打印 `PASS`/`FAIL` 并在结尾汇总，非零退出码或日志中显式的 `FAIL` 均判失败；
未知分段名也会报错。每次运行的完整日志保存在脚本打印的独立临时目录中。
这只是自动检查结果，不能代替阶段出口判定；门禁修复历史见 [报告 0015](0015-verification-review.md)，本次复测见 [报告 0016](0016-working-tree-status.md)。
`all` 已包含 `packages` 与 `preview`：需要 Linux bubblewrap、TeX Live、Poppler 和 Typst CLI 0.15.1；
CLI 路径用 `SCHOLIUM_TYPST_BIN` 指定，默认 `/tmp/scholium-typst-toolchain/bin/typst`。
`reconcile` 分段只跑程序夹具，事务会话测试还需单独执行 `cargo test`，见报告 0016。
回归测试：`bash spikes/tests/verify-stage0.sh`，以及 mixed-build 的 `cargo test --release --offline`。两点注意：

- `cargo deny` 只能在**每个 crate** 运行（仓库根 workspace 没有成员），且 `advisories`
  需要联网获取 RustSec 数据库——离线环境跑不了，脚本会明确提示而不是假装通过。
- 无障碍测试需要会话开关 `IsEnabled` 与 `ScreenReaderEnabled` **都开**，否则 AccessKit 不注册、
  结果是假阴性；输入法的自动注入在共享桌面会话里不稳定，**不能**据一次运行否定框架。

## 语言与工具约束

验证代码遵守 `AGENT.md` 的硬性约束：不使用 npm/Node.js、JavaScript/TypeScript 编辑器或
WebView/Electron/Tauri 作为应用 UI 或验证依赖。旧 ProseMirror/浏览器编辑器实验已于
2026-09-16 从仓库删除，不能作为任何结论的证据，也不能据此重新引入 Web 编辑器路线。

## 当前状态

最新工作区复测与新增实现统一见[报告 0016](0016-working-tree-status.md)；
下表保留各项原始报告的验证范围，新增能力以 0016 为准。阶段 0 尚未关闭。

| 验证项 | 报告 | 结论 |
|---|---|---|
| 第 1 项 原生 UI（候选 1） | [0001-native-ui-iced.md](0001-native-ui-iced.md) | **Iced 结论 Fail**：核心 26 个测试全过、渲染后端与真实输入法均有可复现证据；可访问性不通过（框架无 accesskit）。 |
| 第 1 项 原生 UI（候选 2） | [0002-native-ui-gpui.md](0002-native-ui-gpui.md) | **GPUI 结论 Fail**：结构渲染可行且与 Iced 可比，但核心**没有文本输入控件**，且同样无 accesskit。 |
| 第 1 项 原生 UI（候选预筛） | [0003-native-ui-prescreen.md](0003-native-ui-prescreen.md) | EUI-NEO 许可证 Apache-2.0 但无无障碍（不投入）；Slint 有无障碍但许可证被政策阻断；**egui 三项全过，进入实现**。 |
| 第 1 项 原生 UI（候选 3，**选定**） | [0004-native-ui-egui.md](0004-native-ui-egui.md) | **egui 结论通过共同验收的可验证部分**：可访问性通过（发布对象树）、输入法可用（用户实机确认）、结构编辑与选区有命令与 headless 测试（包裹/解除/循环变体、拖拽选取、同叶删除）、点击定位与性能见[报告 0005](0005-typst-mapping.md)与报告 0004 的性能节。**唯一同时具备输入法与可访问性的候选**；锁定 eframe/egui 0.36.2 + winit 0.30.13 + accesskit 0.24.1，平台 Arch Linux / niri 26.04 / fcitx5 5.1.22。后续已补文本端点跨节点删除、纯文本复制/剪切、源码应用和后台预览（[0016](0016-working-tree-status.md)）；槽位端点、完整桌面复验、正式渲染质量与增量布局仍未完成。 |
| 第 2 项 Typst 映射 | [0005-typst-mapping.md](0005-typst-mapping.md) | **结论 Pass（机制，含行内结构坐标）**：20 页 668 ms 编译；2050/2050 锚点解析；分数与上下标各有区间且反查 40/40 命中；增量编译 200 ms（冷的 30%）。已渲染首页叠加锚点核对；多页覆盖与文本字符级映射（误差 0.24 pt）已验证。附带结论：预览必须异步去抖，不能按帧预算设计。 |
| 第 3 项 源码 reconcile | [0006-source-reconcile.md](0006-source-reconcile.md) | **结论 Pass（限定范围）**：受支持结构的受支持编辑在 LaTeX/Typst 两种方言下 8/8 用例通过，未知语法落成 Raw 且原文逐字保留、往返一致；结构级重构与增删行一律报冲突。 |
| 出口条件 安全 | [0011-untrusted-input.md](0011-untrusted-input.md) | 原始报告发现 TeX 配置无法阻止越界读取；后续已补可运行的 Linux 沙箱和正负对照，当前实测及运行时挂载边界见[0016](0016-working-tree-status.md)。 |
| 第 6 项 团队语言协调 | [0009-team-language.md](0009-team-language.md) | **结论 Pass（内存模型 + 确定性时序）**：60 用例 60/60，含切换屏障扫描（重叠 0）、epoch 隔离不自动回灌、重启恢复字段级一致、分区/旧包/19 类恶意写集逐条拒绝。真实网络、多进程、磁盘与解析器未验证。 |
| CRDT 引擎预筛 | [0013-crdt-prescreen.md](0013-crdt-prescreen.md) | Loro 1.16 / Yrs 0.27 / Automerge 0.11 均为 MIT 且无 C 依赖。 |
| CRDT 引擎对比 | [0014-crdt-engines.md](0014-crdt-engines.md) | **结论 Pass，选定 Loro 1.16.0**：只有它同时具备可移动树与能过滤远端输入的内置 undo，且都端到端跑通；三者收敛但并发插入次序不同。**性能未对比**，留待阶段 1 首个迭代。 |
| 第 4 项 CRDT 赛马 | [0007-crdt-race.md](0007-crdt-race.md) | **结论 Pass（自研最小实现）**：136 用例逐条通过——收敛 14/14、本地 undo 不回滚远端 5/5、快照 5/5、10 万动作 3.7 s（重复运行 2.8–6.0 s；堆增量 87.5 MiB）、110 轮随机化全部收敛。该自研 spike 不覆盖第三方引擎；引擎选型见报告 0014，树语义边界仍未覆盖。 |
| 第 5 项 构建/恢复 | [0008-build-recovery.md](0008-build-recovery.md) | **结论 Pass（机制）**：15 个用例组全通过（隔离构建、WAL 正常/截断/损坏恢复、外部冲突拒写、169 条随机截断断言）；SIGKILL 撕裂恢复到记录边界。缺口：Typst 未产出 PDF、夹具仅 1 页、未覆盖产品真实写盘路径。 |
| 第 7 项 混合构建 | [0010-mixed-build.md](0010-mixed-build.md) | **结论 部分 Pass**：19 个夹具按适用宿主执行，36/36 运行通过；六类内容各有成功/失败夹具与三类支持矩阵；两宿主都既做宿主也做被嵌入方；最终件栅格图像 0 个；引用振荡第 3 轮停止且不发布正式产物。标准包/混合快照包的干净环境重建已新增验证（[0016](0016-working-tree-status.md)）；**未验证**：行内矢量嵌入、组件内部符号与链接保真、超长文档。 |

**测试前提**：无障碍测试必须在会话 `org.a11y.Status IsEnabled = true` 下进行。
关闭时 AccessKit 类框架不会注册，结果是**假阴性**——本目录早期记录曾因此出错，已在报告 0001/0002 更正。

阶段 0 七项清单与出口条件见 [路线图](../ROADMAP.md)。
