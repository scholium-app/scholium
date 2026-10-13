# 第 1 阶段实现记录从 docs/spikes 迁到 docs/log

> 本文是第 1 阶段文档整理的追溯记录（过去时），不是阶段 0 验证证据，也不作为实现依据。

- 日期：2026-10-13。

## 问题

`docs/spikes/` 只收阶段 0 否决性验证：每份报告对应路线图阶段 0 条目、被其链接，结论为 Pass/Fail/Blocked。
阶段 0 关闭后，第 1 阶段的产品实现（字素导航、正文范围、页面选区、视觉行导航、默认编辑器、旧编辑器删除、
编辑面无障碍、源码视图、A4 编辑面、数学上下标、交互修复、行内样式、清晰栅格、视图跟随光标、按词撤销、
公式内回车）仍被写成 SPK-0055 至 SPK-0070。结果是它们违反 spike 规则，且“阶段 0 出口条件”的证据入口和
`docs/spikes/README.md` 的“规则”段被追加了阶段 1 链接清单，每个 PR 都改这两处而冲突。

## 改动

- 16 份报告用 `git mv` 移到 `docs/log/<日期>-<topic>.md`（日期取报告自己的日期行），证据目录移到
  `docs/log/evidence/<topic>/`（SPK-0068 没有证据目录）。标题去掉编号，补追溯声明，删去 Pass/Fail 结论和
  与阶段 0 路线图的对应，事实、测量、命令与截图保持原样；相对链接已修复。
- ADR 0035 至 0038、`docs/modules/{app,typst}.md`、`docs/plan/ROADMAP.md`、`scholium-typst` 的 Cargo 注释和
  三个窗口检查脚本的注释改为指向开发日志；路线图只保留阶段 0 的报告 0054 及之前的条目。
- `SPK-0012` “证据入口”和 `docs/spikes/README.md` “规则”段中的阶段 1 链接清单移除，判定未改。
- `docs/log/README.md` 改写边界表，用命名规则取代手写日期清单，旧无日期日志一次性列出。
- 新增 `scripts/check-doc-links.py`，检查 `docs/`、`AGENT.md`、`README.md` 的相对 Markdown 链接和图片；
  未接入 CI。

## 移动对照

迁移前文件名为 `docs/spikes/SPK-00NN-<topic>.md`，编号 0055 至 0070 与本目录日志一一对应
（如 0063 对应 `2026-10-12-a4-editor-surface.md`）；此后源码与文档不再使用这些编号。
