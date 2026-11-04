# T46：源码编辑器与预览窗格

日期：2026-11-04。分支：`ation_ciger/feat/source-preview-layout`。基线：`8713f75839795f009faa963a5a3fd0e072becf2a`（PR #103 合并）。

## 问题

普通 `.typ` 项目只有未主动聚焦的最小输入框，没有真实预览。LaTeX 源码编辑器未占满工作区，页面编辑开关仍留下源码窗格；
各源码预览缺少统一分隔交互、实际缩放与页面导航。原生生成源码需要保留只读、块高亮和主题更新，并补齐行号与语法样式。

## 改动

- 三种源码视图采用平面两栏和 1 px 分隔线；分隔条支持拖动、聚焦后的左右键、双击复位，按项目根目录原子保存本机比例。
- 共享源码编辑器占满窗格，增加行号、当前行底色、真实诊断的独立标记槽和悬停消息。只对当前资源和 revision 装饰，不展示陈旧错误。
- 预览居中，使用页面真实 pt 尺寸响应状态栏缩放、100% 和适合宽度，并提供前后页。失败保留最后成功页面、显示版本与问题入口。
- LaTeX 缺工具链时展示真实 ToolchainDetector 的安装建议与重新检测；页面编辑显示全宽预览与阶段 3 说明，切回恢复分隔。live/full 诊断接入共享 gutter。
- `.typ` 以 SourceProject 为唯一权威，打开即聚焦，保留私人草稿和 IME 提交语义；真实 Typst 编译结果按 revision 接收，旧结果丢弃。
- 原生生成源码保留“由文档生成 · 只读”、块高亮和点击预览定位；新增官方 Typst 语法高亮和行号。缓存 LayoutJob，不跨 atlas/主题持有 Galley。

## 实现边界与范围例外

T46 允许的窄内核例外为 `crates/scholium-typst/src/source_syntax.rs`、`source_preview.rs`、导出和单元测试，
以及现有栅格函数的 crate 内可见性；同步更新 `docs/modules/typst.md`。未更改 fork、依赖锁文件或独立 stock reference。

普通外部源码不能交给仅接受可信生成源码的进程内 PreviewCompiler。新增固定 app worker 模式，复用锁定的 Typst 编译、诊断映射与栅格，
通过 Linux bwrap 与 systemd 用户 scope 隔离。此跨模块执行边界记录在 [ADR 0055](../adr/ADR-0055-source-preview-worker.md)，
未搬运废弃原型，也未更改 TeX 沙箱接口。以上作为 PR 的 scoped exception 明列。

worker 无网络、无宿主任意文件或包缓存能力，只读显式项目资源；150 ms 去抖，取消旧任务时终止整个 scope。
限制为 4 GiB 内存/地址空间、零 swap、64 tasks、CPU 30 秒、墙钟 10 秒、128 fd；输入 16 MiB/512 普通文件，
输出最多 100 页、RGBA 总量 64 MiB、单边 8192 px，栅格化前拒绝超限。

## 验证

先红后绿证据：

- [编辑器高度失败](evidence/2026-11-04-source-preview-layout/red-editor.log)：原编辑器高度 457.75，不能填满 900 px 面板。
- [页面编辑布局失败](evidence/2026-11-04-source-preview-layout/red-page-mode.log)：源码在页面模式仍可见。
- [源码快照 API 缺失](evidence/2026-11-04-source-preview-layout/red-source-snapshot.log)：新行为测试在实现前编译失败。

