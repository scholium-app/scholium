# T39：项目文件命令与正式存储接入

基于 `4bb9808`，工作分支 `ation_ciger/feat/project-files`，仅在 wt1 实施。
本批把项目声明、固定语言、正式项目锁与持久化接入文件命令；没有引入 Loro，也没有修改 wt3 的
`typst_editor/source_view`、`source_editor` 或 `latex_workspace`。

## 实施记录

- 新增 ProjectHandle，版本化 TOML 声明、固定语言/authority、独立项目/资源身份、明确入口与普通项目导入。
- 原生 Typst 复用页面 EditorCore，成功命令的每次结构提交追加临时版本化状态 Action；失败/panic 不落中间状态。
  保存写 structured 快照；强杀恢复最后快照之后的 Action。本地撤销和丢弃也追加，未改写共享历史。
- `.typ` 的最小源码输入与 `.tex` 占位共享 SourceProject，私人 draft 独立保存，保存走 hash/materialize。
  外部修改不会被覆盖，界面提供重新载入/另存为；草稿写入失败保留内存输入并可见报错。
  回归修复了“物化已完成但草稿尚未清理”崩溃后的假外部冲突。
- 实现新建语言选择、路径/入口选择、打开、最近项目、保存、另存为、文件关闭与保存/丢弃/取消确认。
  另存为复制普通资源和当前内容，生成新身份，排除私人元数据；已有目标与原项目内部目标拒绝。
  路径选择使用 egui 原生文本对话框，不是系统文件浏览器；`file.export` 保持禁用。
- 默认启动改成 `SCHOLIUM_PROJECT_PATH` 或数据目录 default-project。旧会话只读一致迁移到独立项目，
  校验保存/重开结果，写兼容报告，保留旧库；中断迁移目标拒绝作为空白项目打开。
  `SCHOLIUM_SESSION_FILE` 已退役，窗口检查统一改成项目目录，最近项目配置也隔离到检查输出。
- 新增 ADR 0051，更新 storage/session/app 模块的现行规范；新增中英 catalog 键，删除旧启动路径的 allowlist 条目。

## 测试与门禁

测试先行记录保存在本槽位 `target/t39-*-red.log` 和各次窗口日志中。实现前项目/会话 API 测试编译失败；
资源另存为测试曾因未复制附件失败；草稿清理崩溃用例曾报假 Conflict；注入 Commit 故障的输入保留测试曾返回旧文本。
修复后均通过。首次新增窗口驱动在窗口尚未映射时设置焦点收到 BadMatch，改为只选择可见窗口；
chrome 新建检查曾在新文档的首个布局到达前读取 raster，改为等待 current 和真实 raster，未放宽排版阈值。

最终 Rust 门禁使用同一份应用二进制，SHA-256 为
`b53d031ad2f54f024def9b20dfd07f81819284cdb083a1e0fbdb4c6fe8c19261`。
原始输出保留在本槽位 target，不把测试项目数据库加入 Git；可移交证据见
[门禁摘要](evidence/project-files/gates.json)与[完整命令](evidence/project-files/gate-commands.txt)。

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 通过 |
| `RUST_TEST_THREADS=1 cargo test --locked --workspace` | 609 通过、0 失败、11 忽略，28 个测试套件 |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | 通过 |
| `python3 scripts/check-rust-size.py` | 通过 |
| `python3 scripts/check-doc-links.py` | 通过 |
| `python3 scripts/check-ui-strings.py` | 通过 |
| `python3 scripts/check-layers.py` | 通过 |
| `cargo deny --locked check licenses bans sources` | 通过；保留已有未命中许可白名单警告 |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | 通过 |

首次默认并行 workspace 测试在未修改的 LaTeX 子进程监督计时用例失败；没有调整其阈值或实现，
最终单线程运行通过。较早的窗口全量运行被 SIGTERM 中断，不作为验收通过证据。

按 typst-fork 工作流分别执行 fork example、独立 stock reference 和像素比较；系统字体及仅内置字体
各 31/31 像素完全一致，结果见[系统字体](evidence/project-files/stock-system.json)与
[内置字体](evidence/project-files/stock-bundled.json)。本批未修改 fork/adapter 或独立 reference 夹具。

## 窗口证据

`python3 scripts/window-checks.py --locale zh-Hans en --out target/t39-window-final` 完成全部 52 项，
首轮 51 项通过、英文 native 失败：Xvfb 进程被终止，应用收到 XOpenDisplayFailed。
同入口定向复验也遇到 Xvfb 终止；随后带 strace 的定向诊断运行通过 native 全部 33 阶段（1/1），
最终 52 个不同检查项均取得通过结果。三次运行没有更换应用二进制或调整断言；Xvfb 终止来源未确定，
内核日志未发现 OOM 记录。未将首轮描述为一次全量无失败，完整输出与精确命令见
[窗口汇总](evidence/project-files/window-summary.txt)。所有运行均通过 runner 的机器锁，在私人 Xvfb 内完成。

新增三个真实窗口流程均覆盖中英界面：原生项目新建—编辑—保存—强杀—重开；源码外部修改后的保存冲突与
原文件保留；LaTeX 打开与占位工作区。截图分别为：

- [原生内容重开](evidence/project-files/native-reopened.png)
- [源码冲突提示](evidence/project-files/source-conflict.png)
- [LaTeX 占位](evidence/project-files/latex-placeholder.png)

## 文件范围与限制

完整变更路径见[文件清单](evidence/project-files/changed-files.txt)。除任务独占模块，必要的共享改动包括
app 状态/命令状态/状态栏语言装配及其测试、i18n 追加、ADR 索引追加、旧 UI 字符串白名单缩减、
窗口脚本的项目路径变量迁移和最近项目配置隔离。Cargo.lock 由 cargo 更新，仅新增 storage 的已有 TOML 依赖引用。
未修改 wt3 的共享源码编辑器或 LaTeX 工作区目录，也未引入 Loro。

本批路径选择器是 egui 文本输入，`.typ` 源码仅有最小输入与存储冲突操作，LaTeX 仅占位；完整源码工作台和
LaTeX 预览由 wt3 后续接入窄接口。原生 Action 仍是版本化全状态临时编码，将由共享 SDG adapter 的 Action
格式替换；没有宣称 CRDT、协作撤销或产品版本史已经实现。全状态 journal 的体积与写入成本尚未优化。
PDF 导出、watcher、跨文件 write group 均不在本批范围。
