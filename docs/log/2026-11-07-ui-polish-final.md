# T48 界面精修交付记录

## 范围

本次从 `e625189`（PR #105 合并后的 `origin/main`）开始，覆盖源码面包屑、两类编辑面命令框、状态栏与提示审查、深色/窄窗口可读性，以及 18 项历史审查发现的复核。界面实现在 app；面包屑的轻量结构扫描作为 `scholium-latex` / `scholium-typst` 的小型辅助接口。

## 证据

按评审保留一套最终 after：每个主界面的 zh/en、light/dark、1440×900，1024/480 仅保留三种内核主窗口。before 仅保留下表直接比较的 4 张。PNG 用 `magick -strip +dither -colors 256 PNG8:` 量化，不保留初轮或中间运行文件。

| 对照场景 | 之前 | 之后 |
| --- | --- | --- |
| 中文深色 LaTeX 480 | [before](evidence/ui-polish-final/before/zh-Hans/tex/tex-source-dark-480.png) | [after](evidence/ui-polish-final/after/zh-Hans/tex/tex-source-dark-480.png) |
| 英文浅色 LaTeX 1440 | [before](evidence/ui-polish-final/before/en/tex/tex-source-light-1440.png) | [after](evidence/ui-polish-final/after/en/tex/tex-source-light-1440.png) |
| 中文深色命令框 1440 | [before](evidence/ui-polish-final/before/zh-Hans/native/command-dark-1440.png) | [after](evidence/ui-polish-final/after/zh-Hans/native/command-dark-1440.png) |
| 英文深色 Typst 源码 480 | [before](evidence/ui-polish-final/before/en/typ/typ-source-dark-480.png) | [after](evidence/ui-polish-final/after/en/typ/typ-source-dark-480.png) |

1440×900 最终主界面：

| 场景 | 中文浅色 | 中文深色 | 英文浅色 | 英文深色 |
| --- | --- | --- | --- | --- |
| 原生页面 | [zh light](evidence/ui-polish-final/after/zh-Hans/native/native-page-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/native/native-page-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/native/native-page-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/native/native-page-dark-1440.png) |
| 原生公式 | [zh light](evidence/ui-polish-final/after/zh-Hans/native/native-math-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/native/native-math-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/native/native-math-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/native/native-math-dark-1440.png) |
| 导航 | [zh light](evidence/ui-polish-final/after/zh-Hans/native/navigation-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/native/navigation-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/native/navigation-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/native/navigation-dark-1440.png) |
| 生成源码 | [zh light](evidence/ui-polish-final/after/zh-Hans/native/generated-source-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/native/generated-source-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/native/generated-source-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/native/generated-source-dark-1440.png) |
| 页面命令框 | [zh light](evidence/ui-polish-final/after/zh-Hans/native/command-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/native/command-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/native/command-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/native/command-dark-1440.png) |
| 起始页 | [zh light](evidence/ui-polish-final/after/zh-Hans/native/start-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/native/start-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/native/start-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/native/start-dark-1440.png) |
| 新建 | [zh light](evidence/ui-polish-final/after/zh-Hans/native/new-project-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/native/new-project-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/native/new-project-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/native/new-project-dark-1440.png) |
| 打开 | [zh light](evidence/ui-polish-final/after/zh-Hans/native/open-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/native/open-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/native/open-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/native/open-dark-1440.png) |
| LaTeX 源码 | [zh light](evidence/ui-polish-final/after/zh-Hans/tex/tex-source-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/tex/tex-source-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/tex/tex-source-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/tex/tex-source-dark-1440.png) |
| LaTeX 预览 | [zh light](evidence/ui-polish-final/after/zh-Hans/tex/tex-preview-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/tex/tex-preview-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/tex/tex-preview-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/tex/tex-preview-dark-1440.png) |
| LaTeX 问题面板（真实 LuaLaTeX） | [zh light](evidence/ui-polish-final/after/zh-Hans/tex/tex-problems-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/tex/tex-problems-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/tex/tex-problems-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/tex/tex-problems-dark-1440.png) |
| 项目设置 | [zh light](evidence/ui-polish-final/after/zh-Hans/tex/settings-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/tex/settings-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/tex/settings-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/tex/settings-dark-1440.png) |
| 外部冲突 | [zh light](evidence/ui-polish-final/after/zh-Hans/tex/conflict-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/tex/conflict-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/tex/conflict-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/tex/conflict-dark-1440.png) |
| LaTeX 命令框 | [zh light](evidence/ui-polish-final/after/zh-Hans/tex/command-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/tex/command-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/tex/command-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/tex/command-dark-1440.png) |
| Typst 源码 | [zh light](evidence/ui-polish-final/after/zh-Hans/typ/typ-source-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/typ/typ-source-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/typ/typ-source-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/typ/typ-source-dark-1440.png) |
| Typst 预览 | [zh light](evidence/ui-polish-final/after/zh-Hans/typ/typ-preview-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/typ/typ-preview-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/typ/typ-preview-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/typ/typ-preview-dark-1440.png) |
| Typst 问题面板 | [zh light](evidence/ui-polish-final/after/zh-Hans/typ/typ-problems-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/typ/typ-problems-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/typ/typ-problems-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/typ/typ-problems-dark-1440.png) |
| Typst 命令框 | [zh light](evidence/ui-polish-final/after/zh-Hans/typ/command-light-1440.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/typ/command-dark-1440.png) | [en light](evidence/ui-polish-final/after/en/typ/command-light-1440.png) | [en dark](evidence/ui-polish-final/after/en/typ/command-dark-1440.png) |

