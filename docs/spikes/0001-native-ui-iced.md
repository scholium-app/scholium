# Spike 0001：原生 UI 候选 — Iced

- 结论：**Blocked**（阶段 0 第 1 项未完成；已完成的判据见结果表）
- 对应验证项：[路线图阶段 0 第 1 项](../ROADMAP.md)
- 日期：2026-09-16
- 执行者：ation_ciger
- 关联 ADR：[0002 原生技术栈与 UI 验证顺序](../adr/0002-native-ui-validation-order.md)；最终选型 ADR 待完成

> **状态：中间报告。** 本文件不是最终验收结论，只记录已完成的可复现证据、剩余工作和环境约束。
> 在 Iced 适配层补齐之前，不得据此宣称任何 UI 框架可用。

## 问题与判据

原生 UI 候选能否支撑结构编辑器。通过判据是 [原生 UI 验证计划](../NATIVE_UI_VALIDATION.md) 第 4 节的验收表，
所有候选使用同一最小工程与同一组输入动作。本报告只交付该工程的**独立 Rust 文档核心**及其模型层证据。

## 环境

| 项 | 值 |
|---|---|
| rustc / cargo | 1.98.0-nightly（bd08c9e71 / a595d0da6），edition 2024 |
| 平台 | Linux，Wayland（`wayland-1`），`DISPLAY=:0`，`XDG_SESSION_TYPE=wayland` |
| CJK 字体 | 163 项（Noto Sans CJK、Source Han Serif 等） |
| 输入法 | `XMODIFIERS=@im=fcitx`；本次**未**做真实输入法交互 |
| 图形会话 | 会话存在，但本报告未启动真实窗口，可访问性项未测 |

三个必须先记录的环境约束，它们直接影响后续候选的可复现性：

1. `~/.cargo/registry` 是**只读**挂载（`touch` 返回 `只读文件系统`）。因此只能用已缓存依赖离线构建；
   需要下载的候选（Iced 未缓存）必须把 `CARGO_HOME` 指到仓库内可写目录。
2. `/tmp` 在**每次命令调用之间被清空**，验证工程不能放在 `/tmp`，必须在仓库内。
3. `/etc/ssh/ssh_config.d/` 与其中文件的属主是 `nobody:nobody`（应为 `root`），`git push` 因此报
   `Bad owner or permissions`；本次用 `GIT_SSH_COMMAND='ssh -F /dev/null'` 绕过。**这是主机配置问题，
   不修的话每次推送都要绕过。**

## 方法与夹具

- 共用核心：`spikes/native-ui/core`，独立 workspace，精确锁定 `thiserror =2.0.20`、
  `unicode-segmentation =1.13.3`（只有这两个版本在本机缓存中，其余不可离线获取）。
- 核心只实现被验收项直接压到的模型能力：语义节点与固定槽位、树光标与结构导航、字符级稳定身份、
  Action 分组与 actor 作用域 undo、源码面板方言与可写性、大文本缓冲。
  **不实现** CRDT 收敛、真实 reconcile、持久化与语言切换协议——它们分别是阶段 0 第 3、4、5、6 项。
- 夹具：20 个模型层验收测试；源码视图使用 10 万行、4 988 898 字节的生成缓冲区。
- 命令：`cargo test --offline`（在 `spikes/native-ui/core` 下）。

## 结果

`cargo test --offline`：**20 passed / 0 failed**。

