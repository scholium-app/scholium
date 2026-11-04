# 界面精修第一批：令牌、组件与内核装配

任务：协调器下发的 T42-ui-foundation。工作分支 `ation_ciger/feat/ui-foundation`，
工作槽 `/home/ation_ciger/Projects/Mogan/scholium-wt1`。开始基线为 `d11c0c60ca198b9f3997887473f24df3a176ef03`；
期间快进合入文档更新 `c94bb32bbd45eb168001874bcefcc55900728653`（PR #99）。
本次未改 `docs/plan/P1_UI.md`，未改其他执行器负责的内核、诊断模型或构建 crate。

## 问题

旧界面展示未实现的命令和文档属性，源码项目沿用原生 Typst 的操作栏；
诊断右列与 LaTeX 头部暴露开发信息。文件对话框缺少统一的 Enter/Esc 策略，480 px 窗口存在截断风险。
令牌与 2026-11-03 修订不符，项目名和状态栏也没有按当前权威模型装配。

## 改动

- 更新浅色语义色、深色边框、36/32 px 工具栏高度、圆角和弹出层阴影；栏、纸面和面板保持平面。
- 共享按钮、输入框、下拉、分段、弹出层与对话框；所有既有文件/冲突/迁移/错误/丢弃确认及快捷键帮助复用弹窗壳。
  标题关闭、18 px 内边距、主按钮最右、Enter 主操作、Esc 取消、窗口减32宽度上限、短窗口滚动。
- 命令声明适用内核，统一过滤菜单、工具栏、焦点栏、溢出和快捷键；隐藏未实现命令和空菜单。
  源码撤销重做使用 TextEdit 历史并恢复源码焦点，不调用 Typst 会话历史。
- 删除禁用的文档属性、诊断开发列与 LaTeX 调试头部。项目导航和标签使用项目名，默认项目名本地化。
  原始开发数据继续进入 `SCHOLIUM_EDITOR_AUDIT`，新增独立布局统计与弹窗边界记录。
- 按三种内核装配位置、字数、上下文、问题计数、引擎状态与缩放。问题计数包含零条，窄窗口保留可操作入口。
  提示使用操作说明、快捷键和不可用原因，不仅重复标签。新增文字覆盖中英 catalog。

## 验证

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 通过 |
| `cargo test --locked --workspace` | 通过 |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | 通过 |
| `python3 scripts/check-rust-size.py` | 通过 |
| `python3 scripts/check-doc-links.py` | 通过 |
| `python3 scripts/check-ui-strings.py` | 通过 |
| `python3 scripts/check-layers.py` | 通过 |
| `cargo deny --locked check licenses bans sources` | 通过 |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | 通过 |

工作区测试共652项通过，其中应用338项；统计包含各 crate 的单元/集成/doc-test。
[门禁记录](evidence/2026-11-03-ui-foundation/gates.json)。
stock Typst 对照：System 31/31、Bundled 31/31 `equal_pixels=true`，没有修改参考或放宽阈值。
[System](evidence/2026-11-03-ui-foundation/stock-system.json) / [Bundled](evidence/2026-11-03-ui-foundation/stock-bundled.json)。

窗口验收：中英各28项（含新增 ui-foundation 和 AT-SPI），最终56/56均通过。
首次完整运行47/56；其中8项受到外部Xvfb终止影响，英文AT-SPI因旧预期失败。
更新AT-SPI预期后，对7种受影响检查及新增ui-foundation在中英各运行一次，16/16通过。
[逐项汇总](evidence/2026-11-03-ui-foundation/windows.json) /
[首次完整输出](evidence/2026-11-03-ui-foundation/windows-initial.log) /
[重跑输出](evidence/2026-11-03-ui-foundation/windows-recheck.log)。所有真实窗口运行使用私有 Xvfb 和 `/tmp/scholium-window-checks.lock`，未接触桌面。
测试先复现了命令搜索仍可见，以及源码工具栏撤销焦点恢复过晚的问题，再修复；后者的失败结果为 `DraftX != Draft`；另补测源码失焦后状态栏行列由1:7错误归零为1:1，修复后通过。
最近项目长路径原先把480 px弹窗撑宽到约3579 px，限制按钮宽度并截断标签后，
单元测试通过；中英专项增加最近项目弹窗边界断言。
[长路径失败记录](evidence/2026-11-03-ui-foundation/long-recent-red.log)。
[撤销失败记录](evidence/2026-11-03-ui-foundation/source-history-red.log) / [行列失败记录](evidence/2026-11-03-ui-foundation/source-position-red.log)。

