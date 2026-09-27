# 报告 0048：E1 结构光标改造完成验收

- 结论：**Pass（E1 范围）**。体验契约 31/32、原生窗口 8/8，相对报告 0046 基线**零回归**。
  唯一未通过项 `a07` 是**已登记的规范冲突**，不是未完成项（见下）。
- 对应：[E1 实施计划](../plan/E1_STRUCTURAL_CURSOR.md)、[ADR 0031](../adr/ADR-0031-structural-cursor.md)、
  [报告 0046](SPK-0046-editor-usability-audit.md)（本改造的驱动证据）、
  [报告 0047](SPK-0047-structural-cursor-verification.md)（S2 独立验证）。
- 日期：2026-09-27。
- 执行者：cursor-impl（实现）、Lead（逐步独立复核）。
- 关联 ADR：[ADR 0031](../adr/ADR-0031-structural-cursor.md)（双向链接）。

**报告 0046 的 Fail 结论与证据原样保留**，本报告是修复后的新判定，
不改写历史结论（[spikes 规则](README.md)）。

## 问题与判据

报告 0046 判定当前页面编辑器在结构性内容损坏上 Fail：`$`、`*`、`_` 是**投影语法**，
却被当作正文字符参与回车与删除，用户一次 Enter 就能把 `$alpha$` 拆成字面文本并保存。

本报告的判据沿用报告 0046 建立的同一套验收，**不新建口径**：

| 判据 | 目标 |
|---|---|
| 体验契约 | 除 `a07` 外全部通过（31/32） |
| 原生窗口 | 8/8 全通过 |
| 回归 | 报告 0046 基线中已通过（ok）的用例**一项都不得变 FAILED** |
| 门禁 | fmt / 规模 / clippy `-D warnings` / rustdoc 全绿；`cargo test --workspace` 全绿 |

## 环境

- 平台：Linux、Xvfb X11、debug 构建；`xvfb-run -a -s "-screen 0 1280x1024x24"`。
- 原生脚本需软件 GL（见 [证据](evidence/SPK-0048/summary.md) 与本仓库 `scripts/audit-editor.py`）：
  宿主装有 NVIDIA GLX vendor 库时，Xvfb 会失败；`scripts/audit-editor.py` 现已内置
  `LIBGL_ALWAYS_SOFTWARE=1` 与 `__GLX_VENDOR_LIBRARY_NAME=mesa`，否则原生阶段无法运行。
- 复现命令见下"复现步骤"。

## 方法与夹具

分六步实施（[E1 实施计划](../plan/E1_STRUCTURAL_CURSOR.md)），每步独立可交付：

| 步 | 内容 | 结果 |
|---|---|---|
| S1 | ADR 0031 + 坐标契约 | 13/32（无行为改动） |
| S2 | 结构坐标 `Caret` + 命令层 | 16/32，原生 5/8 |
| S3 | 结构编辑语义（公式/粗体回车） | 21/32，原生 7/8 |
| S4 | 几何按 revision 解释 + 崩溃修复 | 25/32 |
| S5 | 回显按结构投影 | 30/32 |
| S6 | 最小本地撤销 | **31/32，原生 8/8** |

三层坐标各司其职：语义位置 `Caret{block, inline, offset}` 是编辑权威；
投影字节只在构造 `BlockEdit` 时临时产生；页面几何只用于命中与绘制。

## 结果

| 判据 | 结果 | 证据 |
|---|---|---|
| 体验契约 | **31/32**（基线 13/32） | [summary.md](evidence/SPK-0048/summary.md)、[usability.log](evidence/SPK-0048/usability.log) |
| 原生窗口 | **8/8**（基线 4/8） | [native-results.json](evidence/SPK-0048/native-results.json) |
| 回归 | **0 项**（逐条比对基线 32 项） | 见 §零回归自查 |
| `cargo test --workspace` | **111 passed / 0 failed**（基线 81 项） | [baseline.log](evidence/SPK-0048/baseline.log) |
| fmt / 规模 / clippy / rustdoc | 全部 exit 0 | [fmt.log](evidence/SPK-0048/fmt.log)、[size.log](evidence/SPK-0048/size.log)、[clippy.log](evidence/SPK-0048/clippy.log)、[rustdoc.log](evidence/SPK-0048/rustdoc.log) |

新通过的 18 项契约：

