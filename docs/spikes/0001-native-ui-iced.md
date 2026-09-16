# Spike 0001：原生 UI 候选 — Iced

- 结论：**Fail**（可访问性项不通过，且是框架级缺口；其余判据多为 Pass，预览项未测）
- 对应验证项：[路线图阶段 0 第 1 项](../ROADMAP.md)
- 日期：2026-09-16
- 执行者：ation_ciger
- 关联 ADR：[0002 原生技术栈与 UI 验证顺序](../adr/0002-native-ui-validation-order.md)；最终选型 ADR 待完成

> **状态：候选结论 Fail。** 共用核心与 Iced 适配层已实现，冒烟、渲染后端与真实输入法（经 fcitx5 注入）
> 都有可复现证据；**可访问性项实测不通过**，原因是 Iced 当前不发布任何可访问对象树。
> 按 [原生 UI 验证计划](../NATIVE_UI_VALIDATION.md) 第 1 节，候选产出结论后才进入下一候选，
> 因此可以推进 GPUI 比较；这不等于最终选型。

## 问题与判据

原生 UI 候选能否支撑结构编辑器。通过判据是 [原生 UI 验证计划](../NATIVE_UI_VALIDATION.md) 第 4 节的验收表，
所有候选使用同一最小工程与同一组输入动作。

## 环境

| 项 | 值 |
|---|---|
| rustc / cargo | 1.98.0-nightly（bd08c9e71 / a595d0da6），edition 2024 |
| 平台 | Linux，Wayland（`wayland-1`），`XDG_SESSION_TYPE=wayland`，合成器 niri |
| GPU | Intel Raptor Lake-S UHD + NVIDIA RTX 4060 Laptop（驱动 615.71.09）；`/dev/dri` 有 card0/card1/renderD128/renderD129 |
| CJK 字体 | 163 项；候选显式加载 `SourceHanSerifCN-Regular.otf` 并设为默认字体 |
| 输入法 | fcitx5 运行中（`fcitx5 -r`）；输入注入用 `ydotool`（`extra/ydotool`，走 `/dev/uinput`，会经过输入法） |

几个必须先记录的环境约束，它们直接影响候选的可复现性：

1. `~/.cargo/registry` 是**只读**挂载。已缓存依赖可离线构建；未缓存的候选必须把 `CARGO_HOME` 指到
   仓库内可写目录（本候选用 `spikes/native-ui/.cargo-home/`，已 gitignore）。
2. `/tmp` 在**每次命令调用之间被清空**，验证工程不能放在 `/tmp`。
3. `/etc/ssh/ssh_config.d/` 及其中文件属主是 `nobody:nobody`（应为 `root`），`git push` 报
   `Bad owner or permissions`；本次用 `GIT_SSH_COMMAND='ssh -F /dev/null'` 绕过。
4. 首次 `cargo fetch` 下载 623 MB 后因 rsproxy.cn 传输超时中断；**重试成功**（`.cargo-home` 共 685 MB）。
   最终报告必须记录实际镜像与下载完整性，否则"某个候选装不上"会被误判成框架问题。

### 渲染后端，与一次已更正的错误结论

先记录一次错误结论及其根因。本报告初稿曾判定"本机 wgpu 路径初始化失败，必须回退软件后端"。
该结论**是错的**，原因是当时 agent 沙箱用 tmpfs 覆盖了 `/dev`，其中没有任何 GPU 设备节点：

```text
/dev 挂载栈（错误结论成立时的状态）
  ... /dev ro,nosuid,nodev - devtmpfs devtmpfs
  ... /dev rw,nosuid,nodev - tmpfs tmpfs          <- 这一层盖住了真实设备节点
```

缺少 `/dev/dri` 与 `/dev/nvidia*` 时，Mesa 只能尝试 zink/dri2 并失败，stderr 因此出现
`ZINK: vkEnumeratePhysicalDevices failed` 与 `egl: failed to create dri2 screen`；候选随后回退软件后端并正常渲染。
**那些报错是环境造成的，既不是驱动问题，也不是 wgpu 或候选的缺陷。**

