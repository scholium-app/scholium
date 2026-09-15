# Spike 0001：原生 UI 候选 — Iced

- 结论：**Blocked**（真实输入法与可访问性验收未完成；其余判据见结果表）
- 对应验证项：[路线图阶段 0 第 1 项](../ROADMAP.md)
- 日期：2026-09-16
- 执行者：ation_ciger
- 关联 ADR：[0002 原生技术栈与 UI 验证顺序](../adr/0002-native-ui-validation-order.md)；最终选型 ADR 待完成

> **状态：中间报告。** 共用核心与 Iced 适配层已实现并跑通冒烟，但真实输入法、可访问性和预览定位三项
> 仍需人工验收。在这三项完成前，不得据此宣称 Iced 已通过原生 UI 验收。

## 问题与判据

原生 UI 候选能否支撑结构编辑器。通过判据是 [原生 UI 验证计划](../NATIVE_UI_VALIDATION.md) 第 4 节的验收表，
所有候选使用同一最小工程与同一组输入动作。

## 环境

| 项 | 值 |
|---|---|
| rustc / cargo | 1.98.0-nightly（bd08c9e71 / a595d0da6），edition 2024 |
| 平台 | Linux，Wayland（`wayland-1`），`XDG_SESSION_TYPE=wayland`，合成器 niri |
| GPU | Intel Raptor Lake-S UHD + NVIDIA AD107M（RTX 4060 Max-Q），Vulkan ICD 齐全 |
| CJK 字体 | 163 项；候选显式加载 `SourceHanSerifCN-Regular.otf` 并设为默认字体 |
| 输入法 | `XMODIFIERS=@im=fcitx`；**真实输入法未测**，见剩余工作 |

几个必须先记录的环境约束，它们直接影响候选的可复现性：

1. `~/.cargo/registry` 是**只读**挂载。已缓存依赖可离线构建；未缓存的候选必须把 `CARGO_HOME` 指到
   仓库内可写目录（本候选用 `spikes/native-ui/.cargo-home/`，已 gitignore）。
2. `/tmp` 在**每次命令调用之间被清空**，验证工程不能放在 `/tmp`。
3. `/etc/ssh/ssh_config.d/` 及其中文件属主是 `nobody:nobody`（应为 `root`），`git push` 报
   `Bad owner or permissions`；本次用 `GIT_SSH_COMMAND='ssh -F /dev/null'` 绕过。
4. 首次 `cargo fetch` 下载 623 MB 后因 rsproxy.cn 传输超时中断；**重试成功**（`.cargo-home` 共 685 MB）。
   最终报告必须记录实际镜像与下载完整性，否则"某个候选装不上"会被误判成框架问题。

### 渲染后端

| 后端 | 结果 | stderr |
|---|---|---|
| `ICED_BACKEND=wgpu`（默认） | 窗口可创建 | `MESA: error: ZINK: vkEnumeratePhysicalDevices failed (VK_ERROR_INITIALIZATION_FAILED)`、`egl: failed to create dri2 screen` |
| `ICED_BACKEND=tiny-skia`（软件） | 窗口可创建 | **0 条 error** |

本机混合显卡 + Wayland 下 wgpu 的 Vulkan/ZINK 路径初始化失败，**必须显式回退软件后端**。
这同时是一条负面性能证据：软件渲染不是硬件路径，16 ms 输入预算与 20 页预览延迟必须在两种后端下分别测量，
不能把软件后端的可用性当成 GPU 路径通过。

## 方法与夹具

- 共用核心 `spikes/native-ui/core`：独立 workspace，精确锁定 `thiserror =2.0.20`、
  `unicode-segmentation =1.13.3`；20 个模型层验收测试。
- Iced 候选 `spikes/native-ui/candidate-iced`：独立 workspace，`iced =0.14.0` 且启用 `advanced`
  feature（用于取得 `input_method` 事件类型），自带 `Cargo.lock`。
- 候选实现：原生窗口 + 正文/源码/预览三区；正文与预览渲染核心投影，**UI 不持有第二份可写正文**；
  输入法 `Preedit` 只更新 UI 状态，`Commit` 才产生一个核心动作；源码面板默认只读，写入被权威缓冲回滚。
- 夹具由一个非本地 actor（`ActorId(99)`）注入，使本地 undo scope 从空开始，可直接观察
  "远端动作不进本地历史"。
- 冒烟脚本：`spikes/native-ui/candidate-iced/scripts/smoke.sh`（启动 → 合成器确认窗口 → 按窗口 ID 截图 →
  干净退出；只截取本应用窗口，不采集屏幕其他内容）。

## 结果

模型层：`cargo test --offline` **20 passed / 0 failed**。
候选冒烟：**窗口创建成功、合成器已列出、0 条 error、SIGTERM 干净退出**。
证据截图：[artifacts/iced-window.png](../../spikes/native-ui/candidate-iced/artifacts/iced-window.png)
（只含本应用窗口）。

