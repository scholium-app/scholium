# 页面编辑模式命名统一

本次将模式名统一为“页面编辑 / Page Editing”，保留“源码预览 / Source Preview”，
同步了模式提示、只读原因、枚举、命令注册表、焦点栏与窗口/A11y 检查消费者，以及七份指定现行文档。
编辑位置的区别写入了体验规范，分段控件仍按文字测宽且每段至少 96 逻辑像素。

在 `crates/`、`spikes/`、`scripts/` 检索了旧模式命令 ID；引用仅属于内存注册表、
Fluent key、控件审计与测试脚本，未见设置、用户键位或会话数据库的持久化路径。
据此将模式与视图命令枚举成员改为 `Page`，命令 ID 改为 `view.mode.page`。

## 验证记录

- 先将运行时切换语言的标签断言更新为新名称，旧实现按预期失败；修改后通过。
- 全量测试发现原有状态栏测试扫描整窗并禁止“页”字，误判了新的模式标签；
  将负向断言收窄到实际页码格式，源码预览页码的正向断言保留。
- `cargo fmt --all -- --check`：通过。
- `cargo clippy --locked --workspace --all-targets -- -D warnings`：通过。
- `cargo test --locked --workspace`：444 通过，2 忽略，0 失败；其中 app 302 通过。
- `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps`：通过。
- `python3 scripts/check-rust-size.py`：通过。
- `python3 scripts/check-doc-links.py`：通过。
- `python3 scripts/check-ui-strings.py`：通过。
- `python3 scripts/check-layers.py`：通过。
- `cargo deny --locked check licenses bans sources`：通过；已有未使用许可证 allowance 提示。
- `cargo build --locked -p scholium-app -p scholium-storage --bins`：通过。
- `python3 scripts/window-checks.py --locale zh-Hans en --out target/t21-window`：30/30 通过（每种语言 15 项；私有 Xvfb）。

首次全量窗口运行在中文 `sharp-raster` 长文档步骤超时；随后确认 `/tmp` 写入报
`disk quota exceeded`，且 app 的最后一条 AUDIT 被截断。保留首轮证据于
`target/t21-window-first/`，将输出迁至 slot 的 `target/` 后重新执行全套双语检查，
未改检查阈值或超时设置。

四个中英文分段标签均通过最小宽度、单行且无省略断言；真实窗口截图见
[中文](evidence/page-editing-mode/zh-Hans.png)与[英文](evidence/page-editing-mode/en.png)，
截图来自各语言 chrome 检查的 `01-chrome.png`。完整窗口日志与证据留在 `target/t21-window/`。
本次只改命名及消费者，未改编辑面的几何、渲染或依赖，因此未触发 stock-Typst 对照要求。

全仓旧术语检索保留 16 个命中：11 个属于既有 ADR/日志，2 个属于任务明确排除的其他产品描述，
3 个属于固定夹具正文及其精确白名单。每个位置的保留理由列在 PR 中，现行模式名称没有遗漏。
未改写历史记录、任务书、夹具正文，也未修改其他执行者的目录；没有遗留待办。
