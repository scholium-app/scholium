# 阶段 0 进度日志

> **体裁：开发日志（过去时）。** 只追加，不删改既有段落；**不得作为实现依据**。
> 当前判定见[报告 0012](../spikes/SPK-0012-exit-criteria.md)；可复现的验证证据见 [`docs/spikes/`](../spikes/README.md)。

## 2026-09-16/17：原生 UI 与出口条件（自 `docs/ROADMAP.md` 移入，现见 `docs/plan/ROADMAP.md`）

> 原生编辑逐项验收见[报告 0021](../spikes/SPK-0021-native-edit-acceptance.md)：真实 IME、双源码、数学编辑、拖选/滚动与 Unicode 桌面场景通过；完整共同验收仍 Fail，槽位选区、大源码和未接入的窗口流程继续阻断。
> Typst 进程隔离和运行时白名单的后续进展见[报告 0018](../spikes/SPK-0018-typst-worker-isolation.md)，条件 6 仍部分满足。
> 后续入口隔离进展见[报告 0017](../spikes/SPK-0017-mixed-build-isolation.md)：普通混合构建 LaTeX 已接入统一沙箱，条件 6 仍部分满足。
> 以上只记时点状态。本文件定义**交付与出口条件**；**当前逐条判定以[报告 0012](../spikes/SPK-0012-exit-criteria.md) 为准**（阶段状态的唯一来源）。

## 2026-09-16：原生 UI 验证计划的状态段（自 `docs/NATIVE_UI_VALIDATION.md` 移入，现见 `docs/plan/NATIVE_UI_VALIDATION.md`）

> **当前状态（2026-09-16，UI 框架已选定，阶段 0 未关闭）**：候选比较已完成，最终选定 **egui / eframe 0.36.2**，
> 依据见 [ADR 0006](../adr/ADR-0006-native-ui-framework.md) 与报告 0001–0004；本页保留为验证方法与判据的记录。
> 后续新增选区、源码应用和预览见[报告 0016](../spikes/SPK-0016-working-tree-status.md)；
> 最新逐项桌面复核见[报告 0021](../spikes/SPK-0021-native-edit-acceptance.md)：六组限定桌面流程通过，完整共同清单仍 Fail；槽位选区、大源码、团队/恢复/完整预览集成和无障碍边界未闭合。
> 本页是**验证方法与判据**，其中的状态叙述只是副本；阶段 0 的**当前逐条判定以[报告 0012](../spikes/SPK-0012-exit-criteria.md) 为准**。

2026-09-16：用户确认技术栈与验证顺序。**候选实测已完成、框架已选定（egui），见 ADR 0006**；
本节的顺序与判据保留为历史记录。

## 2026-09-16：混合源码与团队编辑的阶段 0 验证状态（自 `docs/MIXED_SOURCE_EDITING.md` §10 移入）

> 本节只记时点结论与边界，属副本；阶段 0 的**当前逐条判定以[报告 0012](../spikes/SPK-0012-exit-criteria.md) 为准**。

上面的验收清单由阶段 0 的两项否决性验证部分关闭（**不是全部**）：

| 上面的条目 | 阶段 0 验证 | 结论与边界 |
|---|---|---|
| 1、2、3（同语言编辑、切换屏障、epoch / 离线 / 恶意写集） | [报告 0009](../spikes/SPK-0009-team-language.md) | **Pass（内存模型 + 确定性时序）**：60 用例 60/60，屏障扫描出跨方言许可窗口重叠 0、无屏障语言变化 0；旧 epoch 包被拒且不自动回灌。**真实网络、多进程、磁盘 fsync 与解析器未验证**；写集校验是字符串抽检，**不足以宣称能阻止恶意客户端**。 |
| 4、5、6、7（混合正文/跨页表格/宏/模板/双向引用、干净环境与失败夹具） | [报告 0010](../spikes/SPK-0010-mixed-build.md) | 36/36 宿主运行通过，含六类内容的成功/失败夹具；包重建的后续验证见[报告 0016](../spikes/SPK-0016-working-tree-status.md)。**明确不在范围**：复杂浮动体、超长文档、增量构建、目录/文献/脚注的跨引擎装配。 |

