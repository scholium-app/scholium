# 数学定界符键入与隐形闭侧（T55）

2026-11-10，在分配的 scholium-wt3 槽位，从 origin/main `ccc4a4e9` 完成。
[MATH_INPUT](../plan/MATH_INPUT.md) 批次 1 的“括号自动配对、‖x‖、⌊x⌋、⌈x⌉，退格使右括号隐形”在本次实现并验收；
计划表未改写。规范见 [ADR 0060](../adr/ADR-0060-math-delimiter-input.md) 与各模块文档。

## 改动

- 共享 engine 的临时窗口与 data 规则把刚创建的空绝对值转换为范数、上下取整；没有 app 特定字符分支。
  一次 Undo 恢复原绝对值和 body 光标，后续文字单独撤销。导航、其它输入、粘贴、IME 都终止窗口。
- Backspace 在 Delimited 外侧右边缘把 close 改为隐形并进入 body 末尾；槽尾键入任一支持闭符恢复该侧并到外侧右边缘。
  两者各自独立撤销/重做；隐形结构的空删除、非空导航和循环恢复匹配配对均保留。
  紧邻右侧文字时复用其 byte 0 作为 ADR 0045 的共享边缘，恢复闭侧后箭头导航和再次 Backspace 均有效。
- open/close 字符字段与 v1 envelope 未改形状；close 独立接受八种闭符或 U+0000，open 仍限原八种。
  model、结构存储、SDG shape/Action/snapshot 通过相同验证保存并重放；非法 close 被拒绝且既有 head 不变。
- 编辑面的 Lr 两侧改为真正 SymbolElem。像素验证发现旧 TextElem 没有可伸缩定界符类，因此同时修正独立 stock Content 参照。
  生成源码使用固定白名单符号名表达独立两侧，如 `lr(bracket.l 0 , 1 paren.r)`；用户 body 仍逐原子转义。
- 两语言无障碍朗读明确命名两侧与隐形侧。新增仅在显式审计环境开启的完整 sharp raster 导出，直接比较窗口实际上传像素。

## 验证

先加入行为测试并观察失败：会话四例中三例失败，model 独立两侧测试失败。
复查时另以失败测试复现了紧邻右侧文字的共享边缘问题，再修正光标停点。
修复后工作区测试通过 808 项，21 项既有手工/计时测试保持 ignored，包含存储非法 close 原子拒绝与共享边缘回归测试。
新增覆盖转换窗口、Undo/Redo、八种闭符恢复、导航/空删除、8×9 种合法模型值、存储往返和 SDG 重放。
源码测试实际编译每种开侧的隐形形式、各闭侧与含 `#panic` 的字面正文；验证中修正了角括号的 Typst 符号名为 chevron。

以下门禁均通过：

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --locked --workspace` | PASS（808 passed / 21 ignored） |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | PASS |
| `python3 scripts/check-rust-size.py` | PASS |
| `python3 scripts/check-doc-links.py` | PASS |
| `python3 scripts/check-ui-strings.py` | PASS |
| `python3 scripts/check-layers.py` | PASS |
| `cargo deny --locked check licenses bans sources` | PASS（既有未使用许可 allowance 警告） |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | PASS |
| `python3 scripts/window-checks.py --locale zh-Hans en`（默认 jobs=3） | PASS（首轮 68 passed / 2 skipped；最终复跑 65 + 恢复 3，详见下文） |

独立 stock v0.15.1 验证使用 typst-fork.yml 的两个字体 profile 命令：

```sh
cargo run --locked -p scholium-typst --features editor --example typst-main-fork-check -- /tmp/t55-fork-pixels
cargo run --locked --manifest-path spikes/typst-main-editor/reference/Cargo.toml --target-dir target/stock-reference -- /tmp/t55-stock-pixels
python3 spikes/typst-main-editor/compare.py /tmp/t55-fork-pixels /tmp/t55-stock-pixels
SCHOLIUM_TEST_FONTS=bundled cargo run --locked -p scholium-typst --features editor --example typst-main-fork-check -- /tmp/t55-fork-bundled-pixels
SCHOLIUM_TEST_FONTS=bundled cargo run --locked --manifest-path spikes/typst-main-editor/reference/Cargo.toml --target-dir target/stock-reference -- /tmp/t55-stock-bundled-pixels
SCHOLIUM_TEST_FONTS=bundled python3 spikes/typst-main-editor/compare.py /tmp/t55-fork-bundled-pixels /tmp/t55-stock-bundled-pixels
```

两组各 37/37 个场景的尺寸与全部 RGBA 字节完全相同，没有放宽阈值。
新增六例覆盖直接键入范数/上下取整、带分数的单侧伸缩、隐形区间和半开区间；新增场景的 stock 侧实际编译独立源码。
`verify-typst-fork.py` 对从指定上游提交获取的对象验证通过，vendor 与补丁未修改。

首轮全量窗口 68 passed / 2 skipped / 0 failed。最终二进制的全量复跑先通过 65 项，
末段 ui-polish-final/en 出现窗口缩小断言，随后该检查及 start-page-dialogs/en、a11y/en 均报 EDQUOT，
总 results.json 也未能写出。清理重复窗口 RGBA 后，使用默认 jobs=3 定向重跑这三项，3/3 通过；
没有改代码或断言。因此每个应运行的最终组合都有通过记录，但不把受配额影响的全量复跑报告为一次全绿。
两次全量均由 runner 按既有规则跳过两语言的 latex-exit，实际 TeX Live 验收由 latex-exit.yml 承担。

定向窗口 math-input/math-root-delimiter 在 zh-Hans/en 共 4/4 通过。新增键入检查核对 AUDIT 全部节点属性和 caret，
在 `||x`、`|.x`、`|'x` 以及 `[0, 1` → Tab → Backspace → `)` 路径上，
把窗口上传的 2 px/pt sharp raster 与独立 stock 的打包字体尺寸及 SHA-256 严格比较。
等待基于输入计数、current Scene、deferred 队列和已发布 raster 元数据；未操作真实桌面。

