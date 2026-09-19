# Spike 0032：团队语言门禁进入原生窗口

- 日期：2026-09-19；基线 `adc6bd9` 加本轮修改。
- 结论：**Pass（单窗口三成员模拟）**；阶段出口不升级，见 [0012](SPK-0012-exit-criteria.md)。
- Linux / Wayland / niri，真实 AT-SPI 与键盘；[ADR 0016](../adr/ADR-0016-team-window-gate-spike.md)。

## 实现与真实窗口证据

候选依赖既有语言协调 spike，导出窄适配接口，原命令行验收也运行同一个库。
三成员分别持有源码草稿，共用权威正文；提交在协调器与 reconcile 均成功后发布。
许可刷新不改变旧草稿 epoch，切换要求逐成员确认，离线和未授权内容留在草稿。

[桌面结果](evidence/SPK-0032/desktop/results.json)通过，已人工核对
[截图](evidence/SPK-0032/desktop/team-language.png)：

1. Alice、Bob、Carol 同为 LaTeX，分别输入 AAA、BBB、CCC，依次提交；正文三份都存在。
2. Alice 留下 DRAFT，模拟离线提交返回 NetworkUnreachable，正文不变。
3. 请求 Typst 后新提交被冻结；只有 Alice 确认时完成返回 DrainIncomplete。
4. Bob/Carol 确认后切换至 epoch 2；回到 Alice，DRAFT 逐字保留。
5. 刷新许可仍不能提交 epoch 1 草稿，返回 SourceEpochStale。
6. 显式重新生成后 LaTeX 提交返回 DialectNotActive；改用 Typst 后输入 TYPST 成功，接受总数为 4。

## 自动检查

- 原协调器 **60/60**，包括故意写坏的对照：[日志](evidence/SPK-0032/coordinator.log)。
- 候选 **45 项默认测试通过，11 ignored**：[日志](evidence/SPK-0032/ui-tests.log)。
  新增 3 项验证失败时正文/接受计数不变、离线/非活动语言、确认缺失与旧草稿拒绝。
- 两个 crate Clippy 全 targets、格式、候选 licenses/bans/sources 通过；未重跑全阶段套件及独立 ignored 场景。

## 限制

这是单窗口三成员**顺序操作模拟**，不是多窗口并发或远程协作。没有网络、账号认证、
真实 CRDT 合并、服务持久化、协作撤销和跨语言 IME 屏障验证。原写集抽检不能阻挡全部恶意源码。
正文在团队模式只读，以免未接门禁的结构命令绕过本轮测试；普通候选模式不变。
团队模式不保存实验会话文件，草稿仅在当前进程保留；不能宣称离线草稿已持久化。

## 运行

仓库根目录：

```sh
SCHOLIUM_SPIKE_TEAM=1 spikes/native-ui/candidate-egui/target/debug/scholium-spike-egui
```

真实窗口复现：

```sh
SCHOLIUM_TYPST_BIN=/tmp/scholium-typst-toolchain/bin/typst \
  /usr/bin/python spikes/native-ui/candidate-egui/scripts/native-edit-acceptance.py \
  /tmp/scholium-team-window team_language_gate
```