设备节点可见后用 `cargo run --bin gpu_probe` 实测：

```text
wgpu 编译期启用的后端: Backends(VULKAN | GL)
枚举到 3 个适配器
[0] backend=Vulkan type=IntegratedGpu  Intel(R) Graphics (RPL-S)          Mesa 26.2.2   request_device: OK
[1] backend=Vulkan type=DiscreteGpu    NVIDIA RTX 4060 Laptop GPU        615.71.09     request_device: OK
[2] backend=Gl     type=Other          NVIDIA RTX 4060 Laptop GPU        (3.3.0)       request_device: FAIL (Parent device is lost)
```

结论：**Vulkan 路径可用**。GL 适配器那一条以 `Parent device is lost` 失败，它是 MESA 噪声的唯一来源，
不影响 Vulkan 选择，也不需要任何显式后端覆盖。

`scripts/compare-backends.sh` 的冷启动与稳态开销（各 2 次）：

| 后端 | 首窗口 | 稳态 CPU（约 3 s） | 常驻内存 | stderr error |
|---|---|---|---|---|
| 默认（wgpu → Vulkan） | 855 / 954 ms | 0.84 / 1.00 s | 295 / 347 MB | 0 |
| `ICED_BACKEND=wgpu` | 722 / 843 ms | 0.79 / 1.15 s | 294 MB | 0 |
| `ICED_BACKEND=tiny-skia` | 482 / 490 ms | 0.55 s | 103 MB | 0 |

软件后端在**静态窗口**下反而更快、更省内存，这符合预期（省掉了 GPU 初始化与显存占用）。
但静态窗口不代表编辑负载：真实输入延迟与 20 页预览延迟必须在两种后端下分别测量，
既不能把软件后端的可用性当作 GPU 路径通过，也不能用本次数字宣称任一后端达标。

## 方法与夹具

- 共用核心 `spikes/native-ui/core`：独立 workspace，精确锁定 `thiserror =2.0.20`、
  `unicode-segmentation =1.13.3`；20 个模型层验收测试。
- Iced 候选 `spikes/native-ui/candidate-iced`：独立 workspace，`iced =0.14.0` 且启用 `advanced`
  （`input_method` 事件类型）与 `canvas`（自绘结构）两个**非默认** feature，自带 `Cargo.lock`。
- 共用核心提供布局：`core/src/layout.rs` 把语义图布局成 `Text` / `Rule` 图元（含分数线、上下标位置、
  矩阵网格、根号上横线），所有候选绘制**同一份布局**，这是候选之间可比的前提。
- 候选实现：原生窗口 + 正文/源码/预览三区；正文用 `canvas` 绘制核心布局，**UI 不持有第二份可写正文**；
  输入法 `Preedit` 只更新 UI 状态，`Commit` 才产生一个核心动作；源码面板默认只读，写入被权威缓冲回滚。
- 夹具由一个非本地 actor（`ActorId(99)`）注入，使本地 undo scope 从空开始，可直接观察
  "远端动作不进本地历史"。
- 冒烟脚本：`spikes/native-ui/candidate-iced/scripts/smoke.sh`（启动 → 合成器确认窗口 → 按窗口 ID 截图 →
  干净退出；只截取本应用窗口，不采集屏幕其他内容）。
- 输入注入脚本：`scripts/input-test.sh`。用 `ydotool` 经 `/dev/uinput` 注入按键，因此**会经过输入法**；
  每次注入前都复核焦点在本应用窗口，焦点不符立即中止，避免敲进其他窗口。

### 结构渲染：一个被漏掉的验收前提

本报告此前的版本把"正文"渲染成 `to_plain_text()` 投影——分数显示为 `a/b`，上下标显示为 `x_1^2`。
这不只是外观问题，而是**验收方法本身失效**：

- "数学结构"这一项看起来过了，实际只验到模型层；候选是否**能画出结构**完全没测。
- 候选之间无法比较：比较的其实是各框架渲染一段字符串的能力，而不是渲染结构的能力。
- 在这种桩上继续做可访问性等后续验收，证据价值也很低。

修复方式是把布局下沉到**共用核心**，让所有候选绘制同一份结构：

