# Spike 0021：原生编辑完整清单复核

- 日期：2026-09-17；基线 `c3552e6` 加本轮修复。
- **整体结论：Fail。** 已实现的六组桌面流程通过，但共同验收清单仍有缺失能力，不能宣布原生编辑全部通过。
- 阶段 0 出口仍为 **6 条限定范围满足、4 条部分满足**；本轮只集中处理原生编辑，不升级其它出口。
- 环境：Arch Linux、niri 26.04 / Wayland、fcitx5 5.1.22 + Rime 5.1.16；egui/eframe 0.36.2、AccessKit 0.24.1；Rust 1.98.0-nightly (2026-06-25)。
- Source Han Serif CN 字体；内置屏 2560×1600、1.5 倍缩放。键盘由 ydotool 注入，指针由 Python evdev/uinput 注入，AT-SPI 按被测 PID 查询。
- Windows/macOS：**Blocked（无本轮测试环境）**，不能由 Linux 结果推断通过。

## 对照共同清单逐项判定

以下十行对应 [原生 UI 验证计划 §4](../NATIVE_UI_VALIDATION.md)，不是阶段 0 十条出口条件。
“Pass”只覆盖本行明确列出的样本。缺少功能记 Fail，不能因其它 spike 的模型测试通过而放行桌面集成。

| 共同验收项 | 判定 | 证据与边界 |
|---|---|---|
| 中文与 Unicode | Pass（当前 Linux/Rime 样本） | 真实预编辑、Escape 取消、空格选词提交；正文组合时转源码，旧组合串不跨编辑区提交；中文、ZWJ emoji、组合字符通过系统剪贴板粘贴并连续退格 |
| 数学结构 | Fail（完整范围） | 分子/分母上下导航、分子嵌套根式、解除后继续输入、跨文本叶子拖选/剪切/撤销通过；**槽位端点选区仍明确拒绝**，整节点选择与完整槽位导航反馈尚未验收 |
| 源码视图 | Pass（受支持子集） | LaTeX/Typst 分别编辑并应用；未闭合片段拒绝，正文/历史不变、草稿保留；脏草稿下尝试切语言不生效。不是完整语言解析器，Raw 原样往返不代表所有语法均有效 |
| 团队规则 | Fail（桌面集成） | 原生候选只绑定一个本地 core；未接活动语言/epoch 控制器或可注入的远端 UI 会话。既有核心保留远端输入测试仍通过，但不冒充同语言多端桌面验收 |
| 核心集成 | Pass（本轮编辑路径） | 输入、IME 提交、选区替换、剪切经核心动作；预编辑不入历史，替换为单个动作；撤销追加补偿，修复后的光标可继续输入 |
| 预览 | Fail（完整共同范围） | 已有后台预览及旧 revision 拒绝证据；候选仍仅首页 Image，**缺点击定位、缩放和 20 页窗口交互**。实际呈现 p95 归独立出口条件 3 |
| 大源码 | Fail | 10 万行 release 无窗口逻辑侧局部编辑 p95 **32.25 ms / 30 样本**，超过 16 ms 输入目标；尚无真实大文件打开/滚动/编辑的桌面证据与增量解析 |
| 可访问性 | Fail（完整范围） | 正文 Text/选区、键盘 Tab 焦点、Math 角色及分子/分母/上下标/单元格描述通过真实 AT-SPI；禁用按钮仍被适配器报告 Enabled，且未作 Orca 等屏幕阅读器用户验收 |
| 文件恢复 | Fail（桌面集成） | 候选源码是内存 Session/TextEdit；没有打开/保存/重开/草稿恢复入口，未接 storage。独立 recovery spike 不能替代这些窗口动作 |
| 构建与依赖 | Pass（Linux 构建/启动） | Cargo 离线构建和真实启动通过，未增加应用依赖，不依赖 Node/npm/WebView；独立 crate 的既有 lint/格式失败另列，不宣称全部质量门禁通过 |

## 桌面执行证据

[六组结果](evidence/0021/results.json) 对应脚本的**限定场景**，全部通过并不等于上表十项全过：

1. `ime`：`nihao` 预编辑，正文和动作计数不变；Escape 取消；再次输入并空格选词得到“你好”，仅一个提交动作；`ni` 尚在组合时切源码，通知平台中断，源码不会收到旧候选。
2. `source_dialects`：LaTeX 和 Typst 分别在开头输入 CHECK 并应用；插入 `$broken(` 后拒绝应用，草稿保留；尝试切 LaTeX 仍保持 Typst 草稿。
3. `math_edit`：以 AT-SPI 光标进入分子，原生 Up/Down 在分子/分母间移动；按钮包裹 `sqrt(a)`、解除，直接键入 Q 并撤销，仍编辑原分子。
4. `accessibility`：读回文本选区、正文焦点；Tab 切出；真实树存在 Math 角色，名称包含结构描述。没有用按钮名称替代数学语义。
5. `pointer_selection`：真实按下/移动/释放从正文开头选到分子 a，读回偏移 0–8；Ctrl+X/Ctrl+Z；长文本输入使光标自动进入视口，再通过水平滚轮改变屏幕字符坐标。
6. `unicode_clipboard`：系统粘贴 `中👩‍💻é`，三次 Backspace 分别删组合字素、完整 ZWJ emoji、中文，最终恢复原正文；原文本剪贴板只在内存备份并恢复，不写入证据。

