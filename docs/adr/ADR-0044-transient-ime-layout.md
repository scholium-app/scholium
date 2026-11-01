# ADR 0044：IME 预编辑作为独立的 Typst 派生排版输入

- 状态：Accepted
- 日期：2026-11-01
- 影响模块：app、session、typst
- 相关：[ADR 0042](ADR-0042-session-editing-core.md)

## 问题

egui 文字叠层不参与 Typst 排版，组合串覆盖 caret 后的正文。把预编辑写进 SDG 再回滚会污染
快照、源码、历史及保存，也会创造第二份权威。

## 决策

app 保留平台组合生命周期，将正文组合翻译成独立 `Preedit`；session runtime 仅为派生请求保留它，
沿既有 worker 提交不可变权威快照与显示输入。Typst Content cache 在 inline Text 构造处注入
带下划线的字面 run，仅失效旧/新目标叶及其段落。每次更新分配新的完整 layout request identity。

拆分 run 会使后串 byte 从零开始，因此组合 Scene 的几何只用于显示 caret 和审计，session 不把它
安装成文档输入 geometry。app 暂停指针、导航、选区及 AccessKit 位置几何，AccessKit 文字仍来自权威。
取消/提交后以正常 Scene 恢复文档几何。新结果未到时继续显示上次 Typst 栅格。Commit 清除临时输入，
真实文字走原有提交命令。数学内暂不注入临时 run，保持数学提交与撤销行为。

## 验证

Content Frame 测试检查插入位置和后串的位移；session 测试检查快照、源码、历史、保存不变和几何
暂停/恢复；窗口检查经同一 egui 输入路径检查中段组合、候选窗矩形、内部 caret、取消、提交和撤销。
独立 stock Typst 构造同样的下划线段落逐像素对照。真实 fcitx5/Wayland 协议交付仍需原生验收。
