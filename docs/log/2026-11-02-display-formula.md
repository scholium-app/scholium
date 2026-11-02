# Display formula implementation

Implemented the additive Equation block kind for Typst display formulas. Equation blocks validate as one Math inline, serialize through the existing version-1 structured envelope, lay out as block equations, and generate standalone spaced-dollar source with block ranges. Insertion splits body text or extracts a same-row math selection; Enter, Backspace, undo and empty-equation deletion use block semantics. Inline/display conversion remains deferred.

Tests were added before implementation for the Equation model/validation and codec, insertion and
selection behavior, block-boundary editing, source ranges, Content geometry, and display-aware session
parity. The initial model test failed because Equation did not exist; the initial placeholder/Enter
regression failed because the temporary Attach slot was still unfilled; and the selection-transfer test
initially failed because display roots rejected selected math. Each passed after its corresponding fix.

`cargo fmt --all -- --check`, workspace clippy with `-D warnings`, rustdoc with `-D warnings`,
workspace binary build, Rust-size, doc-link, UI-string, layer, and `cargo deny` checks passed. Both
`cargo test --locked --workspace` and `SCHOLIUM_TEST_FONTS=bundled cargo test --locked --workspace`
reported 517 passed, 0 failed, and 4 ignored. Cargo deny reported the existing unused
`Unicode-DFS-2016` allowance warning; bans, licenses, and sources passed.

The independent stock Typst reference matched all 19 fork fixtures pixel-for-pixel under system fonts
and all 19 under bundled fonts, including display equations alone, at document start/end, and between
paragraphs. No fork/vendor patch was needed and comparison thresholds were unchanged. The full private
Xvfb window suite passed 42/42 checks across `zh-Hans` and `en`, including display insertion, shortcut,
menu, Enter/Backspace, and shared parity replay. That full run used the binary built before the final
same-row selection guard adjustment; after rebuilding the final source, the focused display-formula
window workflow passed again in both locales (2/2). Screenshot and run evidence is in the ignored
`target/t33-window/` and `target/t33-window-current-focused/` outputs.

The shortcut is Ctrl+Shift+E because Ctrl+Shift+M already switches page/source mode. Inline/display
focus-toolbar conversion remains deferred, as permitted by the task spec; the block-boundary policy for
surrounding text needs a product decision before conversion can be made unambiguous.


## 合并根式与定界符

协调者要求合并 PR #88 后，本分支合并了 main 的 `3dc2f63`，保留 ADR 0046 与 ADR 0047
的模型、存储、源码、Content、命令、双语无障碍和 fixture 覆盖。新审计 fixture
`equation-root-delimiter` 包含带指数根式、定界符和分数；独立 stock fixture
`display-root-delimiter` 在两段正文之间覆盖同一组合。

先用失败测试确认 Equation 标签仍为英文，再将其改为 `独立公式`。由于协调者明确要求模型标签
与现有中文块标签一致，同步在 UI 字串 allowlist 中登记这一例外，没有修改扫描器。
合并后的完整删除审计复现了空 Root/Delimited 公式删除的 WrongTarget，嵌套选区提取测试也先失败；
补齐 Equation 模块的后代查找、落点和空树遍历后均通过。结构审计 fixture 构造拆到子模块，
以满足文件大小门禁；既有审计断言和像素阈值保持不变。

本次合并后完整门禁的 fmt、clippy（workspace/all-targets/-D warnings）、两种字体的 workspace tests
（各 535 passed、0 failed、4 ignored）、rustdoc（-D warnings）、bins build、Rust size、doc links、
UI strings、layers 与 cargo deny 均通过。cargo deny 只报告既有 Unicode-DFS-2016 未使用 allowance。
`verify-typst-fork.py` 重建 438 个文件、校验全部五份补丁成功；独立 stock 比较在两种字体下均为
31/31 fixture 尺寸及 RGBA 字节一致。压缩后的可复核记录保存在
[evidence](evidence/2026-11-02-display-formula/fork-source-summary.json)，stock 明细见
[system](evidence/2026-11-02-display-formula/stock-system.json) 与
[bundled](evidence/2026-11-02-display-formula/stock-bundled.json)。

最终源码构建后的完整私有 Xvfb 套件双语共 44/44 通过，包含 display-formula、
math-root-delimiter、全部共享 parity replay、sharp-raster 和 a11y。见
[窗口摘要](evidence/2026-11-02-display-formula/window-summary.txt)。完整运行证据位于
`target/t33-integration/`；相对 main 的改动见[文件清单](evidence/2026-11-02-display-formula/changed-files.txt)。
