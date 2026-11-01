# IME 候选窗与行内预编辑

## Part A：候选窗锚点

修正了当前 caret 和 pending Scene fallback 两处 IME 输出。egui-winit 0.36.2
以 `IMEOutput.rect` 调用 `set_ime_cursor_area`；两矩形均发布 caret，并限制在可见页面。
保持已有 pending 时不断开输入法的策略。AUDIT 增加两矩形，真实窗口检查比较其一致性。

先运行 `cargo test --locked -p scholium-app ime_area_tracks -- --nocapture`，回归测试失败：
实际 rect 为整页 `[80,100,680,800]`，cursor 为 `[250.5,172,250.5,180.3]`。
修复后同一测试通过（1/1）。Part A 先独立提交，Part B 与全量门禁随后执行。

## Part B：独立派生输入与 Typst 行内排版

新增了独立 `Preedit`、worker 提交接口与 Scene 显示 caret。临时串只在 Content 构建时插入；
正文前串、下划线组合串、后串均由 Typst 排版。文档 snapshot/history/source/store 和 AccessKit
文字始终只含已接受正文。拆分后的后串 byte 从零开始，因此组合 Scene 不安装为文档输入 geometry；
指针、导航、选区、caret-follow 和辅助技术位置暂时停用，正常 Scene 到达后恢复。
每次组合/取消使用新 request stamp，旧结果不能覆盖取消或提交；等待期间保留上次 Typst 栅格。
数学临时组合显示保留为范围外，数学 Commit 仍经原有字面提交路径。

Content cache 只失效旧/新目标叶及其段落，未变段落继续复用。未修改 T27 的 session input/key，
未修改 T29 的 editor/style、structured_source、vendor 或 `block()`。`content.rs` 仅接入独立的 `content/preedit.rs` hook；临时串注入、缓存失效、inline/text 构建与空段落 preedit 包装均在该子模块：空段落临时串使用正常段首缩进，避免提交跳动。

### 先失败、再修复的证据

- Typst run/reflow 测试先失败于找不到预编辑 run，修复后通过；测量实际 Frame 中插入位置与后串位移。
- session 测试先失败于组合 geometry 仍可回答文档查询，修复后通过；同时验证 snapshot/source/history/保存重开。
- app 测试先失败于组合期间仍有 egui Text shape，移除叠层后通过；另覆盖 IME scalar 索引、旧图保留和数学提交。
- 空段落测试先测得组合末尾 x=24 pt、提交末尾 x=48 pt，独立空段落 hook 修复后位置完全相等。
- 另覆盖 AccessKit 只读权威文字、组合时无位置几何及取消后恢复，以及旧 composition Scene 被取消 stamp 拒绝。

### 本地非窗口门禁

`cargo fmt --all -- --check`、`cargo clippy --locked --workspace --all-targets -- -D warnings`、
`cargo test --locked --workspace`（460 passed，4 ignored）、
`RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps`、
`python3 scripts/check-rust-size.py`、`python3 scripts/check-doc-links.py`、
`python3 scripts/check-ui-strings.py`、`python3 scripts/check-layers.py`、
`cargo deny --locked check licenses bans sources` 和
`cargo build --locked -p scholium-app -p scholium-storage --bins` 均通过。
日志与逐命令状态在 `target/t28-gates/`（不入 Git）。

按 typst-fork workflow 运行了 formal adapter、独立 stock 参照及 compare；保留原有四项严格断言，
增加 preedit 的独立 stock `UnderlineElem` 构造。五个场景尺寸与所有 RGBA byte 完全一致，
并验证相同 preedit 的二次提交不重建 Content（built=0）。另以 `scripts/verify-typst-fork.py`
验证了锁定上游 `9dfd3a08500b7896045f907433cf7b4b02434fad` 的补丁树，vendor 未改。

### 窗口验证范围

main checkout 中现有 `typst-main-editor` 窗口检查没有 Preedit/Commit 事件入口；因此除了扩展
native 检查的 IME 矩形断言，还新增自动发现的 `ime-inline-check.py` 与同时要求 AUDIT 的进程内事件 seam。
该 seam 原子读取测试事件后调用普通 egui 路径，不依赖宿主安装的输入法，不操作用户桌面。
验证 X11 窗口中的 inline reflow、后串不重叠、caret/候选窗区域、内部 caret、取消、一次提交和撤销；
不声称验证了 fcitx5 在 niri/Wayland 上的真实协议交付。所有窗口检查经标准脚本和机器级锁串行运行。


### 窗口最终结果与证据

`python3 scripts/window-checks.py --locale zh-Hans en --out target/t28-window` 完成了 34/34 检查，
两种语言各 17 项（含 native、parity、sharp-raster、AccessKit/AT-SPI 与新增 IME 检查），全部通过。
新检查实测组合宽 60.022583 px、后串位移 60.022614 px；取消后原位置恢复，提交一次且不跳位。
已查看截图，组合串继承文档字体并带下划线，后串无叠盖。只使用私有 Xvfb，未操作用户桌面。

提交的证据包含：[完整门禁摘要](evidence/2026-11-01-ime-inline-preedit/gate-summary.json)、
[双语言窗口结果](evidence/2026-11-01-ime-inline-preedit/window-summary.txt)、
[IME 几何摘要](evidence/2026-11-01-ime-inline-preedit/ime-window-summary.json)、
[窗口截图](evidence/2026-11-01-ime-inline-preedit/inline-preedit.png)、
[stock 逐像素对照](evidence/2026-11-01-ime-inline-preedit/stock-comparison.json) 和
[变更文件清单](evidence/2026-11-01-ime-inline-preedit/changed-files.txt)。