专项窗口验收（中英）已通过内核装配、源码撤销重做、快捷键帮助输入隔离、新建/打开/另存为Esc、
480 px Enter新建、冲突Enter另存为与Esc保留外部磁盘内容。
最初专项6项中5项通过，中文chrome因外部`Xvfb Killed`失败；完整套件重新覆盖它。
完整套件也遇到外部按进程名终止Xvfb。保留原始中断记录，重跑使用系统`/usr/bin/Xvfb`的逐字节副本`target/t42-xserver`，
仅改变进程名，避免外部脚本误杀；仍经原始`xvfb-run`、私有display和机器共享锁。
两文件SHA-256相同：`6584e6cf04ca1357cb50fd2feb1ad449b5f45dfe6cc61d2bbf006f752df592de`。
隔离进程名没有改变检查逻辑、像素阈值、超时或基准文件。
长路径修复在完整套件运行期间完成，新增ui-foundation已以最终二进制在中英环境复核；
已执行的其他窗口场景不受长路径截断影响，普通按钮尺寸与行为保持一致。

## 修改前后截图

截图由相同检查脚本驱动同一组抛弃项目。基线以 `d11c0c6` 源码重建，最终截图来自本分支二进制；
基线/最终二进制及44张PNG的SHA-256见[清单](evidence/2026-11-03-ui-foundation/screenshots.json)。
基线部分只记录画面，最终部分同时断言内核装配、撤销/重做、模态键盘和480 px边界。

### zh-Hans

| 场景 | 修改前 | 修改后 |
|---|---|---|
| Typst浅色 | ![修改前](evidence/2026-11-03-ui-foundation/before/zh-Hans/typst-page-light.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/zh-Hans/typst-page-light.png) |
| Typst深色 | ![修改前](evidence/2026-11-03-ui-foundation/before/zh-Hans/typst-page-dark.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/zh-Hans/typst-page-dark.png) |
| 公式焦点 | ![修改前](evidence/2026-11-03-ui-foundation/before/zh-Hans/formula-focus.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/zh-Hans/formula-focus.png) |
| 菜单 | ![修改前](evidence/2026-11-03-ui-foundation/before/zh-Hans/menu.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/zh-Hans/menu.png) |
| 诊断面板 | ![修改前](evidence/2026-11-03-ui-foundation/before/zh-Hans/diagnostics.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/zh-Hans/diagnostics.png) |
| 新建对话框 | ![修改前](evidence/2026-11-03-ui-foundation/before/zh-Hans/dialog-new.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/zh-Hans/dialog-new.png) |
| LaTeX工作台 | ![修改前](evidence/2026-11-03-ui-foundation/before/zh-Hans/latex-workbench.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/zh-Hans/latex-workbench.png) |
| .typ项目 | ![修改前](evidence/2026-11-03-ui-foundation/before/zh-Hans/typ-project.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/zh-Hans/typ-project.png) |
| 480 px窗口 | ![修改前](evidence/2026-11-03-ui-foundation/before/zh-Hans/window-480.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/zh-Hans/window-480.png) |
| 480 px对话框 | ![修改前](evidence/2026-11-03-ui-foundation/before/zh-Hans/dialog-480.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/zh-Hans/dialog-480.png) |
| 外部冲突 | ![修改前](evidence/2026-11-03-ui-foundation/before/zh-Hans/external-conflict.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/zh-Hans/external-conflict.png) |

### en

| 场景 | 修改前 | 修改后 |
|---|---|---|
| Typst浅色 | ![修改前](evidence/2026-11-03-ui-foundation/before/en/typst-page-light.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/en/typst-page-light.png) |
| Typst深色 | ![修改前](evidence/2026-11-03-ui-foundation/before/en/typst-page-dark.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/en/typst-page-dark.png) |
| 公式焦点 | ![修改前](evidence/2026-11-03-ui-foundation/before/en/formula-focus.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/en/formula-focus.png) |
| 菜单 | ![修改前](evidence/2026-11-03-ui-foundation/before/en/menu.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/en/menu.png) |
| 诊断面板 | ![修改前](evidence/2026-11-03-ui-foundation/before/en/diagnostics.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/en/diagnostics.png) |
| 新建对话框 | ![修改前](evidence/2026-11-03-ui-foundation/before/en/dialog-new.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/en/dialog-new.png) |
| LaTeX工作台 | ![修改前](evidence/2026-11-03-ui-foundation/before/en/latex-workbench.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/en/latex-workbench.png) |
| .typ项目 | ![修改前](evidence/2026-11-03-ui-foundation/before/en/typ-project.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/en/typ-project.png) |
| 480 px窗口 | ![修改前](evidence/2026-11-03-ui-foundation/before/en/window-480.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/en/window-480.png) |
| 480 px对话框 | ![修改前](evidence/2026-11-03-ui-foundation/before/en/dialog-480.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/en/dialog-480.png) |
| 外部冲突 | ![修改前](evidence/2026-11-03-ui-foundation/before/en/external-conflict.png) | ![修改后](evidence/2026-11-03-ui-foundation/after/en/external-conflict.png) |

