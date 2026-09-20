# Spike 0044：P0 范围核对与必要补测

- 日期：2026-09-20；基线：7404f60。
- 依据：用户本轮缩减打磨要求；[ADR 0023](../adr/ADR-0023-stage0-feasibility-boundary.md)。
- 当前阶段状态只见 [0012](SPK-0012-exit-criteria.md)。

## 核对结果

| 原始要求 | 现有证据与本轮处理 |
|---|---|
| Linux 原生编辑/IME/结构选区/双源码 | 0021–0030、0040、0042；已有真实窗口，不再重复优化 |
| 后台 Typst、不采纳旧结果、声明规模 | 0029/0030；用户认可延迟；不新增 GPU/逐帧极限指标 |
| 文件打开/保存/重开/草稿恢复 | 0031；原计划没有要求 UI 选型先实现永久历史身份 |
| 大源码、系统辅助功能 | 0040 十万行 release，0042 数学槽位 Orca；人工听感/跨平台列后续 |
| 团队模拟、远端输入与本地撤销 | 0038/0039 已有语言屏障和源码双副本；本轮只补结构窗口直接路径 |
| CRDT 两端收敛/undo/快照 | 0007/0014；完整共享 SDG adapter 属阶段 1 集成，不伪装已经实现 |
| 受支持 reconcile/Raw | 0006、0016、ADR 0008；正式 parser 不再追加进 P0 |
| 恢复机制、长期格式 | 0008/0031/0037；永久 schema/数据库比较移到阶段 1 存储开工前 |
| 编译入口隔离与根外文件 | 0033/0036/0040/0043；显式澄清固定运行时例外，本轮安全 7/7 |
| 六类混排/引用/包重建 | 0034/0035、0040；沿用声明支持矩阵与正负对照，不追加任意格式支持 |

此前的“未完成”包含真实产品缺口，也包含额外出口门槛；本轮不将它们冒充已实现。
安全白名单与阶段安排是显式范围裁决，非新实验推翻旧失败。/work 配额失败原型 0041 继续保留。

## 必要修改与验证

原生结构窗口新增显式环境开关 `SCHOLIUM_SPIKE_REMOTE_ACTION=1`，仅作为受信夹具入口：
对当前文本叶子调用核心 `apply_remote` 追加 REMOTE，不建第二份可写模型，不伪造网络。
团队模式不提供该按钮，组合输入期间拒绝注入。标准模拟正文已有远端 actor 夹具。

先写回归，发现本地 Wrap 原先没有 inverse，会挡住后面的文字撤销。
为根式/定界符单槽包裹记录新建 wrapper 身份，撤销调用语义 Unwrap，保留当前子节点和其中远端文字，
追加补偿动作，不恢复历史快照。多槽分数解包、结构 redo 与并发树移动仍不承诺。

- UI 默认 63 passed / 11 ignored；共享核心 47 passed；Clippy all-targets / -D warnings 通过。
- 真实窗口 3/3：原数学编辑、整节点选区、新增结构远端撤销。
  新路径：分子输入 Q → 包裹根式 → 模拟 REMOTE → 撤销包裹 → 撤销 Q → REMOTE 保留 → 再次输入成功。
- 安全复测 7/7；已有完整阶段首轮和修复补测见 0040，未把它改写为同一次完整全过。
- 本轮没有新增依赖、正式 schema、服务端或网络。
- 另查 core 自身的严格 Clippy，发现 18 条既有文档/风格诊断；以 HEAD 的独立临时副本复核同样失败。
  不把 UI Clippy 通过等同全仓通过，也不为 spike 收尾扩展成全库文档清理；阶段 1 核心整理时处理。
  源码 reconcile 的现有 4 项 Cargo 回归通过。

证据：[目录](evidence/SPK-0044/)。复现：

```sh
CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" cargo test --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" cargo test --offline --manifest-path spikes/native-ui/core/Cargo.toml
SCHOLIUM_SPIKE_BINARY=spikes/native-ui/candidate-egui/target/release/scholium-spike-egui \
SCHOLIUM_TYPST_BIN=/tmp/scholium-typst-toolchain/bin/typst \
/usr/bin/python spikes/native-ui/candidate-egui/scripts/native-edit-acceptance.py /tmp/scholium-scope structure_remote_undo math_edit slot_selection
SCHOLIUM_TYPST_BIN=/tmp/scholium-typst-toolchain/bin/typst bash spikes/verify-stage0.sh security
```