对应 [IME 日志](evidence/0021/ime.log)、[IME 截图](evidence/0021/ime.png)、
[数学对象树](evidence/0021/accessibility.json)、[鼠标日志](evidence/0021/pointer.log)、
[滚动坐标](evidence/0021/scroll.json)、[禁用状态差异](evidence/0021/disabled-state.json)。
脚本截图只截被测进程；临时启用无障碍/ydotool，退出恢复此前配置与输入法状态。
绝对指针设备在用后销毁，不修改用户 niri 鼠标加速配置。

## 本轮发现并修复

- 退格使用删除后的文本计算前一字素边界，可能跳过字素；同时没有折叠旧选区。改为编辑前计算，并同步光标/选区。
- 撤销后原 byte offset 可能超出文本；解除结构后焦点仍指向已脱离的包装节点。现在修复为有效字素边界/存活叶子，工具栏动作后焦点回正文。
- 替换选区先删除再插入产生两个动作。改为同一 `apply_batch`，一次撤销恢复原内容。
- 正文组合时切源码，仅清理本地 preedit 不够；真机截图曾出现旧候选“你”落到源码。增加 `should_interrupt_composition` 平台请求，再以完整源码不变的断言复测通过。
- IME 组合期间不执行正文方向键导航；失焦时清理正文组合状态。
- 点击定位原来按 Unicode scalar 取位置，会落在组合字素内部；改为已有 unicode-segmentation 的字素边界。
- 按下和移动处于同一帧时用当前指针位置当选区锚点；回归先失败，再改用 `press_origin`。
- 未闭合的未知语法片段曾被转成 Raw 并应用。现在在 Raw 转换前使用既有片段分隔符检查拒绝；这只是 spike 的保守片段门禁，**不是完整 LaTeX/Typst 语法验证**。
- 新增 Math 角色与按语义槽位生成的中文描述、近似逐字符几何；光标改变后滚动进入视口。没有提供完整 MathML、精确字体塑形或双向文本保证。

## 性能与质量门禁

10 万行夹具为 `source 0123456789\n` 重复 100000 次，约 1.8 MB；先布局，再输入 X，随后 30 次独立 y 编辑帧。
release 首次布局 52.28 ms，首个编辑帧 56.35 ms，后续编辑帧 p95 32.25 ms，进程 VmHWM 44352 kB。
这是当前机器的**无窗口逻辑帧**，不含 GPU、AT-SPI、磁盘打开或真实滚动，不拿它关闭呈现门禁。
[完整基准日志](evidence/0021/source-100k-release.log)。

- egui 常规测试 **29 通过、2 ignored**；其中 10 万行 benchmark 已按上述方式单独运行，另一个为既有预览集成测试。
- core **38/38**；source-reconcile 事务 **3/3**，两方言命令行夹具 **16/16**。
- egui 全 targets Clippy `-D warnings`、格式、构建通过；本轮改动的 core/reconcile Rust 文件 rustfmt 检查通过。
- 独立 core Clippy 仍有 **18 个既有错误**，source-reconcile 有 **2 个既有过长函数错误**；source-reconcile 整 crate fmt 也有既有差异。
  已从 `c3552e6` 导出干净基线并重现相同失败。本轮没有用依赖编译时的 lint 抑制冒充这些 crate 门禁通过。
- 没有重跑与本轮无关的全部阶段 0 安全、重建包与团队脚本。

## 复现

```bash
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"
cargo test --offline --manifest-path spikes/native-ui/core/Cargo.toml
cargo test --offline --manifest-path spikes/source-reconcile/Cargo.toml
cargo run --offline --manifest-path spikes/source-reconcile/Cargo.toml
cargo test --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
cargo clippy --offline --all-targets --manifest-path spikes/native-ui/candidate-egui/Cargo.toml -- -D warnings
cargo build --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
python3 spikes/native-ui/candidate-egui/scripts/native-edit-acceptance.py /tmp/scholium-native-edit
cargo test --release --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml source_100k_lines -- --ignored --nocapture
```

桌面脚本需要 niri、fcitx5/Rime、python-atspi、python-evdev、ydotool、wl-clipboard 和 `/dev/uinput` 权限。
当前指针脚本按本机单屏布局设计，多屏需先扩展坐标映射，不宣称多屏已通过。

## 后续阻断项

优先处理槽位端点/整节点选择与源码长文档输入；随后接完整预览定位、团队语言门禁和文件/草稿恢复窗口流程。
可访问性还需处理禁用状态适配和真实屏幕阅读器验收。上述工作是明确的未完成项，不能以本轮六组通过删除。