主窗口的较窄视图（高 700 px）：

| 内核 | 宽度 | 中文浅色 | 中文深色 | 英文浅色 | 英文深色 |
| --- | --- | --- | --- | --- | --- |
| 原生 | 1024 | [zh light](evidence/ui-polish-final/after/zh-Hans/native/native-page-light-1024.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/native/native-page-dark-1024.png) | [en light](evidence/ui-polish-final/after/en/native/native-page-light-1024.png) | [en dark](evidence/ui-polish-final/after/en/native/native-page-dark-1024.png) |
| 原生 | 480 | [zh light](evidence/ui-polish-final/after/zh-Hans/native/native-page-light-480.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/native/native-page-dark-480.png) | [en light](evidence/ui-polish-final/after/en/native/native-page-light-480.png) | [en dark](evidence/ui-polish-final/after/en/native/native-page-dark-480.png) |
| LaTeX | 1024 | [zh light](evidence/ui-polish-final/after/zh-Hans/tex/tex-source-light-1024.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/tex/tex-source-dark-1024.png) | [en light](evidence/ui-polish-final/after/en/tex/tex-source-light-1024.png) | [en dark](evidence/ui-polish-final/after/en/tex/tex-source-dark-1024.png) |
| LaTeX | 480 | [zh light](evidence/ui-polish-final/after/zh-Hans/tex/tex-source-light-480.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/tex/tex-source-dark-480.png) | [en light](evidence/ui-polish-final/after/en/tex/tex-source-light-480.png) | [en dark](evidence/ui-polish-final/after/en/tex/tex-source-dark-480.png) |
| Typst 源码 | 1024 | [zh light](evidence/ui-polish-final/after/zh-Hans/typ/typ-source-light-1024.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/typ/typ-source-dark-1024.png) | [en light](evidence/ui-polish-final/after/en/typ/typ-source-light-1024.png) | [en dark](evidence/ui-polish-final/after/en/typ/typ-source-dark-1024.png) |
| Typst 源码 | 480 | [zh light](evidence/ui-polish-final/after/zh-Hans/typ/typ-source-light-480.png) | [zh dark](evidence/ui-polish-final/after/zh-Hans/typ/typ-source-dark-480.png) | [en light](evidence/ui-polish-final/after/en/typ/typ-source-light-480.png) | [en dark](evidence/ui-polish-final/after/en/typ/typ-source-dark-480.png) |