- 新增 `core/src/layout.rs`：递归布局语义图，产出 `Item::Text { x, y, size, content }` 与
  `Item::Rule { x, y, width, height }`；分数画分数线、上下标错位并缩小、矩阵按列排网格、
  根式画根号与上横线；行内按基线对齐。
- 新增 `core/tests/layout.rs`（6 个测试），断言的是结构本身而不是文本：
  分子底边在分数线之上、分母顶边在其下、上标高于底且下标低于底、上下标字号小于底、
  矩阵第 2 格在同一行右侧、第 3 格换行、根式恰有一条上横线与一个根号符号。
- 候选侧用 `canvas`（iced 的非默认 feature）绘制这些图元，见 `src/structure_view.rs`。

证据截图：[artifacts/iced-window-default.png](../../spikes/native-ui/candidate-iced/artifacts/iced-window-default.png)
显示分数线、`x` 的上下标、2 列矩阵网格与根号上横线，均非纯文本投影。
布局器是**刻意简化**的（固定字形宽度、无换行与真字体度量），够用于候选比较，不足以评估排版质量。

**修正（2026-09-16，用户指出"公式渲染都有错位"）**：初版布局把 `Item::Text` 的纵坐标当作
绘制框上沿，上下标位置又是用临时常数凑的，导致上标抬一整行、下标降一整行
（上标基线偏高 24.6px、下标偏低 14.4px；修正后为 0.42em / 0.20em）。
改法是把文本纵坐标改为**基线**并由渲染方统一换算。

**但这套渲染整体是临时的，不是设计。** 固定字形宽度、无换行、根号用文本字符加横线拼出，
只为让各候选画同一份图元从而可比。正式渲染等语义树（AST）与渲染分层确定后重做。

### 输入法：一次实测发现的误路由与修复

第一次注入拼音时暴露了一个真实缺陷，值得完整记录：

- **现象**：`preedit="a b ni hao"` 与 `错误：源码面板为只读：拒绝非活动语言的写入，控件内容已回滚`
  交替出现；提交后的中文落在**正文**里，同时源码控件也被拒绝并回滚。
- **原因**：输入法事件是**窗口级**的，不带目标控件信息。当时源码面板即使只读也用 `text_editor` 渲染，
  它持有键盘焦点并代表窗口请求了输入法；而应用的订阅把窗口级 `Preedit`/`Commit` 一律当成正文区输入。
- **修复**：① 只读面板改用文本视图——只读面板本来就不该是可编辑控件；
  ② 应用显式跟踪输入焦点所在区域（正文 / 源码），只把属于正文区的事件交给核心；
  ③ 正文区通过自定义控件 `ime_host::ImeHost` **自己请求输入法**。

第 ③ 点是框架层面的关键结论：**iced 只在控件显式调用 `Shell::request_input_method` 时才开启输入法**
（`iced_winit` 中 `set_ime_allowed(true/false)`）。自绘的结构编辑器必须自己请求，否则窗口收不到
`Preedit`/`Commit`。好消息是 iced 支持 on-the-spot 预编辑
（`InputMethod::Enabled { cursor, purpose, preedit }`），代价是实现者要自己维护光标几何。

修复后实测：

```text
preedit="ni hao"（未进历史）
preedit=""（未进历史）
core: ImeCommit "打我" → action ActionId(11) revision 12
```

界面计数 `预编辑事件 30 / IME 提交 1 / 核心动作 11 / 错误 无`：30 次预编辑产生 **0** 个核心动作，
提交恰好产生 **1** 个动作，正文收到文本，源码面板保持原样。
证据截图：[artifacts/iced-ime-test.png](../../spikes/native-ui/candidate-iced/artifacts/iced-ime-test.png)。

### 可访问性：实测不通过（框架级缺口）

判据原文是"键盘遍历、焦点通知、角色/名称/文本与选区暴露 → 真实系统辅助功能检查，有缺口就记录失败"。
本机 AT-SPI 基础设施在运行（`org.a11y.Bus`、`at-spi2-registryd`，niri 提供
`org.freedesktop.a11y.Manager`），因此可以做真实检查。

