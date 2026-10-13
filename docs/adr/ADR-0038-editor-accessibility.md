# ADR 0038：Typst 编辑面的正文无障碍投影

## 状态

接受。把 [ADR 0021](ADR-0021-native-accessibility-patches.md) 中 `accesskit_atspi_common` 的禁用状态补丁
从阶段 0 实验扩展到主程序；ADR 0021 的 egui TextRun 坐标补丁不采用（本方案不用 egui TextEdit）。

## 背景

阶段 0 的正文 Text/选区与数学读屏（报告 0020、0021、0042）只在 `spikes/native-ui` 候选里实现；
主程序旧编辑面只发布了一个带名称的 TextEdit 角色，删除后 Typst 编辑面在 AT-SPI 中只是一个无名
`unknown` 节点，读屏无法读取正文。主程序还把禁用的 Ribbon 按钮报为 Enabled/Sensitive。

## 决定

1. 编辑面节点是 AccessKit `MultilineTextInput`（名称“正文”），支持 `SetTextSelection`；子节点为
   TextRun：每个正文 Text 叶按当前 Scene 精确 caret 分视觉行（纵向重叠为同一行，与上下行导航同规则），
   段落末尾追加 `\n`。位置在 AccessKit（TextRun、Unicode 标量序号）与编辑器（叶、UTF-8 字节）之间换算；
   字符几何为屏幕逻辑像素，同簇字符共享 cluster 停靠点。
2. 排版 pending 时继续发布权威文本、不带几何，不能临时移除 TextRun（会注销 AT-SPI Text 接口）。
3. 辅助技术选区只按同一快照发布的 runs 解码（同一快照的每次发布文本相同，几何以 Frame pt 缓存，
   滚动不重建）；没有正文 run 的位置（公式旁空叶、数学槽、纯公式段落）读作本段最近正文或段落占位 run；正文已变、端点落在字素内部、输入排队或输入法组合期间
   拒绝并提示，不猜位置。
4. 行内公式是正文之外的 `Math` 兄弟节点（`公式：分数（分子：…；分母：…）`），不写入 TextRun 文本；
   光标在数学槽时，一个稳定的 polite live `Status` 节点给出由外到内的槽位路径与内容（空槽读“空”）。
   措辞沿用报告 0042 经 Orca 验证的约定。
5. 根 workspace 以 `[patch.crates-io]` 使用 `vendor/accesskit_atspi_common`（0.18.1 原包，`src/node.rs`
   一处条件修改）：禁用节点不再发布 Enabled/Sensitive；只读但未禁用的节点改为同时发布 ReadOnly 与
   Enabled/Sensitive（上游此时两者都不发布）。升级 accesskit 时先查上游是否已修复再移除。

## 验证与边界

isolated D-Bus 会话里的 AT-SPI 实机验收与单元回归见[开发日志《Typst 编辑面的正文无障碍》](../log/2026-10-11-editor-accessibility.md)。
本批没有 Orca 语音实测、跨平台（Windows UIA / macOS）验收、RawMath 内部导航、数学槽位的 Text 偏移，
也不改变 AccessKit 对 TextRun 之外节点的文本模型。