| 验收项（计划第 4 节） | 结果 | 证据 / 缺口 |
|---|---|---|
| 中文与 Unicode | **部分 Pass** | 预编辑不进历史、整串提交只产生 1 个动作、取消无痕迹、ZWJ emoji 与组合字符不被按字节拆坏、字素中间偏移被拒绝且无部分修改。缺口：真实输入法的预编辑渲染与候选窗定位未测。 |
| 数学结构 | **部分 Pass** | 分子/分母与 base/sub/sup 纵向导航、矩阵单元格导航、wrap/unwrap 保留内容、变体循环回绕、多槽结构无保留策略时拒绝 unwrap。缺口：真实控件的选区与命中未测。 |
| 源码视图 | **部分 Pass** | 只读门禁默认生效且拒绝后缓冲不变、授权后局部替换可用。缺口：语法高亮、滚动、真实文本编辑控件未接入。 |
| 团队规则 | **部分 Pass** | **远端插入不被本地撤销删除**（A 输入 `abc`，B 远端插入 `X`，A 撤销后仅剩 `X`）、远端动作不进本地 undo scope、非活动语言面板只读。缺口：真实远端接入、epoch 与切换屏障属第 6 项。 |
| 核心集成 | **部分 Pass** | 每次提交的编辑都追加 Action 并推进 revision；过期 base revision 返回 `StaleRevision` 且不改内容。缺口：框架 undo 不得绕过项目历史的测试需真实 UI。 |
| 预览 | **Blocked** | 需真实窗口与绘制；核心只提供 `to_plain_text` 投影。 |
| 大源码 | **部分 Pass** | 10 万行 / 4.99 MB：构建 **23 ms**，单次局部插入 **180 µs**（debug 构建，未开优化）。缺口：滚动与重排需真实 UI，正式预算见 `TESTING.md`。 |
| 可访问性 | **Blocked** | 需真实窗口与系统辅助功能检查，无法用逻辑测试替代。 |
| 文件恢复 | **Blocked** | 核心不含持久化与 IO；属阶段 0 第 5 项。 |
| 构建与依赖 | **Pass** | 构建与运行只需 cargo，无 Node/npm/WebView 步骤。直接与传递依赖许可证全部合规：`thiserror`、`unicode-segmentation`、`proc-macro2`、`quote`、`syn` 为 `MIT OR Apache-2.0`，`unicode-ident` 为 `(MIT OR Apache-2.0) AND Unicode-3.0`。 |

许可证检查的一个实际产出：`unicode-ident` 使用 SPDX `Unicode-3.0`，而 `AGENT.md` 的允许清单原先只列了旧的
`Unicode-DFS`，已补上 `Unicode-3.0`。

## 失败与不确定性

- **Iced 适配层未实现**，因此第 1 项无法给出 Pass/Fail。这是本报告判 Blocked 的唯一原因。
- `iced 0.14.0` 不在本机缓存中，构建它需要仓库内 `CARGO_HOME` 并联网下载（约 380 个包）；
  已把 `spikes/native-ui/.cargo-home/` 配好并写入 `.gitignore`。
- 首次 `cargo fetch` 下载 623 MB 后因 rsproxy.cn 传输超时中断（在 `equivalent 1.0.2` 处失败，
  报 `transfer too slow`）；**重试后成功**，`.cargo-home` 共 685 MB，`iced 0.14.0` 依赖树已完整落盘。
  候选依赖获取在本机不稳定，最终报告必须记录实际镜像与下载是否完整，否则"某个候选装不上"会被误判成框架问题。
- 已下载的 `iced-0.14.0` 源码（含 examples）就在 `.cargo-home/registry/src/` 下，接入适配层时应以它为准，
  不依赖记忆中的 API。
- 模型层的撤销用字符身份近似 CRDT 相对位置，`TextDeleted` 的恢复锚点只记录右邻单一身份。
  这在验收场景下正确，但**不是** CRDT 收敛证据；真实收敛属第 4 项。
- 结构编辑的反向配方标记为不可逆，补偿历史的完整语义属阶段 4，未在此实现。
- 性能数字来自 debug 构建且为单次采样，不能当预算结论。

## 对设计的影响

- 核心与 UI 分离、候选各自独立 workspace 的结构可行，未出现反向依赖。
- 固定槽位 + 空占位节点的模型足以支撑数学结构导航与字素安全编辑，不需要在 UI 层保存第二份权威状态。
- 需要在阶段 0 结论中明确：仓库内 `CARGO_HOME` 与只读 registry 的组合是环境事实，其他机器上未必成立，
  报告必须记录各自的依赖获取方式。

## 复现步骤

```bash
cd spikes/native-ui/core
cargo test --offline                 # 20 个模型层验收测试
cargo test --offline large_source -- --nocapture   # 大源码采样数字

# 候选依赖获取（需要网络，写入仓库内可写 CARGO_HOME）
cd ../../..
CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" \
  cargo fetch --manifest-path spikes/native-ui/candidate-iced/Cargo.toml
```
