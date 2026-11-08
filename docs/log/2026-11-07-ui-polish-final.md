# T48 界面精修交付记录

## 范围

本次从 `e625189`（PR #105 合并后的 `origin/main`）开始，覆盖源码面包屑、两类编辑面命令框、状态栏与提示审查、深色/窄窗口可读性，以及 18 项历史审查发现的复核。界面实现在 app；面包屑的轻量结构扫描作为 `scholium-latex` / `scholium-typst` 的小型辅助接口。

## 证据

`docs/log/evidence/ui-polish-final/before/` 与 `after/` 保存 zh-Hans/en、light/dark、1440/1024/480 的主界面、源码/预览、问题面板、对话框、起始页和命令框截图，截图的最终数量见文末验证记录。新增 `ui-polish-final-check.py` 运行面包屑逐级跳转、顶部/底部命令框避让、对话框窄窗边界及主题矩阵。

| 代表场景 | 之前 | 之后 |
| --- | --- | --- |
| 中文深色 LaTeX 480 | [before](evidence/ui-polish-final/before/zh-Hans/tex/tex-source-dark-480.png) | [after](evidence/ui-polish-final/after/zh-Hans/tex/tex-source-dark-480.png) |
| 英文浅色 LaTeX 1440 | [before](evidence/ui-polish-final/before/en/tex/tex-source-light-1440.png) | [after](evidence/ui-polish-final/after/en/tex/tex-source-light-1440.png) |
| 中文深色命令框 1024 | [before](evidence/ui-polish-final/before/zh-Hans/native/command-dark-1024.png) | [after](evidence/ui-polish-final/after/zh-Hans/native/command-dark-1024.png) |
| 英文深色 Typst 源码 480 | [before](evidence/ui-polish-final/before/en/typ/typ-source-dark-480.png) | [after](evidence/ui-polish-final/after/en/typ/typ-source-dark-480.png) |

布局参考为 [LaTeX 工作台](../plan/assets/p1-ui/2026-11/latex-workbench.png) 与 [公式命令框](../plan/assets/p1-ui/2026-11/typst-math-focus.png)。真实界面保留项目当前已实现的操作；没有为复刻设计图加入不可用入口。

## 可读性

T48 调整了深色 `border` `#5A5A60 -> #96969D`、`text_dim` `#A6A6A6 -> #B6B6B6`、`text_faint` `#7E7E84 -> #B6B6B6`；浅色 `border` `#CFCFD4 -> #797981`、`text_faint` `#8E8E95 -> #5C5C63`。单测覆盖普通文字 4.5:1、控件边框/焦点环 3:1；控件按下/选中底色也单独检查。

按 sRGB 相对亮度计算，以下为 field / toolbar / focusbar / hover / pressed / active_bg 六类实际底色中的最低值；禁用文字与非交互分隔线不计入可操作边界。

| 配色 | 调整前最低对比度 | 调整后最低对比度 | 要求 |
| --- | --- | --- | --- |
| 深色边框 | 1.36:1 | 3.16:1 | 3:1 |
| 深色次级文字 | 3.82:1 | 4.58:1 | 4.5:1 |
| 深色弱文字 | 2.30:1 | 4.58:1 | 4.5:1 |
| 浅色边框 | 1.09:1 | 3.04:1 | 3:1 |
| 浅色弱文字 | 2.29:1 | 4.67:1 | 4.5:1 |

深色焦点环 `#6EA4FF` 未改色，最低 3.72:1。弹出层和对话框改用主题边框，提示中的快捷键及原因使用完整文字色。命令候选额外打包未修改的 Noto Sans Math 3.000 及 OFL 1.1；无系统字体单测覆盖全部候选符号，避免依赖开发机字体掩盖缺字。

## 18 项历史发现

