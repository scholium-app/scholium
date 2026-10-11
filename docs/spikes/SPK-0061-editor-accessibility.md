# SPK-0061：Typst 编辑面的正文无障碍

- 日期：2026-10-11；结论：**Pass（Linux AT-SPI，isolated 会话）**。
- 对应：[ADR 0038](../adr/ADR-0038-editor-accessibility.md)、阶段 0 条件 1（[报告 0012](SPK-0012-exit-criteria.md)）的
  正文 Text/选区与数学读屏能力在主程序重新实现；前序实验见报告 0020、0021、0042。
- 基线：main `ca846ba`；分支 `ation_ciger/feat/editor-accessibility`。

## 问题

阶段 0 的正文无障碍只在 `spikes/native-ui` 候选里实现，主程序从未接入。删除旧编辑器后，编辑面在
AT-SPI 里只是一个无名 `unknown` 节点，没有 Text 接口；禁用的“复制”按钮被报为 ENABLED + SENSITIVE。
见[改动前的对象树](evidence/SPK-0061/before-tree.txt)。

## 实现

- `typst_editor::access`：编辑面节点为 `MultilineTextInput`（“正文”），支持 `SetTextSelection`；
  TextRun 按当前 Scene 精确 caret 分视觉行，段落末尾 `\n`，字符几何为屏幕逻辑像素。
  同一行的 CJK 回退字体与拉丁字形顶边不同，按纵向重叠归行（首轮按顶边相等分组，把“中文abc”拆成
  两行并让辅助选区解码到错误的 run，单元回归先失败后修复）。
- pending 时发布权威文本而不带几何；辅助选区只按上一帧发布的 runs 解码，正文已变、字素内部、
  输入排队或组合期间拒绝并提示。
- `access::speech`：公式为 `Math` 兄弟节点（`公式：分数（分子：12；分母：空）`），槽位上下文为
  polite live `Status`（`分数分母：空`），措辞沿用报告 0042。
- 根 workspace patch `vendor/accesskit_atspi_common`（0.18.1 + 3 行）：禁用节点不报 Enabled/Sensitive。
- 代码评审后补充（先写失败回归）：离开段尾公式后光标在不发布的空叶上，读屏丢失 caret——
  现在读作本段最近正文；纯公式段落（含末段）有占位 run 并可接收折叠选区；软换行行末的辅助 caret
  解码为 Upstream，与发布一致；runs 拆到 `access/runs.rs`，Scene caret 按叶一次分组，几何以 Frame pt
  缓存，滚动不重建；首行总从字节 0 开始；harness 记录助手日志、助手退出即失败、CI 步骤限时。
- 实机验收：`spikes/typst-main-editor/a11y/`。私有 a11y 总线 + `at-spi2-registryd` + 一个只提供
  `org.a11y.Status` 的测试服务（at-spi2-core 2.62 已无 AccessKit 等待的 `ScreenReaderEnabled`），
  不连接桌面会话；CI 窗口作业同样运行。

## 复现

```sh
cargo +1.96.0 test --locked --workspace
cargo +1.96.0 build --locked -p scholium-app -p scholium-storage --bins
xvfb-run -a -s '-screen 0 1280x900x24' dbus-run-session -- \
  bash spikes/typst-main-editor/a11y/run.sh target/debug/scholium-app /tmp/scholium-a11y
```

## 结果

| 检查 | 结果与证据 |
|---|---|
| 回归 | **155 passed / 0 failed**，新增 13 项无障碍用例（读序、分行与几何、pending 文本、caret/选区编码、辅助选区替换、过期发布拒绝、字素内部拒绝、混排同行、数学对象与槽位、嵌套播报、公式后空叶 caret、纯公式段落、软换行行末）；[tests.log](evidence/SPK-0061/tests.log) |
| gates | fmt、clippy、rustdoc、文件规模、licenses/bans/sources 通过 |
| AT-SPI 实机 | **9/9**：`entry` + Text 接口且 FOCUSED/EDITABLE/MULTI_LINE；输入文本与 caret 12；段落 `\n`；AT-SPI 设选区 0–5 后键入一次替换；连续输入 91 次读取 0 失败（期间 34 帧 pending）；字符范围在窗口内；公式与槽位名称；禁用“复制”不再 ENABLED/SENSITIVE、“粘贴”仍可用；[summary.json](evidence/SPK-0061/a11y/summary.json)、[日志](evidence/SPK-0061/a11y-check.log) |
| 既有窗口流程 | 33 步与迁移检查仍通过；[native-check.log](evidence/SPK-0061/native-check.log) |
| advisories | **Fail**，仍是既有 7 项；[advisories.log](evidence/SPK-0061/advisories.log) |

## 未覆盖

没有 Orca 语音实测（只验证 AT-SPI 对象、文本、状态与名称），没有 Windows UIA / macOS 验收。
数学槽位没有 Text 偏移（光标在槽内时正文 caret 读作公式位置），RawMath 只给原串名称。
阶段判定不升级。