```
a01 a02 a03 a04          公式/粗体回车的结构语义
a05 a06 b10             公式边界删除保持结构
b01                     真实字形命中后回车
b02 b03 b04 b05         回显按结构投影、不画标记字符、无几何也有反馈
b06 b07 b08 b13         几何按 revision 解释；同帧点击+键入不崩溃
a15                     无几何时 Home/End 停在块边界
a17                     本地撤销
```

原生新通过 4 项：`formula-backspace`、`inline-enter`、`display-enter`、`undo`。
其中 `formula-backspace` 是报告 0046 记录的内容损坏类缺陷。

### 零回归自查

逐用例比对报告 0046 的原始证据（`docs/spikes/evidence/SPK-0046/summary.md` 的 32 行表格）
与本轮结果（[evidence/SPK-0048/summary.md](evidence/SPK-0048/summary.md) 的"体验明细"表）：

- 基线通过（`ok`）的 13 项中，**没有一项**变 FAILED；
- 基线失败的 19 项中，18 项转 `ok`，仅 `a07` 按裁决保留 FAILED。

原生窗口同理：基线通过（`passed`）的 4 项全部保持 passed，
基线失败的 4 项（`inline-enter`、`display-enter`、`formula-backspace`、`undo`）全部转 passed。

报告 0047 另以**基线提交新建 worktree 重跑**验证过同一口径（得 13/19，与 0046 逐用例完全一致），
因此"零回归"是算出来的而不是照抄的。

## `a07`：已登记的规范冲突（保留 FAILED）

`a07_paste_plain_punctuation_matches_typing` 断言"粘贴普通标点必须等于逐字键入"。
但[体验规范 §9](../EXPERIENCE.md) 要求"普通粘贴**优先使用剪贴板的 HTML/MathML 等结构格式**，
退化到纯文本"——**结构感知粘贴是规范要求的行为**。

因此本改造按规范实现结构感知粘贴，`a07` 维持 FAILED，**不做 expected-failure 豁免**，
也不为它牺牲内容完整性（转义会让 `$x$` 静默降级为字面文本，正是报告 0046 判 Fail 的同一类损坏）。
裁决与理由记在 [ADR 0031](../adr/ADR-0031-structural-cursor.md)「规范冲突登记项」。
§9 尚未实现的其余部分（结构格式读取、"粘贴为"、源码预览）属阶段 3 格式交换范围。

## 失败与不确定性

- **未覆盖**：真实 fcitx/IBus 候选窗、Wayland、Windows/macOS、读屏、不同 DPI/GPU 的系统验收；
  本轮只跑根 workspace，未重跑全部历史 spike、安全沙箱、网络协作与多后端转换验证。
- **撤销的范围**：单机产品动作层的最小实现，**不是** CRDT 历史或 checkpoint；
  不含 actor scoped undo、时间线、分支与合并（阶段 4）。
- **公式仍是源串**：`Inline::Math` 保存 Typst 数学源码，没有槽位结构，
  故分数/上下标的槽位导航仍不可用；升级需新 ADR 与数据模型迁移。
- **性能未改善承诺**：E1 是正确性改造。报告 0046 的 debug 采样只作观测，不设门槛。
- 原生化窗口验证固定窗口与夹具；无窗口边界用例才是精确的事件时序证据。

## 对设计的影响

- [ADR 0031](../adr/ADR-0031-structural-cursor.md) 已 Accepted，固定三层坐标与结构编辑语义。
- `docs/modules/app.md` 的 `page_editor` 段落已更新坐标契约与职责边界。
- [报告 0012](../spikes/SPK-0012-exit-criteria.md) 的**阶段出口判定不受影响**：
  本改造在阶段 1 范围内，不改变阶段 0 的任何条件结论。
- [区域计划](../plan/RENDER_EDIT_REWORK.md)（R/E 阶段）的执行序需据此重排：
  E1 已提前完成，其 S4 吸收了原 E2 的剩余判据；R2（字体打包）仍未做。
  该重排属计划文档工作，需在同一变更内说明理由。

## 复现步骤

```bash
cd <repo>
# 全量：门禁 + 体验契约 + 原生窗口。--out 必须指向空目录。
python3 scripts/audit-editor.py --native --out .audit-e1
# 只跑体验契约：
python3 scripts/audit-editor.py --quick --out .audit-e1-quick
cat .audit-e1/summary.md
```

注：本仓库的审计脚本拒绝写入非空目录，以免覆盖既有证据。