| 编号 | 历史发现 | 状态与修复来源 |
| --- | --- | --- |
| 1 | LaTeX 错误未进入问题面板 | 已修复：#105；T48 复测真实编译诊断与截图 |
| 2 | `.typ` 源码缺预览及初始焦点 | 已修复：#104；T48 双主题三宽度复测 |
| 3 | 打开对话框 Esc 不关闭 | 已修复：#103；T48 对话框矩阵复测 |
| 4 | LaTeX 页面编辑切换无效 | 已修复：#104；T48 全宽预览复测 |
| 5 | 480 px 对话框越界 | 已修复：#103；T48 边界断言复测 |
| 6 | 外壳未按项目语言装配 | 已修复：#101/#103/#104/#105；T48 三内核复核 |
| 7 | LaTeX 面板显示 Typst Content | 已修复：#104/#105；T48 复核 |
| 8 | 大量禁用入口与空菜单 | 已修复：#101；T48 去除重复标签提示，保留不可用原因 |
| 9 | 开发 revision/cache/layout 外露 | 已修复：#104/#105；仅 AUDIT 保留 |
| 10 | 生成源码、导航与默认标签术语 | 已修复：#104；T48 复核 |
| 11 | LaTeX 工具链与引擎标签混乱 | 已修复：#103/#105；T48 状态复核 |
| 12 | 引擎/同步/模式切换提示含混 | 已修复：#101/T48；增加引擎设置动作提示，去重复提示 |
| 13 | 生成源码缺行号/高亮/当前块 | 已修复：#104；T48 生成源码矩阵复测 |
| 14 | LaTeX 固定矮源码框、无行号/分隔调整 | 已修复：#104；T48 布局窗口检查复测 |
| 15 | 预览缩放/适合宽度/空状态 | 已修复：#104；T48 三内核预览矩阵复测 |
| 16 | 新建/打开对话框字段与操作层次 | 已修复：#103；T48 深浅主题和窄窗复测 |
| 17 | 外部冲突说明与按钮层次 | 已修复：#103；T48 真实外部编辑复测 |
| 18 | 锁定项目/空起始页缺操作 | 已修复：#103；T48 起始页矩阵及全量锁定检查复测 |

## 结论

面包屑结构不执行源码：LaTeX 只扫描章节和环境，Typst 使用官方 CST 的 heading/math/raw；未提交 IME 时不跳转。状态栏继续按整段优先级删除，不裁剪字符；窄窗零问题隐藏，非零问题保留严重级别图标。命令框显示符号、命令名、类别和单行键提示，底部空间不足时整体翻到光标行上方。

## 范围例外与布局取舍

- 按 T48 授权，在 `scholium-latex` / `scholium-typst` 新增只读面包屑辅助接口、单测和对应模块规范；没有新增 UI 内解析器，也没有改动存储接口。
- `scripts/window-checks.py` 只为 `ui-polish-final` 和 `problem-panel` 指定 1440×900 的私有 Xvfb 根屏幕，使 1440 px 窗口截图不被默认 1280 px 根屏幕裁掉；自动发现、全量执行、锁和失败策略保持原规则。
- LaTeX 章节使用源码标题，不猜宏展开后的编号；公式处暂不加阶段 3 的页面编辑操作。窄面包屑省略中间祖先，文件与当前元素保留。
- 主界面继续保留四层顶部外壳与正文优先布局；命令框通过空间判断翻转而不是覆盖光标。对照设计稿仅调整可用性和对比度，没有修改 P1_UI 规划。

## 验证

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 通过 |
| `cargo test --locked --workspace` | 740 通过、0 失败、16 显式忽略 |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | 通过 |
| `python3 scripts/check-rust-size.py` | 通过 |
| `python3 scripts/check-doc-links.py` | 通过 |
| `python3 scripts/check-ui-strings.py` | 通过；39 项既有白名单 |
| `python3 scripts/check-layers.py` | 通过 |
| `cargo deny --locked check licenses bans sources` | 通过；既有未使用 Unicode-DFS-2016 allowance 提示 |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | 通过 |
| `python3 scripts/window-checks.py --locale zh-Hans en` | 完整 64 项均验证通过；固定最终构建完整运行 62 项通过，应用名修正后的同字节 AT-SPI 复测 2 项通过，详见下文 |

Typst 独立对照按 `typst-fork.yml` 的 fork-check / stock reference / compare 三条命令运行，系统字体与 `SCHOLIUM_TEST_FONTS=bundled` 均为 31/31 像素完全一致；不需要改变独立 reference 或阈值。
新增测试先观察失败再修复，覆盖面包屑跳转和窄宽度、两类弹出框避让、取消补全后保留源码焦点、Typst 片段、按下态对比度以及无系统字体候选字形。[先失败再修复的记录](evidence/ui-polish-final/test-first.txt) 与最终门禁分开保存，不能把开发中的预期失败计作最终检查结果。

### 窗口与截图结果

