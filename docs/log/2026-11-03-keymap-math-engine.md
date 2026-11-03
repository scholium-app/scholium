# T36：键位表与共享数学输入引擎

任务：`T36-keymap-math-engine`，工作槽 `scholium-wt2`，基线 `891be743767a4ddd0b68d703cefac7153a3e7cf6`。
实现依据为 ADR 0048 和数学输入计划的第 1 批。本文只记录实现与验证，不作为产品规范。

## 问题

命令注册表与会话按键分派分别硬编码绑定，修改键位无法同步菜单、帮助和实际输入。
数学输入规则留在 Typst 会话内，另一语言无法复用；Tab 只遍历叶子，缺少符号循环与命令补全。

## 改动

- 默认键位由嵌入 TOML 按稳定命令 id 定义；Ctrl/Alt/Shift/Meta、global/page/math/source/focus 作用域，
  同作用域冲突、未知命令与无效覆盖均拒绝。用户配置逐条替换或解除，错误保持默认并显示诊断。
  菜单、悬停、帮助从生效表生成。旧数学键直接移除；原生剪贴板事件也按覆盖表重新分派。
- 新增层 1 的 `scholium-math-input`：独立编写的 Unicode/语言名称、组合、循环、结构槽和命令名数据，
  文档无关的事件/上下文/意图接口，以及共享按键序列到归一化公式树的验收合约。
- 分数、根式、定界符、上下标、独立公式迁移到共享意图，Typst 适配器继续使用唯一权威文档与原子 StructuralEdit。
  临时 query/cycle 状态纳入失败回滚，不进入保存。IME 提交与粘贴保持字面输入。
- 实现核心组合、Tab 补全/双向循环/槽位前进与焦点提示、页面正文/公式反斜线命令浮层、四种括号配对。
  末槽后停在结构 after edge；下标后追加上标扩展同一 Attach。窄焦点栏命令放入“»”，不遮挡模式切换。
- 实际窗口复现并修复 Linux Alt 按键额外产生文字事件的问题。浮层仅显示 query/候选；接受文字仍仅由 Typst 绘制。

## 验证

行为测试先失败再实现。保留首次失败的键位/引擎/适配器编译验收、Alt 额外字母与窄焦点栏重叠记录，
见 [evidence](evidence/2026-11-03-keymap-math-engine/alt-red.log) 与
[窄焦点栏失败](evidence/2026-11-03-keymap-math-engine/focus-red.log)。

全工作区测试：553 通过、4 项原有忽略。包含共享 9 个公式树夹具、数据一致性与前缀冲突、覆盖替换/解除/失败回退、
作用域、平台修饰键映射、剪贴板无旧别名、IME/粘贴字面行为、Tab 各分支、四种配对与原有完整回归。