共 100 张 PNG（before 4、after 96）；[清单](evidence/ui-polish-final/manifest.json) 记录每张量化后图片的尺寸与 SHA-256。布局参考为 [LaTeX 工作台](../plan/assets/p1-ui/2026-11/latex-workbench.png) 与 [公式命令框](../plan/assets/p1-ui/2026-11/typst-math-focus.png)。清单仅收录当前保留的截图，并区分基线、真实诊断和本轮构建的来源。

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
| 1 | LaTeX 错误未进入问题面板 | 已修复：#105；T48 复测真实编译诊断与截图；[最终界面](evidence/ui-polish-final/after/zh-Hans/tex/tex-problems-dark-1440.png) |
| 2 | `.typ` 源码缺预览及初始焦点 | 已修复：#104；T48 双主题三宽度复测；[最终界面](evidence/ui-polish-final/after/zh-Hans/typ/typ-source-dark-1440.png) |
| 3 | 打开对话框 Esc 不关闭 | 已修复：#103；T48 对话框矩阵复测；[最终界面](evidence/ui-polish-final/after/zh-Hans/native/open-dark-1440.png) |
| 4 | LaTeX 页面编辑切换无效 | 已修复：#104；T48 全宽预览复测；[最终界面](evidence/ui-polish-final/after/zh-Hans/tex/tex-preview-dark-1440.png) |
| 5 | 480 px 对话框越界 | 已修复：#103；T48 边界断言复测；[最终界面](evidence/ui-polish-final/after/zh-Hans/native/new-project-dark-1440.png) |
| 6 | 外壳未按项目语言装配 | 已修复：#101/#103/#104/#105；T48 三内核复核；[最终界面](evidence/ui-polish-final/after/zh-Hans/tex/tex-source-dark-1440.png) |
| 7 | LaTeX 面板显示 Typst Content | 已修复：#104/#105；T48 复核；[最终界面](evidence/ui-polish-final/after/zh-Hans/tex/tex-problems-dark-1440.png) |
| 8 | 大量禁用入口与空菜单 | 已修复：#101；T48 去除重复标签提示，保留不可用原因；[最终界面](evidence/ui-polish-final/after/zh-Hans/native/native-page-dark-1440.png) |
| 9 | 开发 revision/cache/layout 外露 | 已修复：#104/#105；仅 AUDIT 保留；[最终界面](evidence/ui-polish-final/after/zh-Hans/tex/tex-problems-dark-1440.png) |
| 10 | 生成源码、导航与默认标签术语 | 已修复：#104；T48 复核；[最终界面](evidence/ui-polish-final/after/zh-Hans/native/generated-source-dark-1440.png) |
| 11 | LaTeX 工具链与引擎标签混乱 | 已修复：#103/#105；T48 状态复核；[最终界面](evidence/ui-polish-final/after/zh-Hans/tex/tex-source-dark-1440.png) |
| 12 | 引擎/同步/模式切换提示含混 | 已修复：#101/T48；增加引擎设置动作提示，去重复提示；[最终界面](evidence/ui-polish-final/after/zh-Hans/native/native-page-dark-1440.png) |
| 13 | 生成源码缺行号/高亮/当前块 | 已修复：#104；T48 生成源码矩阵复测；[最终界面](evidence/ui-polish-final/after/zh-Hans/native/generated-source-dark-1440.png) |
| 14 | LaTeX 固定矮源码框、无行号/分隔调整 | 已修复：#104；T48 布局窗口检查复测；[最终界面](evidence/ui-polish-final/after/zh-Hans/tex/tex-source-dark-1440.png) |
| 15 | 预览缩放/适合宽度/空状态 | 已修复：#104；T48 三内核预览矩阵复测；[最终界面](evidence/ui-polish-final/after/zh-Hans/tex/tex-preview-dark-1440.png) |
| 16 | 新建/打开对话框字段与操作层次 | 已修复：#103；T48 深浅主题和窄窗复测；[最终界面](evidence/ui-polish-final/after/zh-Hans/native/new-project-dark-1440.png) |
| 17 | 外部冲突说明与按钮层次 | 已修复：#103；T48 真实外部编辑复测；[最终界面](evidence/ui-polish-final/after/zh-Hans/tex/conflict-dark-1440.png) |
| 18 | 锁定项目/空起始页缺操作 | 已修复：#103；T48 起始页矩阵及全量锁定检查复测；[最终界面](evidence/ui-polish-final/after/zh-Hans/native/start-dark-1440.png) |

## 结论

面包屑结构不执行源码：LaTeX 只扫描章节和环境，Typst 使用官方 CST 的 heading/math/raw；未提交 IME 时不跳转。状态栏继续按整段优先级删除，不裁剪字符；窄窗零问题隐藏，非零问题保留严重级别图标。命令框显示符号、命令名、类别和单行键提示，底部空间不足时整体翻到光标行上方。

## 范围例外与布局取舍

