# SPK-0060：删除旧页面编辑器并合并开发构建

- 日期：2026-10-10；结论：**Pass**。
- 对应：[ADR 0037](../adr/ADR-0037-remove-legacy-editor.md)、[ADR 0036](../adr/ADR-0036-typst-default-editor.md)、
  [主程序接入计划 E](../plan/TYPST_MAIN_APP_INTEGRATION.md)。
- 基线：`c48de54`（普通启动改用 Typst Content，报告 0059）；分支 `ation_ciger/refactor/remove-legacy-editor`。

## 改动

- 删除旧编辑面 `page_editor`、`session`（SessionBridge 与体验审计）、`workspace`、`preview`、`paper`、
  `sample`、`native_text`、`undo` 及测试；删除只驱动旧界面的 `scripts/audit-editor*.py`、
  `review-ribbon.py`、`ui-smoke.sh`。应用 crate 净删约 9 700 行，全仓约 −16 800 / +550 行（含开发 manifest 锁文件与旧界面脚本）。
- `main` 直接持有唯一的 `EditorSession`；`--legacy`、`typst-editor` 特性移除。`state` 只保留视图偏好与
  编辑器元数据；`chrome` 状态栏、`commands`、Ribbon 开始/插入改为只服务该编辑面；`Candidate*` 更名为
  `EditorSession`/`EditorView`/`SessionError`。未使用的主题色槽、页面尺寸常量与分屏图标一并删除。
- 删除 `dev/typst-editor`：fork 像素对照为 `scholium-typst` 的 `typst-main-fork-check` 示例；来源校验移至
  `scripts/verify-typst-fork.py`；CI 合并为根 workspace 上的 `typst-editor-window` 作业。
- 迁移所需的旧格式读取保留；启动失败页不再提示旧编辑器，只提示移走旧文件。`default-launch-check.py`
  改用存储层 `hammer` 写入真实 schema 0 旧会话作为迁移夹具。
- 旧编辑面测试的替代：新增 `typst_editor/window_tests.rs`（快捷键 + chrome + Ribbon + 编辑面同帧：
  三种窗口尺寸下编辑面/导航面板/结构视图不越界、视图快捷键不改文档、Ribbon intent 按序作用于会话、
  Ctrl+S 清 dirty、从结构视图返回后可直接键入、结构视图中排队的 Ribbon intent 不写正文），
  Ribbon 自身测试改为纯视图状态。

代码评审后补充：`Ctrl+1`/视图按钮回到编辑面时请求页面焦点（结构视图不绘制页面，egui 会丢弃其焦点，
原先需要再点一次才能键入）；会话侧拒绝非编辑面模式下的结构命令，不只依赖按钮禁用；计划文档的
接入点改指 `typst_editor`，`scholium-typst` 文档标注旧预览栈无应用调用方；两个依赖旧界面的历史
spike 脚本改为明确退出。

## 复现

```sh
cargo +1.96.0 fmt --all -- --check
cargo +1.96.0 clippy --locked --workspace --all-targets -- -D warnings
cargo +1.96.0 test --locked --workspace
RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace --no-deps
python3 scripts/check-rust-size.py
cargo deny --locked --config deny.toml check licenses bans sources
cargo deny --locked --config deny.toml check advisories
cargo +1.96.0 build --locked -p scholium-app -p scholium-storage --bins
python3 scripts/verify-typst-fork.py /tmp/typst-origin   # 上游 9dfd3a0… 浅克隆
cargo run --locked -p scholium-typst --features editor --example typst-main-fork-check -- /tmp/p-fork
cargo run --locked --manifest-path spikes/typst-main-editor/reference/Cargo.toml --target-dir target/stock-reference -- /tmp/p-stock
python3 spikes/typst-main-editor/compare.py /tmp/p-fork /tmp/p-stock
xvfb-run -a -s '-screen 0 1280x900x24' python3 spikes/typst-main-editor/native-check.py target/debug/scholium-app /tmp/n
xvfb-run -a -s '-screen 0 1280x900x24' python3 spikes/typst-main-editor/default-launch-check.py target/debug/scholium-app target/debug/hammer /tmp/d
```

## 结果

| 检查 | 结果与证据 |
|---|---|
| 回归 | **142 passed / 0 failed / 0 ignored**（旧编辑面 32 项 opt-in 体验用例随之删除）；[tests.log](evidence/SPK-0060/tests.log) |
| gates | fmt、clippy、rustdoc、文件规模、licenses/bans/sources 通过 |
| fork 来源 | 上游对象 + 四补丁逐文件重建一致；[fork-source.json](evidence/SPK-0060/fork-source.json) |
| fork/stock | full/styles/heading 三组逐像素一致；[stock-comparison.json](evidence/SPK-0060/stock-comparison.json) |
| 实机 | 普通启动 33 步全通过，111 audit frames、47 pending，committed egui Text draw **0**；[summary.json](evidence/SPK-0060/native/summary.json) |
| 迁移 | `hammer` 旧会话经普通启动迁移，旧文件 sha256 不变；[summary.json](evidence/SPK-0060/default-launch/summary.json) |
| advisories | **Fail**，仍是与 main 相同的 7 项既有通告；[advisories.log](evidence/SPK-0060/advisories.log) |

## 未覆盖

正文无障碍 Text 接口与数学播报、分页与纸张、生成源码工作区与快速预览、未完成公式原位诊断、
阶段 0 远端动作注入须在新编辑面上重新实现与验收。`scholium-typst` 中旧模型的预览编译栈暂留
（ADR 0037 第 4 条）。依赖旧界面的历史 spike 复现脚本不再可运行，其报告只作追溯。阶段判定不升级。