| 门禁命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --locked --workspace` | PASS，553 通过、4 忽略 |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | PASS |
| `python3 scripts/check-rust-size.py` | PASS |
| `python3 scripts/check-doc-links.py` | PASS |
| `python3 scripts/check-ui-strings.py` | PASS，既有白名单仍为 47 |
| `python3 scripts/check-layers.py` | PASS |
| `cargo deny --locked check licenses bans sources` | PASS，既有 unused allowance 警告 |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | PASS |
| `python3 scripts/window-checks.py --locale zh-Hans en --out target/t36-window-all-1` | 首轮 43/46；修正检查脚本后受影响项双语 6/6，全部 46 个组合有通过记录 |
| `python3 scripts/window-checks.py --only backspace-block-merge fraction-at-caret a11y --locale zh-Hans en --out target/t36-window-rerun-1` | PASS，6/6 |

stock Typst 对照分别运行生产 fork example、独立 reference 与 `compare.py`，系统字体与 bundled-only 两种配置
各 31 个夹具均 `equal_pixels=true`，没有修改 reference 或阈值：
[系统字体](evidence/2026-11-03-keymap-math-engine/stock-comparison.json)、
[仅内置字体](evidence/2026-11-03-keymap-math-engine/stock-bundled-comparison.json)。

命令目录 `KeyChord::` 搜索只剩解析与测试，没有绑定字面量。`CommandSpec` 不含 shortcuts 字段。

## 真实窗口与证据

通过共享 runner 的机器级锁，使用私有 Xvfb、抛弃式数据库，未访问用户桌面。
新增 `math-input-check.py` 的四个场景已在中英文下通过。
[中文命令浮层](evidence/2026-11-03-keymap-math-engine/zh-command-overlay.png)、
[英文嵌套括号与焦点提示](evidence/2026-11-03-keymap-math-engine/en-brackets.png) 已检查；
[中文 AUDIT](evidence/2026-11-03-keymap-math-engine/zh-math-input-audit.json) 与
[英文 AUDIT](evidence/2026-11-03-keymap-math-engine/en-math-input-audit.json) 各保留 21 个状态。
完整套件共 23 项 × 2 种语言：[首轮日志](evidence/2026-11-03-keymap-math-engine/window-all.log) 为 43/46。
中文 backspace-block-merge 与 fraction-at-caret 失败是检查脚本把 TOML 的大写显示字母直接交给 xdotool，
隐式多带 Shift；修正为小写物理键并保留显式修饰键，函数键继续走 keycode。
中文 AT-SPI 失败是测试仍发送旧 `Ctrl+/`，已换为 `Alt+F`，
[失败证据](evidence/2026-11-03-keymap-math-engine/a11y-old-binding-red.log) 保留。
冻结预期未放宽。上述三项在全新目录中双语[复验 6/6](evidence/2026-11-03-keymap-math-engine/window-rerun.log)，
其余 43 项已在首轮通过；全部 46 个检查/语言组合有通过记录。
AT-SPI 两种语言均完成 11 条断言：
[中文](evidence/2026-11-03-keymap-math-engine/zh-a11y-summary.json)、
[英文](evidence/2026-11-03-keymap-math-engine/en-a11y-summary.json)。
最后一次 Rust 门禁后只改了上述 Python 检查与证据文档，Python 编译、文档链接和差异空白检查另行复核。

新增窗口检查逐步断言 Alt 公式/分数键、`RR`/`<=`、Tab 双向循环与槽位/after edge、正文命令 Tab 接受、
公式命令 Enter 接受、Esc 保持文档/revision、四种嵌套配对，以及 `page_text_draws == 0`。

## 文件边界与偏差

未修改 storage/build/latex 或 ADR 0049/0050。共享文件只追加 workspace member、层级登记、模块索引行及 cargo 生成的 lock 项。
为完成焦点提示与 i18n，最小扩展了 app 的焦点栏、EditorView 元数据、双语 catalog 和相应测试；
为旧场景改键，把冻结场景的硬编码和弦改成稳定命令 id，由窗口 runner 读取默认键位文件，核心 runner 直接执行同一命令。
嵌套 Tab 夹具显式包含结构 after edge。AT-SPI 与截图工具中的旧分数键也同步迁移。
未新增 ADR：键位文件结构与加载规则写入 app 模块规范。

## 待办与接受边界

- LaTeX 源码工作台尚不存在，本批未实现源码中的反斜线补全；LaTeX L3 适配器后续复用共享验收。
- 未裁决真实 fcitx5/niri 下公式 IME 开关策略；保留既有 IME 行为并运行窗口回归。Xvfb 的通过不等于真实桌面接受。
- macOS Cmd/Option/Meta 映射有代码层测试，没有原生 macOS/Windows 会话验收；Linux 本次实测 Alt 系绑定。
- 全集符号、结构变体/拆除、矩阵、Alt 结构遍历、改键 UI 均依任务留待后续批次。

## 提交与交付

按键位、引擎、迁移、交互分步签署提交，分支 `ation_ciger/feat/keymap-math-engine`。
依任务例外只推送分支，不创建 PR；协调者可直接以本文“问题 / 改动 / 验证 / 待办”组织 PR 正文。

## 协调者复审修正

将符号表 `latex` 统一为完整数学模式源码片段：命令包含反斜线，普通字符直接输出 `R`、`<`、`>`；
数据一致性校验拒绝裸命令名，模块规范明确适配器原样输出及 TeX 控制词连接边界。
命令框 query 非空时 Backspace 删除末字符，已空时按 Backspace 与 Esc 相同，关闭且不修改文档。
先新增数据、引擎及 app 输入测试，三项均在修正前失败；修正后运行
`cargo test --locked -p scholium-math-input -p scholium-session -p scholium-app`：396 通过、1 项既有忽略。
`cargo fmt --all -- --check`、`cargo clippy --locked --workspace --all-targets -- -D warnings`、
`RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` 均通过，
文档链接、Rust 文件规模及 `git diff --check` 通过。本轮依复审范围重跑受影响测试，没有重跑完整窗口套件。