精简的测试、像素与窗口证据见 [verification.json](evidence/2026-11-10-math-delimiter-input/verification.json)。
本地日志、AUDIT 与 stock RGBA 保留在 `/tmp/t55-*.log`、`/tmp/t55-windows*` 与 `/tmp/t55-*-pixels`。
最终复跑末段遇到 EDQUOT，清理本任务三个窗口证据目录中约 553 MB 的重复 RGBA；未删除日志、摘要、项目、target 或其它任务文件。
未把原始 RGBA 或私有项目库提交到 Git。

## 范围与边界

没有新增 worktree，没有修改共享索引、计划表、vendor、锁文件或工作流。没有偏离任务范围。
旧测试中“拒绝不匹配配对”的断言按新模型合约替换为“拒绝非法两侧”，删除审计 oracle 增加了闭侧隐藏这一合法编辑；
朗读期望也随新增两侧名称更新。所有像素阈值维持零容差。
开侧隐形仍不支持。PR 不合并、不轮询 CI；远端 CI 由协调者验收。

## 文件清单

本次共 50 个文件：

- `crates/scholium-app/i18n/en.ftl`
- `crates/scholium-app/i18n/zh-Hans.ftl`
- `crates/scholium-app/src/i18n/tests.rs`
- `crates/scholium-app/src/source_editor/completion.rs`
- `crates/scholium-app/src/typst_editor/access/root_delimiter_tests.rs`
- `crates/scholium-app/src/typst_editor/access/speech.rs`
- `crates/scholium-app/src/typst_editor/audit.rs`
- `crates/scholium-app/src/typst_editor/scene.rs`
- `crates/scholium-document/src/structured_session.rs`
- `crates/scholium-math-input/src/data.rs`
- `crates/scholium-math-input/src/engine.rs`
- `crates/scholium-math-input/src/lib.rs`
- `crates/scholium-math-input/src/tests.rs`
- `crates/scholium-model/src/structured.rs`
- `crates/scholium-model/src/structured/tests.rs`
- `crates/scholium-model/src/structured/validate.rs`
- `crates/scholium-sdg/src/schema.rs`
- `crates/scholium-sdg/src/tests/replay.rs`
- `crates/scholium-session/src/typst.rs`
- `crates/scholium-session/src/typst/math_input.rs`
- `crates/scholium-session/src/typst/root_delimiter.rs`
- `crates/scholium-session/src/typst/structural_delete.rs`
- `crates/scholium-session/src/typst/structure_audit_tests.rs`
- `crates/scholium-storage/src/structured/tests.rs`
- `crates/scholium-typst/src/editor/content.rs`
- `crates/scholium-typst/src/structured_source.rs`
- `crates/scholium-typst/src/structured_source/root_delimiter_tests.rs`
- `docs/adr/ADR-0046-math-root-delimiter.md`
- `docs/modules/app.md`
- `docs/modules/collab.md`
- `docs/modules/document.md`
- `docs/modules/math-input.md`
- `docs/modules/model.md`
- `docs/modules/session.md`
- `docs/modules/storage.md`
- `docs/modules/typst.md`
- `spikes/typst-main-editor/compare.py`
- `spikes/typst-main-editor/fixture.rs`
- `spikes/typst-main-editor/fork-check.rs`
- `spikes/typst-main-editor/math-input-check.py`
- `spikes/typst-main-editor/parity-check.py`
- `spikes/typst-main-editor/stock-reference.rs`
- `spikes/typst-main-editor/stock_empty.rs`
- `crates/scholium-math-input/src/delimiter_tests.rs`
- `crates/scholium-session/src/typst/delimiter_input.rs`
- `crates/scholium-session/src/typst/delimiter_input_tests.rs`
- `docs/adr/ADR-0060-math-delimiter-input.md`
- `docs/log/2026-11-10-math-delimiter-input.md`
- `docs/log/evidence/2026-11-10-math-delimiter-input/verification.json`
- `spikes/typst-main-editor/reference/delimiter-pixels.json`