- 第一轮 `python3 scripts/window-checks.py --locale zh-Hans en --out target/t48-window-final` 为 64/64 通过，见 [初轮完整记录](evidence/ui-polish-final/window-checks-initial.txt)。期间补充了部分截图场景，因此交付截图统一以固定最终构建的复跑为准。
- 固定构建执行 `python3 scripts/window-checks.py --binary target/t48-delivery-app --locale zh-Hans en --out target/t48-delivery-windows`，62/64 通过，见 [完整复跑记录](evidence/ui-polish-final/window-checks-final.txt)。两项 AT-SPI 均在查找正文对象时超时：脚本硬编码按 `scholium-app` 找应用根对象，而固定二进制改名成了 `t48-delivery-app`。这不是产品代码修改；同一字节复制到 `target/t48-delivery/scholium-app`，SHA-256 均为 `7a323fabcc534a00b45f6a23ddfe1b150488922f9c2a4d8b0f314a14d4ae5a6b`，再单独复测两种语言的 a11y。未把此失败归因于 Xvfb 或外部 agent。
- `python3 scripts/window-checks.py --binary target/t48-delivery/scholium-app --only a11y --locale zh-Hans en --out target/t48-delivery-a11y` 为 2/2 通过，每种语言 11 条断言通过；见 [同字节 AT-SPI 复测](evidence/ui-polish-final/window-checks-a11y-recheck.txt)。最终构建完整 64 项因此均有通过证据，没有放宽检查或修改产品实现。
- T48 专项两种语言各 36/36 断言通过，覆盖面包屑跟随和逐级跳转、源码/页面命令框上下避让、四类对话框边界，以及主题/尺寸矩阵；[中文](evidence/ui-polish-final/ui-polish-zh-Hans-summary.json)、[英文](evidence/ui-polish-final/ui-polish-en-summary.json)。
- 真实 LuaLaTeX 扩展运行 `SCHOLIUM_T47_NATIVE=1 SCHOLIUM_T47_LIVE=1 SCHOLIUM_T48_MATRIX=1`，复用本机已验证 rtex PIN 和只用于检查的显式工具链路径，双语 2/2 检查、各 22/22 断言通过；见 [native 汇总](evidence/ui-polish-final/native-window-checks.txt)。包含错误和未定义引用、选中范围、成员文件跳转、清除问题、主题菜单及三宽度截图。标准全套中可选 native 条目的 skip 由本次真实补充检查覆盖。
- 基线二进制来自 `e625189`，通过 `SCHOLIUM_T48_BASELINE=1` 仅采集旧界面，不执行尚不存在的新接口断言，双语 2/2 采集通过；见 [before 记录](evidence/ui-polish-final/before-window-checks.txt)。没有切换或改写工作树来采集旧版。
- 提交截图共 534 张：before 224、after 310（其中主界面矩阵 224、真实诊断补充 86）。主界面覆盖两种语言、两套主题和 1440×900 / 1024×700 / 480×700；另有顶部/底部命令框和 480×900 诊断场景。全部 PNG 已解码验证，[manifest](evidence/ui-polish-final/manifest.json) 记录各文件尺寸、哈希与基线/最终二进制哈希。

## PR 正文素材

**问题**：源码焦点缺少结构导航，命令候选没有统一的光标避让与信息层次，部分深色文字/边界对比度不足，悬停提示存在重复标签。

**改动**：接入 LaTeX / Typst 内核只读面包屑与点击跳转；共用命令框显示符号、等宽查询、命令名和类别，底部自动翻转；整理状态栏动作与提示，保留整段优先级删除；修正深浅主题对比度和无系统字体下的候选缺字；补齐 18 项审查追溯与 534 张截图。Scoped exceptions 为任务允许的两个内核辅助接口及模块文档，以及窗口 runner 的两项屏幕尺寸配置。

**验证**：全部共享门禁通过；workspace 740 通过 / 0 失败 / 16 忽略；完整窗口 64 项均验证通过（最终固定构建 62/64，加同字节应用名修正后的 2/2 AT-SPI 复测）；真实 LuaLaTeX 双语 2/2，T48 专项各 36/36；系统/仅打包字体 stock 对照均为 31/31 像素一致。原始失败记录和复测均保留。

**待办**：本任务无未完成项。LaTeX 不展开宏或推断章节编号，页面公式编辑继续按任务边界留在阶段 3；未修改 P1_UI 规划。分支直接推送，PR 由协调者创建。