Rust/静态门禁记录见 [gates.json](evidence/2026-11-04-source-preview-layout/gates.json)，对应原始日志同目录：

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --locked --workspace` | PASS，708 passed / 0 failed / 15 ignored |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | PASS |
| `python3 scripts/check-rust-size.py` | PASS |
| `python3 scripts/check-doc-links.py` | PASS |
| `python3 scripts/check-ui-strings.py` | PASS |
| `python3 scripts/check-layers.py` | PASS |
| `cargo deny --locked check licenses bans sources` | PASS |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | PASS |

stock 比较按 `typst-fork.yml` 的正式 Content adapter 与独立 reference 两组命令执行；
[系统字体](evidence/2026-11-04-source-preview-layout/stock-system.json)和
[仅打包字体](evidence/2026-11-04-source-preview-layout/stock-bundled.json)各 31/31 用例像素完全相同。未改阈值或 reference。

窗口检查执行 `python3 scripts/window-checks.py --locale zh-Hans en --out target/t46-all-windows-1`，
全部 30 项 × 双语言运行，原有 58 项全部通过；新增中文版超时夹具先命中 Typst 自身循环保护，造成首次 59/60。
夹具改为昂贵的有限计算，未改产品或放宽阈值；随后用同一 runner 重跑 `--only source-preview-layout --locale zh-Hans en`，
两种语言全部通过，最终覆盖 60/60 项。保留[首次全量日志](evidence/2026-11-04-source-preview-layout/windows-full.log)与
[最终复跑日志](evidence/2026-11-04-source-preview-layout/windows-focused.log)，不隐藏夹具修正过程。

新检查每个语言覆盖 16 项断言：打开聚焦与真实编译更新、全高、拖动/键盘、持久化、双击复位、缩放/fit、
语法失败保留页图及诊断、恢复、分页、拒绝宿主文件、昂贵计算 10 秒超时、超时后恢复、LaTeX 缺工具链、
全宽页面模式、切回恢复分隔、生成源码只读。所有窗口在私有 Xvfb 下，经机器锁顺序运行。
详情见 [zh-Hans](evidence/2026-11-04-source-preview-layout/window-zh-Hans.json)与
[en](evidence/2026-11-04-source-preview-layout/window-en.json)。

基线截图由保留的 main `8713f75` 二进制通过同一夹具的截图模式生成，仅跳过新增行为断言；
其 SHA-256、测试与性能汇总见 [summary.json](evidence/2026-11-04-source-preview-layout/summary.json)。
截图含 `.typ`、LaTeX 缺工具链和原生生成源码，覆盖 zh/en、light/dark：

| 场景 | zh light | zh dark | en light | en dark |
|---|---|---|---|---|
| Typst 源码 | [前](evidence/2026-11-04-source-preview-layout/before/zh-Hans/source-light.png) / [后](evidence/2026-11-04-source-preview-layout/after/zh-Hans/source-light.png) | [前](evidence/2026-11-04-source-preview-layout/before/zh-Hans/source-dark.png) / [后](evidence/2026-11-04-source-preview-layout/after/zh-Hans/source-dark.png) | [前](evidence/2026-11-04-source-preview-layout/before/en/source-light.png) / [后](evidence/2026-11-04-source-preview-layout/after/en/source-light.png) | [前](evidence/2026-11-04-source-preview-layout/before/en/source-dark.png) / [后](evidence/2026-11-04-source-preview-layout/after/en/source-dark.png) |
| LaTeX | [前](evidence/2026-11-04-source-preview-layout/before/zh-Hans/latex-light.png) / [后](evidence/2026-11-04-source-preview-layout/after/zh-Hans/latex-light.png) | [前](evidence/2026-11-04-source-preview-layout/before/zh-Hans/latex-dark.png) / [后](evidence/2026-11-04-source-preview-layout/after/zh-Hans/latex-dark.png) | [前](evidence/2026-11-04-source-preview-layout/before/en/latex-light.png) / [后](evidence/2026-11-04-source-preview-layout/after/en/latex-light.png) | [前](evidence/2026-11-04-source-preview-layout/before/en/latex-dark.png) / [后](evidence/2026-11-04-source-preview-layout/after/en/latex-dark.png) |
| 生成源码 | [前](evidence/2026-11-04-source-preview-layout/before/zh-Hans/generated-light.png) / [后](evidence/2026-11-04-source-preview-layout/after/zh-Hans/generated-light.png) | [前](evidence/2026-11-04-source-preview-layout/before/zh-Hans/generated-dark.png) / [后](evidence/2026-11-04-source-preview-layout/after/zh-Hans/generated-dark.png) | [前](evidence/2026-11-04-source-preview-layout/before/en/generated-light.png) / [后](evidence/2026-11-04-source-preview-layout/after/en/generated-light.png) | [前](evidence/2026-11-04-source-preview-layout/before/en/generated-dark.png) / [后](evidence/2026-11-04-source-preview-layout/after/en/generated-dark.png) |

另附 [.typ 错误保留页](evidence/2026-11-04-source-preview-layout/after/zh-Hans/source-error.png)与
[LaTeX 全宽页面模式](evidence/2026-11-04-source-preview-layout/after/zh-Hans/latex-page-mode.png)。

## 5,000 行测量

[性能日志](evidence/2026-11-04-source-preview-layout/performance.log)对应行为测试
`a_single_edit_in_five_thousand_lines_reuses_unchanged_layout_rows`：5,000 行样本，冷语法与布局 109.679 ms，
单行修改 31.579 ms，复用 5,000/5,001 个布局行对象。高亮范围排序后使用前向扫描，egui 保留换行段落的布局缓存。
这是 debug 单元测试中的内核语法与 egui 布局耗时，未声称为真实屏幕端到端延迟或统计性能基准。

## 待办与限制

- 外部 `.typ` 预览目前需要 Linux 的 bwrap、systemd 用户 scope 与 prlimit；其他平台或缺隔离能力时显式失败，不在 UI 进程降级执行。
- worker 使用打包/embedded 字体与显式资源白名单，不接用户字体、系统时间、网络或包缓存；每次请求启动 worker，暂不保留跨进程编译缓存。
- 问题面板重做与诊断跳转属于第 4 批；LaTeX 页面公式编辑属于阶段 3。本任务未扩大这些范围。

## PR 正文材料

**问题**：源码编辑与预览缺少一致的布局和操作；`.typ` 没有真实预览，LaTeX 页面模式未成为全宽预览。

**改动**：统一全高源码面板、行号/诊断/语法高亮、持久化可键盘操作分隔条、预览缩放与分页；
接通隔离的真实 Typst 源码预览和 last-good 行为，补齐 LaTeX 工具链空态与全宽页面模式。
Scoped exception：最小 `scholium-typst` 语法/快照 API 及测试、其模块文档、ADR 0055 的执行边界；未改其他执行器所有的模块。

**验证**：上述全部 Rust/静态门禁通过，workspace 708 passed / 0 failed；stock 系统/打包字体各 31/31 完全一致；
5,000 行单次修改复用 5,000/5,001 行。窗口最终覆盖 60/60（全量 59/60 后修正新增超时夹具，受影响检查双语言复跑 2/2、32 个断言）；前后截图覆盖 zh/en、light/dark。

**待办**：跨平台源码 worker、包缓存与用户字体集成另行实现；问题面板和阶段 3 公式编辑保持后续任务范围。