**方法**：`scripts/a11y-test.sh` + `scripts/a11y-probe.py`（`python-atspi`）。先由合成器确认候选窗口
确实存在（列出 `Scholium spike — Iced candidate`），再遍历 AT-SPI 桌面对象树。

**结果**：AT-SPI 桌面只列出 5 个应用——`xdg-desktop-portal-gtk`、`com.follow.clash`、`qs`、
`Avalonia Application`、`Unnamed`；**候选完全不在其中**。为排除"探针本身失效"的误判，
对同一时刻的其他应用做了对照转储：

```text
APP 'Avalonia Application' childCount=1
    WIN role=frame  name='G-Helper - ASUS TUF Gaming F16 FX607JV_FX607JV'  childCount=2
        - role=panel name='WindowChrome'
        - role=panel name='Panel'
APP 'Unnamed' childCount=1
    WIN role=frame  name='dsh web'  childCount=1
        - role=panel name=''
```

其他工具包（Avalonia、Web/Electron）都能正常发布 frame/panel 及名称，说明探针有效；
候选是**完全没有对象**，不是命名或匹配问题。

**静态证据**（本地已下载源码，可复核）：

- `iced 0.14.0`、`iced_core`、`iced_widget`、`iced_winit` 中 `accesskit` 出现 **0 次**，
  也没有 `a11y` / `accessibility` feature。
- 其依赖的 `winit 0.30.13` 源码中同样 **0 次** 提及 `accesskit`，feature 列表里也没有对应项。
  也就是说，即使想在候选侧自行接入，窗口层也没有现成钩子。

**后果**：屏幕阅读器读不到候选窗口的角色、名称、文本与选区——不是"标签不全"，而是整棵树不存在。
要达标本项目要求，需要 iced/winit 上游提供 AccessKit 集成，或自行实现 AT-SPI 桥，
属于上游工程量级，不是候选层打补丁能解决的。

**证据强度说明**：这次实测发生在一个最小候选上——当时正文甚至连结构都没渲染（见上一节）。
因此"该候选不发布可访问对象树"这一现象证据充分，但"一个功能完整的编辑器在 iced 上**还**会暴露哪些
可访问性缺口"不能由此推断，那需要候选本身足够完整。框架层面的结论不受影响：
iced 与 winit 源码中都不存在 accesskit 集成，任何候选实现都无从发布可访问树。

**测试前提更正（2026-09-16 补充）**：上面的实测是在**会话无障碍处于关闭状态**下做的
（`org.a11y.Status IsEnabled = false`）。AccessKit 类框架此时不会向 AT-SPI 注册，因此
"看不到对象"在那种条件下是必然的——对 iced 而言结论不变（它本就没有 accesskit），
但**当时的测试前提记录有误**，不能作为"框架不支持无障碍"的独立证据。

在**启用无障碍**后重测，iced 仍不出现在 AT-SPI 应用列表中，静态证据与实测因此一致。
后续所有无障碍测试都必须在 `IsEnabled = true` 下进行；`a11y-probe.py` 已改为先打印该开关并在
关闭时明确提示"这是假阴性"。详见[报告 0004](0004-native-ui-egui.md) 的"测试方法错误"一节。