## 规格缺口与后续边界

- `.typ` 预览属于第三批，当前显示“Typst · 尚未生成预览”，避免显示错误的已同步状态。
- 项目设置尚未实现，后端状态保持提示而不提供无效点击入口。
- 原生编辑面连续、不分页，页码暂为1/1；生成的分页预览可用时显示其页码。
- LaTeX 问题数来自既有工作台错误；共享诊断和问题面板接入属于第四批，未越界依赖另一个槽的新类型。
- 源码/预览布局和模式分段保持既有行为，布局重整属于第三批；深色除边框外保留原有颜色。
- 移除占位项后主工具栏在480 px可以完整放下；整组溢出逻辑继续由250 px单元测试覆盖。
- 额外共享文件之一为 `scripts/ui-strings-allowlist.txt`：仅删去已移除导航文字对应的两条过期豁免，
  否则 UI 字符串门禁会因 stale allowlist 失败。
- 另一处最小范围例外是嵌套的 `spikes/typst-main-editor/a11y/a11y_check.py`：旧检查要求未实现删除线、
  文档/工具菜单和搜索仍存在，与本次隐藏规则冲突。实测先失败后，改成断言未实现操作不进入可访问树；
  保留已实现不可用redo的禁用断言（同时检查ENABLED与SENSITIVE），保留开关角色和全部有效控件命名验收。
  未改门禁规则或像素阈值。

## 待办

T42范围内实现和本地验收已完成；后续批次按上述边界推进。分支推送后由协调器审查并创建PR，
本执行器没有创建PR、合并或轮询CI。

## 变更范围

应用变更位于 `crates/scholium-app/**`；窗口期望更新为 `chrome-check.py`，新增 `ui-foundation-check.py`。
模块规范更新 `docs/modules/app.md`，本日志与证据记录验证。另有上文列明的两处最小文件范围例外。
未修改Cargo.lock、内核/诊断/构建crate，也未新增依赖。

## 协调器复核修复：窄窗口状态栏

协调器指出480 px中文状态栏把“正文”裁到半个字形，裸“0”也无法表达问题数量。
本轮先加入失败回归测试，再改为先绘制右侧控件、按其实际左边界保留空间；
左侧标签按完整宽度计算，空间不足时依次隐藏上下文、字数、页码（源码项目最高优先级为行列），不裁切标签。
计数始终带最严重问题的图标；窄窗口零问题隐藏整个计数，非零显示图标加数字，宽窗口保留完整标签。
共享状态按钮为图标和文字分配独立空间；可访问名称仍为完整的问题数量。

新增优先级边界单元测试，以及中英480 px零/非零问题和文字裁剪边界测试。
[修复前失败记录](evidence/2026-11-03-ui-foundation/status-narrow-red.log)。
本轮十项非窗口门禁再次全部通过，工作区654项测试通过（应用340项）；
[本轮门禁](evidence/2026-11-03-ui-foundation/status-narrow-gates.json)。
本轮运行 `python3 scripts/window-checks.py --only chrome diagnostics ui-foundation source-theme --locale zh-Hans en`，
使用同一私有Xvfb/共享锁及先前的进程名隔离，8/8通过，无重试、无阈值调整。
[本轮窗口输出](evidence/2026-11-03-ui-foundation/status-narrow-windows.log)。
已刷新 `after/zh-Hans/window-480.png` 与 `after/en/window-480.png` 两张截图；
逐张目视确认标签完整、窄窗口没有零问题计数。截图清单已更新对应哈希和本轮二进制哈希。

推送修复 `4856098` 前的最后一次fetch发现主线合入诊断流水线（`80722fd`）；
依共享约定以带sign-off的merge同步主线，无冲突，未手工修改其他执行器的文件。
同步后再次通过全部十项非窗口门禁，工作区668项测试通过；
[合并后门禁](evidence/2026-11-03-ui-foundation/status-merged-gates.json)。
因主线更新了Typst适配及Cargo.lock，另重跑System/Bundled stock对照，各31/31完全一致：
[System](evidence/2026-11-03-ui-foundation/status-merged-stock-system.json) /
[Bundled](evidence/2026-11-03-ui-foundation/status-merged-stock-bundled.json)。

主线同步后的完整中英窗口套件56/56一次通过：
[合并后完整窗口输出](evidence/2026-11-03-ui-foundation/status-merged-windows.log)。
两张480 px截图已再次由主线同步后的最终二进制刷新，哈希同步更新。