- 按 T48 授权，在 `scholium-latex` / `scholium-typst` 新增只读面包屑辅助接口、单测和对应模块规范；没有新增 UI 内解析器，也没有改动存储接口。
- `scripts/window-checks.py` 只为 `ui-polish-final` 和 `problem-panel` 指定 1440×900 的私有 Xvfb 根屏幕，使 1440 px 窗口截图不被默认 1280 px 根屏幕裁掉；自动发现、全量执行、锁和失败策略保持原规则。
- LaTeX 章节使用源码标题，不猜宏展开后的编号；公式处暂不加阶段 3 的页面编辑操作。窄面包屑省略中间祖先，文件与当前元素保留。
- 主界面继续保留四层顶部外壳与正文优先布局；命令框通过空间判断翻转而不是覆盖光标。对照设计稿仅调整可用性和对比度，没有修改 P1_UI 规划。

## 验证

初次交付的完整门禁已通过：workspace 740 通过、16 忽略；完整窗口 64 项均有通过证据。固定构建使用 `t48-delivery-app` 文件名时 AT-SPI 两项因硬编码查找 `scholium-app` 应用名失败；相同二进制改为该名称后双语 2/2 通过。仅保留最终断言汇总：[中文 AT-SPI](evidence/ui-polish-final/a11y-zh-Hans-summary.json)、[英文 AT-SPI](evidence/ui-polish-final/a11y-en-summary.json)，不归因于 Xvfb，不保留中间或初轮运行文件。

真实 LuaLaTeX 检查双语 2/2、各 22/22 断言通过：[native 汇总](evidence/ui-polish-final/native-window-checks.txt)、[中文断言](evidence/ui-polish-final/latex-live-zh-Hans-summary.json)、[英文断言](evidence/ui-polish-final/latex-live-en-summary.json)。它覆盖错误与未定义引用、行选择、成员文件跳转、清除问题和主题菜单；最终只保留主问题面板的 1440×900 深浅主题截图。

独立 stock Typst 系统字体和仅打包字体均 31/31 像素一致：[system](evidence/ui-polish-final/stock-parity-system.json)、[bundled](evidence/ui-polish-final/stock-parity-bundled.json)。本次仅改无损字体容器，不改变解压后的字体字节或正文 FontBook。

### 评审修订

数学字体以 `NotoSansMath-Regular.ttf.br` 保存：原始 990,564 bytes，压缩 390,512 bytes；原始 SHA-256 为 `d51afd5739c7ba6c44fcab35a88160e25dfb69a2d4ad0bd99533f8d894af1f96`。复用现有纯 Rust Brotli 解压函数和进程内缓存；新增单测校验原始长度、SHA-256 和缓存复用，原有候选字形测试通过。界面加载路径和资产说明同步更新，OFL 许可保留。

本次按要求重跑 fmt、clippy、workspace tests、Rust 文件规模、文档链接以及 `ui-polish-final` 双语窗口检查，均通过；workspace 为 741 通过、0 失败、16 忽略。窗口检查为双语 2/2 通过（[最终运行记录](evidence/ui-polish-final/window-checks.txt)）。窗口专项断言汇总：[中文](evidence/ui-polish-final/ui-polish-zh-Hans-summary.json)、[英文](evidence/ui-polish-final/ui-polish-en-summary.json)。截图更新后再次运行文档链接检查。证据目录为 100 PNG（4 before、96 after），整目录大小约 1.8 MB，低于 3 MB。不合并 main，不创建 PR。

## PR 正文素材

**问题**：源码焦点缺结构导航，命令候选和提示层次不统一，部分配色与候选字形不完整；原始证据与字体资产体积过大。

**改动**：内核只读面包屑与跳转、共用命令框、状态栏动作与提示、深浅配色和数学字体回退；证据精简为最终 96 张 after 和 4 张直接比较的 before，PNG 统一 256 色，字体改为 Brotli 无损容器。Scoped exceptions 为任务允许的内核导航接口及模块文档、共用字体解压入口，以及窗口 runner 的屏幕尺寸配置。

**验证**：fmt、clippy、workspace tests（741/0/16）、Rust 文件规模、文档链接和双语 `ui-polish-final` 均通过。证据目录 100 张 PNG、约 1.8 MB；数学字体原始哈希和候选 glyph 测试通过。

**待办**：本任务无新增未完成项。LaTeX 不展开宏或推断章节编号，页面公式编辑仍属阶段 3。PR 由协调者创建。