| 验收项（计划第 4 节） | 结果 | 证据 / 缺口 |
|---|---|---|
| 中文与 Unicode | **部分 Pass** | 界面中文、标签与日志全部正确渲染，无 `.notdef` 方块（截图）；模型层验证预编辑不入历史、整串提交只产生一个动作、ZWJ emoji 与组合字符不被拆坏。**缺口：真实输入法的预编辑与候选窗未测**（截图时计数为"预编辑事件 0 / IME 提交 0"）。 |
| 数学结构 | **部分 Pass** | 模型层验证分子/分母与上下标纵向导航、矩阵单元格、wrap/unwrap 保留内容、变体循环、多槽结构拒绝无策略展开。UI 已接按钮与方向键，但未做人工交互验收。 |
| 源码视图 | **部分 Pass** | 面板显示 `[只读]`，非活动语言写入被拒绝并把控件回滚到权威缓冲（截图 + 代码路径）。缺口：语法高亮、10 万行滚动与真实编辑手感未测。 |
| 团队规则 | **部分 Pass** | 远端 actor 注入的夹具留在历史但**不进本地 undo scope**：界面显示"动作 10 / 本地历史为空"；模型层另有"远端插入不被本地撤销删除"测试。缺口：真实多端同步与语言切换属阶段 0 第 6 项。 |
| 核心集成 | **Pass（本轮范围）** | 焦点直接显示核心 `Cursor`，revision 与动作数来自 core；每次编辑产生 Action 并推进 revision；过期 base revision 返回 `StaleRevision`。缺口：框架快捷键绕过项目历史的验证见剩余工作。 |
| 预览 | **Blocked** | 仅投影占位，界面明确标注"占位，不冒充最终排版"。真实 Typst 编译与点击定位属阶段 0 第 2 项。 |
| 大源码 | **部分 Pass（模型层）** | 10 万行 / 4.99 MB：构建 **23 ms**，单次局部插入 **180 µs**（debug 构建）。缺口：UI 滚动与软件后端的重排延迟未测。 |
| 可访问性 | **Blocked** | 未做系统辅助功能检查，逻辑测试不能替代。 |
| 文件恢复 | **Blocked** | 核心不含持久化与 IO；属阶段 0 第 5 项。 |
| 构建与依赖 | **Pass** | `iced 0.14.0` 全树编译通过；构建与运行只需 cargo，无 Node/npm/WebView。直接依赖许可证：`thiserror`、`unicode-segmentation`、`proc-macro2`、`quote`、`syn` 为 `MIT OR Apache-2.0`，`unicode-ident` 为 `(MIT OR Apache-2.0) AND Unicode-3.0`。 |

许可证检查的一个实际产出：`unicode-ident` 使用 SPDX `Unicode-3.0`，而 `AGENT.md` 的允许清单原先只列了旧的
`Unicode-DFS`，已补上 `Unicode-3.0`。

## 失败与不确定性

- **真实输入法、可访问性、预览定位未验收**，这是本报告判 Blocked 的原因。
- wgpu 路径在本机初始化失败（见上），当前结论只在软件后端下成立。
- 模型层的撤销用字符身份近似 CRDT 相对位置，`TextDeleted` 的恢复锚点只记录右邻单一身份；
  这在验收场景下正确，但**不是** CRDT 收敛证据（属第 4 项）。
- 结构编辑的反向配方标记为不可逆，补偿历史的完整语义属阶段 4。
- 性能数字来自 debug 构建、单次采样，且核心性能与 UI 性能不是一回事。
- 窗口被 niri 平铺为 1270×1494，与请求的 1280×820 不同；布局验收需在固定几何下重做。

## 对设计的影响

- 核心与 UI 分离、候选各自独立 workspace 的结构可行，未出现反向依赖。
- 固定槽位 + 空占位节点足以支撑数学结构导航与字素安全编辑，UI 无需保存第二份权威状态。
- 输入法必须在应用层区分 `Preedit` 与 `Commit`，否则预编辑会污染历史；这一条已由实现与模型测试共同固定。
- 阶段 0 结论必须分别记录每个候选使用的渲染后端；"能开窗口"不等于 GPU 路径可用。

## 剩余工作

1. **真实输入法**：运行冒烟脚本后手动用 fcitx5 输入中文，观察界面计数——预编辑期间"核心动作"不得增加，
   提交后只增加一次。
2. **可访问性**：用真实窗口与系统辅助功能检查角色/名称/选区暴露。
3. **框架快捷键绕过**：确认 Ctrl+Z 不会被 Iced 默认绑定截走而不经过 `Editor::undo`。
4. **性能**：在 `tiny-skia` 与 `wgpu`（若修复驱动）两种后端下分别测量输入延迟与 20 页预览延迟。
5. **固定窗口几何**重做布局与滚动验收。

## 复现步骤

```bash
# 1. 模型层验收
cd spikes/native-ui/core
cargo test --offline
cargo test --offline large_source -- --nocapture

# 2. 候选依赖获取与构建（需要网络，写入仓库内可写 CARGO_HOME）
cd ../../..
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"
cargo fetch  --manifest-path spikes/native-ui/candidate-iced/Cargo.toml
cargo build  --manifest-path spikes/native-ui/candidate-iced/Cargo.toml

# 3. 冒烟 + 窗口截图（只截本应用窗口）
bash spikes/native-ui/candidate-iced/scripts/smoke.sh tiny-skia
```
