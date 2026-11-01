# 正文段落边界删除

2026-11-01，T27 修复了页面编辑中第二段段首 Backspace、前段段尾 Delete 无法删除段落边界的问题。
实现只修改会话输入层，没有修改 T28 的 IME/绘制、T29 的空段渲染与 Typst fork 所有权文件。

## 实现与边界

- 新增 `typst/block_merge.rs`，只识别当前块首/尾正文叶的真实边界。首块段首 Backspace 与末块段尾
  Delete 直接返回空操作，不产生请求或历史。
- 正常端点用空文字 `ReplaceBodyRange` 合并；前块保留身份与类型，保留两个端点叶及公式/样式结构。
  Backspace 光标落在前段旧末尾，Delete 保持原位置及 affinity。每次合并独立一步撤销，undo/redo 恢复原块与光标。
- 正常插入公式会创建公式后的空正文叶，故公式后拼接仍使用范围编辑。另验证了恢复文档中公式直接处于块尾/块首、
  没有正文边界叶的情形：复用已有 `StructuralEdit::MergeWithNext`，没有新增结构编辑类型；
  拼接点分别以后一块正文叶 byte 0 / 前一块正文叶旧末尾表示，完整保留公式身份。
- 数学空槽 `RemoveSlot` 仍先执行。同块公式后正文叶 byte 0 的 Backspace 保持原行为（本任务明确不处理）。

## 先失败、后修复

先添加八个会话回归测试，并运行 `cargo test --locked -p scholium-session block_merge -- --nocapture`：
首次运行 5 失败、3 通过，其中 4 项复现合并没有改变文档，1 项为公式开头 fixture 漏传请求日志，
随后修正为传入原 journal；文档边界、同块公式后正文叶与数学槽位保护通过。
证据复核时临时移除边界分发、保留最终 fixture，负对照 5 项均在 `outcome.changed` 失败；恢复分发后 8 项全部通过。
[失败与负对照摘要](evidence/2026-11-01-backspace-block-merge/tests-first.txt)。
随后添加共享 JSON 场景，运行 `cargo test --locked -p scholium-session every_shared_scenario`，
在第 5 步 Backspace 失败：光标仍为第二块 byte 0，而非第一块旧末尾 byte 4。

修复后会话层 20 通过、1 个既有计时测试忽略。八个新增测试覆盖：双向中英文段落合并、空中段、
标题/正文两种顺序、文档边界空操作、公式边界（有/无空正文叶）、公式开头后块、同块公式后正文叶保护、数学空槽优先。
需要合并的用例均验证单步 undo/redo、块身份、内容与光标。

共享 `scenarios/backspace-block-merge.json` 固定了完整语义树、光标、anchor 和历史深度；
headless `scenario_tests.rs` 与窗口 `parity-check.py` 重放同一输入与断言。
新增自动发现的 `backspace-block-merge-check.py` 复用窗口重放器，通过 AUDIT 验证输入两段、Home、BackSpace 后
只剩一个块且文本为 `leftright`，随后验证 undo/redo 与 Delete。没有修改现有场景断言或阈值。

## 验证

以下工作区门禁全部通过：

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps
python3 scripts/check-rust-size.py
python3 scripts/check-doc-links.py
python3 scripts/check-ui-strings.py
python3 scripts/check-layers.py
cargo deny --locked check licenses bans sources
cargo build --locked -p scholium-app -p scholium-storage --bins
```

工作区共 459 个测试通过、4 个既有显式计时/外部字体测试忽略。cargo deny 的 licenses/bans/sources 通过，
仅有既有未匹配许可允许项提示。

另运行工作流的独立 stock Typst 对照：

```sh
cargo run --locked -p scholium-typst --features editor --example typst-main-fork-check -- /tmp/t27-fork-pixels
cargo run --locked --manifest-path spikes/typst-main-editor/reference/Cargo.toml --target-dir target/stock-reference -- /tmp/t27-stock-pixels
python3 spikes/typst-main-editor/compare.py /tmp/t27-fork-pixels /tmp/t27-stock-pixels
```

full/scripts/styles/heading 四例全部逐像素相等（[对照结果](evidence/2026-11-01-backspace-block-merge/stock-comparison.json)）。
本次没有改变 Content、样式或生成源码，因此独立 reference 保持原样。

真实窗口执行：

```sh
python3 scripts/window-checks.py --locale zh-Hans en --out target/t27-window
```

使用 runner 的机器级锁和私有 Xvfb，没有访问用户桌面。两种语言各 17 项，合计 34/34 通过，
含新检查与共享 parity 全场景。完整未跟踪日志留在 `target/t27-window/`；提交了
[窗口汇总](evidence/2026-11-01-backspace-block-merge/window-summary.txt)和
[双语言 AUDIT 语义结果](evidence/2026-11-01-backspace-block-merge/boundary-window.json)。
本次未改既有测试断言/阈值，没有功能偏离或剩余任务；同块公式后 byte 0 的限制按任务要求保留。