**仍未实现的部分**（与验证结论无关，属阶段 1 工作）：协调协议与桥接的正式实现
（[ADR 0001](../adr/ADR-0001-mixed-source-team-editing.md) 仍为 Proposed）；LaTeX/Typst 解析器与
reconcile 的正式实现（[ADR 0008](../adr/ADR-0008-reconcile-and-toolchain-isolation.md)）；
TeX 编译沙箱的产品集成与跨平台策略（Linux spike 已补良性编译及隔离验证，见[报告 0016](../spikes/SPK-0016-working-tree-status.md)；仍有只读运行时白名单等边界）。

## 2026-09-17：保存当前进展并复测检查点 3a85aae

按用户要求先将已有工作区提交为 `3a85aae`，再进行复测。该检查点包含报告 0022 的槽位选区工作，
以及后续的连续撤销、数学槽位源码投影、结构选区反馈、预览页面/帧指标与 PDF 检查沙箱改动；
本次未逐项审查这些实现，不能将报告 0022 的旧验证范围直接套用于整个检查点。

本轮使用 `CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" cargo test --offline --manifest-path <manifest>`
分别复测四个 crate：

| manifest 所在目录 | 本轮结果 |
|---|---|
| `spikes/native-ui/core` | 45 通过 |
| `spikes/native-ui/candidate-egui` | 30 通过，2 ignored |
| `spikes/source-reconcile` | 4 通过 |
| `spikes/mixed-build` | 修正过时测试后 11 通过 |

混合构建首次复测有 1 项失败：旧 CLI 测试假定清空宿主 `PATH` 会禁用 PDF 检查工具，
但这些工具已在沙箱内使用固定路径。更新测试后，确认空 `PATH` 下 `E1-equation` 成功，
并通过将子进程 `TMPDIR` 指向 `/dev/null` 触发真实沙箱初始化失败，确认输出 `Fail` 且退出码非零。
两次构建在同一测试中顺序执行，避免共用夹具产物目录产生竞争。修改的测试通过 rustfmt，差异通过 `git diff --check`。

本轮未运行 ignored 大源码基准和预览集成测试、真实窗口/IME/AT-SPI 验收、全套混合构建夹具或完整 Clippy。
下一步仍需针对检查点新增路径完成真实窗口选区反馈、预览定位与呈现帧耗时验收；阶段 0 出口判定未升级。

## 2026-09-17：真实窗口续测、左侧排版修正与性能口径调整

完成整节点选区/撤销、AT-SPI、20 页翻页/缩放/段落定位等桌面续测，报告与原始证据见
[0023](../spikes/SPK-0023-native-layout-preview.md)。左侧为 core + egui 自绘，右侧才是 Typst 预览；
窗口 CPU 计时覆盖三个面板，不能说成左侧单独渲染或 Typst 编译时间。

用户指出左侧错位并允许重写，同时明确撤销没有依据的固定 15/16 ms 门槛。
本轮先修正数学槽位仅画首个子节点、根号基线、负坐标归一化高度及 egui 字体实际基线偏移，
补数学间距和分数留白。没有重写为完整排版引擎，真实字宽、完整选区几何和可伸缩数学符号仍待做。

core 47 项、egui 30 项通过（2 ignored），egui Clippy 通过。更新后 release CPU 组内 p95 为
7.608–9.126 ms，Typst 编译加栅格化 2154 ms；只报告数据，不按固定帧预算宣布 Pass/Fail。
桌面首轮 IME 提交超时与选区场景焦点变化均保留证据，随后两项定向复测通过。
下一步优先统一字体测量、绘制和光标几何，再处理完整结构外框；阶段 0 状态不升级。