外部参考（**未在本机验证**，仅作背景）：iced 上游有开放的
[accessibility 支持 issue #552](https://github.com/iced-rs/iced/issues/552) 与
[accessibility RFC 草案](https://raw.githubusercontent.com/iced-rs/rfcs/d25c20f726db173c6b6d36e458199ef03d1a7e7f/text/0000-accessibility.md)。

**已更正**：本节初稿曾写"下一个候选 GPUI 有基于 AccessKit 的可访问性实现"，并据此推断
"可访问性成为区分候选的关键判据"。核实已发布的 `gpui 0.2.2` 后该推断**作废**——发布版 `gpui`
同样没有 accesskit（`Cargo.toml`、feature、源码、整棵依赖树计数均为 0），AT-SPI 实测也读不到
GPUI 候选窗口。Zed 仓库里有 a11y 示例，但没有进入发布 crate。

结论：**可访问性不能区分这两个候选，两者都是 Fail**，详见[报告 0002](0002-native-ui-gpui.md)。

## 结果

模型层：`cargo test --offline` **20 passed / 0 failed**。
候选冒烟：**窗口创建成功、合成器已列出、0 条 error、SIGTERM 干净退出**。
证据截图：[artifacts/iced-window-default.png](../../spikes/native-ui/candidate-iced/artifacts/iced-window-default.png)
（默认后端，wgpu → Vulkan）与
[artifacts/iced-window-tiny-skia.png](../../spikes/native-ui/candidate-iced/artifacts/iced-window-tiny-skia.png)
（软件后端），两者都只含本应用窗口。

| 验收项（计划第 4 节） | 结果 | 证据 / 缺口 |
|---|---|---|
| 中文与 Unicode | **Pass（真实输入法）** | 经 fcitx5 注入拼音：30 次预编辑产生 0 个核心动作，提交"打我"恰好产生 1 个动作（ActionId(11) / revision 12），正文收到文本、源码面板不变（截图 `artifacts/iced-ime-test.png`）。界面中文与标签无 `.notdef` 方块。模型层另有 ZWJ emoji、组合字符不被按字节拆坏的测试。缺口：删除/光标在长组合串下的行为仅在模型层验证。 |
| 数学结构 | **部分 Pass** | **结构渲染已实测**：共用布局器把语义图布局成图元，`canvas` 绘出分数线、上下标错位、矩阵网格与根号（截图 + `core/tests/layout.rs` 6 个断言结构的测试）。导航（分子/分母、base/sub/sup、矩阵单元格）、wrap/unwrap 保留内容、变体循环、多槽结构拒绝无策略展开在模型层验证；UI 已接按钮与方向键，但未做人工交互验收。缺口：无真实字距与行内换行，长公式不折行。 |
| 源码视图 | **部分 Pass** | 面板显示 `[只读]`，非活动语言写入被拒绝并把控件回滚到权威缓冲（截图 + 代码路径）。缺口：语法高亮、10 万行滚动与真实编辑手感未测。 |
| 团队规则 | **部分 Pass** | 远端 actor 注入的夹具留在历史但**不进本地 undo scope**：界面显示"动作 10 / 本地历史为空"；模型层另有"远端插入不被本地撤销删除"测试。缺口：真实多端同步与语言切换属阶段 0 第 6 项。 |
| 核心集成 | **Pass（本轮范围）** | 焦点直接显示核心 `Cursor`，revision 与动作数来自 core；每次编辑产生 Action 并推进 revision；过期 base revision 返回 `StaleRevision`。缺口：框架快捷键绕过项目历史的验证见剩余工作。 |
| 预览 | **Blocked** | 仅投影占位，界面明确标注"占位，不冒充最终排版"。真实 Typst 编译与点击定位属阶段 0 第 2 项。 |
| 大源码 | **部分 Pass（模型层）** | 10 万行 / 4.99 MB：构建 **23 ms**，单次局部插入 **180 µs**（debug 构建）。缺口：UI 滚动与软件后端的重排延迟未测。 |
| 可访问性 | **Fail** | 候选**不在 AT-SPI 桌面对象树中**，屏幕阅读器读不到任何角色、名称、文本与选区。同期其他应用（Avalonia、Electron）可正常发布 frame/panel；iced/iced_core/iced_widget/iced_winit 与 winit 0.30.13 源码中 `accesskit` 出现 0 次，也无 a11y feature。属框架级缺口，非候选层可补。 |
| 文件恢复 | **Blocked** | 核心不含持久化与 IO；属阶段 0 第 5 项。 |
| 构建与依赖 | **Pass** | `iced 0.14.0` 全树编译通过；构建与运行只需 cargo，无 Node/npm/WebView。默认（Vulkan）与 `tiny-skia` 两种后端都渲染正确（截图 `artifacts/iced-window-default.png`、`iced-window-tiny-skia.png`）。直接依赖许可证：`thiserror`、`unicode-segmentation`、`proc-macro2`、`quote`、`syn` 为 `MIT OR Apache-2.0`，`unicode-ident` 为 `(MIT OR Apache-2.0) AND Unicode-3.0`。 |

许可证检查的一个实际产出：`unicode-ident` 使用 SPDX `Unicode-3.0`，而 `AGENT.md` 的允许清单原先只列了旧的
`Unicode-DFS`，已补上 `Unicode-3.0`。

## 失败与不确定性

- **可访问性不通过**，这是本报告判 Fail 的原因，且属框架级缺口（见上节）。
- 预览点击定位未测：需要真实 Typst 布局，属阶段 0 第 2 项。
- 输入注入依赖 `ydotool` 与可用的 `/dev/uinput`，并且需要能稳定取得窗口焦点；
  桌面会话里有其他窗口抢焦点时脚本会中止（这是有意的安全行为）。
- 本报告的 GUI 测量都在 agent 沙箱内完成，沙箱对 `/dev` 的可见性会直接改变渲染后端结论（见上节）。
  在普通会话复现时以 `gpu_probe` 输出为准。
- 模型层的撤销用字符身份近似 CRDT 相对位置，`TextDeleted` 的恢复锚点只记录右邻单一身份；
  这在验收场景下正确，但**不是** CRDT 收敛证据（属第 4 项）。
- 结构编辑的反向配方标记为不可逆，补偿历史的完整语义属阶段 4。
- 性能数字来自 debug 构建、单次采样，且核心性能与 UI 性能不是一回事。
- 窗口被 niri 平铺为 1270×1494，与请求的 1280×820 不同；布局验收需在固定几何下重做。

## 对设计的影响

- 核心与 UI 分离、候选各自独立 workspace 的结构可行，未出现反向依赖。
- **布局必须属于共用核心，不能属于候选。** 只投影纯文本会让"数学结构"验收退化成模型层测试，
  也让候选比较失去意义。后续候选（GPUI 等）必须绘制同一份 `core::layout` 结果，否则结论不可比。
- 自绘结构需要框架提供自定义绘制能力；iced 需要显式启用非默认 `canvas` feature。
- 固定槽位 + 空占位节点足以支撑数学结构导航与字素安全编辑，UI 无需保存第二份权威状态。
- 输入法必须在应用层区分 `Preedit` 与 `Commit`，否则预编辑会污染历史；这一条已由实现与模型测试共同固定。
- 阶段 0 结论必须分别记录每个候选使用的渲染后端；"能开窗口"不等于 GPU 路径可用。

## 剩余工作

若后续仍考虑 Iced，需要先解决 Fail 项：

1. **可访问性**：等待或推动上游 AccessKit 集成；在窗口层没有钩子的情况下自行实现 AT-SPI 桥。
   这是 Fail 项，不是待测项。
2. **预览点击定位**：需要真实 Typst 布局，属阶段 0 第 2 项。
3. **框架快捷键绕过**：确认 Ctrl+Z 不会被 Iced 默认绑定截走而不经过 `Editor::undo`。
4. **性能**：在两种后端下分别测量真实输入延迟与 20 页预览延迟。本次只测了冷启动与静态窗口 CPU，
   不足以判定任何预算。
5. **固定窗口几何**重做布局与滚动验收。
6. **输入法光标几何**：当前 `ImeHost` 传的是近似矩形，真实编辑器需按 caret 位置计算候选窗避让区域。

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

# 3. 先确认 GPU 对该进程可见，再跑冒烟与截图（只截本应用窗口）
cargo run --manifest-path spikes/native-ui/candidate-iced/Cargo.toml --bin gpu_probe
bash spikes/native-ui/candidate-iced/scripts/smoke.sh default
bash spikes/native-ui/candidate-iced/scripts/compare-backends.sh 3

# 4. 输入法验证（需要 ydotool 与正在运行的 ydotoold）
sudo pacman -S --needed ydotool && systemctl --user start ydotool
bash spikes/native-ui/candidate-iced/scripts/input-test.sh

# 5. 可访问性检查（需要 python-atspi 与运行中的 AT-SPI 总线）
sudo pacman -S --needed python-atspi
bash spikes/native-ui/candidate-iced/scripts/a11y-test.sh
```
