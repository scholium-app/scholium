# 根式与自动定界符（T32）

2026-11-02，在 `scholium-wt1` 的 `ation_ciger/feat/math-root-delimiter` 分支实现了数学根式与自动定界符。
基线为 `8c58a8fe98241ea9d32c121fcf4e336538d96a7e`；本次只修改 MathBody 层，没有修改 T33 的 BlockKind 或存储 codec。

## 完成的行为

- 增加 `Root { index?, radicand }`、`Delimited { open, close, body }`，验证严格限定八种配对；v1 存储 round-trip
  保留身份及空槽。ADR 0046 记录了加性 tag 与旧构建显式拒绝的兼容策略。
- 菜单和数学焦点栏接入根式/定界符：在 caret 拆分当前行，选区仅进入新内容槽，支持结构边缘与现有槽位。
  焦点栏支持添加/进入根指数与循环八种配对；字面 `(` 保持文字。撤销恢复文档、caret 与选区。
- 采用 Ctrl+Shift+R 插入根式：R 对应 root，注册表没有重复和弦，精确修饰键测试覆盖 Ctrl+R 不误触发。
  [P1 UI](../plan/P1_UI.md) 中既定的 Ctrl+0/1/2、Ctrl+Shift+M 及现有命令均保持绑定。
- 扩展 ADR 0045 的边缘/槽遍历、结构删除与 Scene 几何。根指数先于被开方数，定界符只有 body；
  额外修复了从复合结构边缘 Tab/Shift+Tab 跳错叶的问题。
- 编辑面直接使用 RootElem/LrElem，index 槽 1、radicand 槽 2、delimiter body 槽 1。
  补丁 0005 补全了 lr 展平后的 body 槽 origin；没有放宽像素对照阈值。
- 生成 `sqrt`、`root`、`lr` 源码，并保留字面引号/转义、逗号参数分组和 `□` 空槽。
  根式/括号和嵌套数学描述、槽名、空槽与边缘均按界面语言播报。

## 测试与修复记录

新增模型/codec/验证、源码实际编译、Content 槽 origin/caret/命中、session 插入/选区/撤销、双语 AccessKit、
快捷键和真实窗口用例，并把根式、带指数根式、定界符、括号内分数、Attach 内根式及相邻根式/括号纳入 T31 表驱动审计。
另加入全空根式/带指数根式/定界符，验证删除只移除全空结构。

接续已有工作后，新增失败测试实际复现并修复了四个问题：已有多节点 index 再次进入时落到 Row，
英文空槽及嵌套数学播报混入中文，结构边缘 Tab 使用文档开头作为错误默认位置，以及父槽覆盖嵌套定界符的 body 标记。
四个测试均先失败后通过；嵌套定界符以不打印的 Sequence 包装保留父槽与自身槽的独立 origin。
中文既有朗读断言保持原值，相关测试显式固定了中文 locale；失效的硬编码 allowlist 条目被移除。

新增窗口脚本的首轮执行在启动应用前因输出目录已存在失败，修正为独立场景子目录后双语定向检查通过。
中断/失败运行保留在 `target/t32-gates/window*`，没有作为最终通过证据。
完整窗口首轮为 41/42；英文 AT-SPI 脚本仍等待中文分母状态，和本次按 locale 输出的数学朗读不符。
脚本保留了完整中文断言，并加入完整英文公式/槽状态断言；双语定向复验 2/2 通过后重新运行完整窗口门禁。

## 门禁

以下门禁均通过，系统/打包字体的工作区测试各为 518 passed、4 ignored、0 failed：

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
SCHOLIUM_TEST_FONTS=bundled cargo test --locked --workspace
RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps
cargo deny --locked check licenses bans sources
cargo build --locked -p scholium-app -p scholium-storage --bins
python3 scripts/check-rust-size.py
python3 scripts/check-doc-links.py
python3 scripts/check-ui-strings.py
python3 scripts/check-layers.py
git diff --check
```

最终完整窗口命令 `python3 scripts/window-checks.py --locale zh-Hans en --out target/t32-gates/window-rechecked`
通过 42/42（21 个检查 × 两种语言），包括新增根式/定界符场景、共享 parity 和两种语言的 AT-SPI。
全部窗口在机器锁内使用私有 Xvfb；没有使用桌面 display。
[窗口汇总](evidence/2026-11-02-math-root-delimiter/window-summary.txt)、
[中文截图](evidence/2026-11-02-math-root-delimiter/root-index-brackets-zh-Hans.png)、
[英文截图](evidence/2026-11-02-math-root-delimiter/root-index-brackets-en.png)均来自最终完整运行。
截图检查确认焦点栏完整，带指数根式及嵌套方括号显示正常，caret、底色与槽轮廓可见。
[变更文件清单](evidence/2026-11-02-math-root-delimiter/changed-files.txt)记录了本任务完整范围。
独立 stock 对照在两个字体 profile 下各 26 个夹具全部逐字节相等，包含平方根、带指数根式、根式内定界符及八种高括号。
`verify-typst-fork.py` 从上游 `9dfd3a08500b7896045f907433cf7b4b02434fad` 和五个补丁重建，
完整清点了 438 个 vendored 文件；源校验及像素摘要见本日志的 evidence 目录。

源重建及 stock 对照命令如下；第二组像素命令再次以 `SCHOLIUM_TEST_FONTS=bundled` 执行，输出目录换为
`fork-bundled` / `stock-bundled`：

```sh
python3 scripts/verify-typst-fork.py target/t32-gates/typst-origin
cargo run --locked -p scholium-typst --features editor --example typst-main-fork-check -- target/t32-gates/fork-system
cargo run --locked --manifest-path spikes/typst-main-editor/reference/Cargo.toml --target-dir target/stock-reference -- target/t32-gates/stock-system
python3 spikes/typst-main-editor/compare.py target/t32-gates/fork-system target/t32-gates/stock-system
```

## 边界

没有任务功能延期；根指数 UI 也接入了。跨槽选区、字面括号自动配对、通用源码解析仍按任务边界不提供。
schema v1 未升版本，旧构建不能打开含新 tag 的文档。没有修改 Git 配置，没有合并 PR，也没有轮询 CI。
