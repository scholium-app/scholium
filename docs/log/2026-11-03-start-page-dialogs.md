# 起始页与对话框精修（T45）

本批在 `ation_ciger/feat/start-page-dialogs`、指定的 wt1 槽执行。前态二进制来自 `6d03a90`。
在环境组接线前已快进合入 PR #102 的主线 `4dfb4dd`，直接使用真实 toolchain API，没有 stub。
协调者批准了 `scholium-latex` 的最小 API 例外；项目声明的显式引擎现已进入派生预览快照。

## 问题

旧空工作区只有提示，新建缺名称、模板与位置浏览，打开混入迁移入口；锁冲突没有项目路径与重试。
项目设置尚无入口，外部修改冲突缺差异视图、修改时间及明确操作主次。

## 改动

- 增加平面起始页：开始、最近、环境；最近记录可移除，支持方向键与 Enter，锁定项目有重试。
- 新建默认 LaTeX；article / ctexart / Typst 空白模板，名称、位置、浏览、目标预览和字段错误。
- 打开使用 `FilePicker` 契约与应用内回退，多入口再选主文件；旧版本导入移到起始页及文件菜单。
- 项目设置与状态栏引擎入口共用命令；主文件、引擎、参考文献偏好通过持锁存储 API 保存。
- 环境组读取后台 `ToolchainDetector` 缓存，提供重新检测，按 API 消息 ID 本地化问题及建议。
- 冲突对话框显示文件和 UTC 修改时间，提供统一差异、取消、重新载入、另存为；迁移与锁定用同一框架。
- 修复起始页方向键与 egui 空间导航重复移动及同帧多次按键漏计；回归单测先失败后通过。
- 主文件切换复用源码打开的崩溃恢复基线，保留已发布但未清理的草稿；相应回归先失败后通过。

存储改动限于 `crates/scholium-storage/src/project/`：`settings.rs`、`handle.rs`、`handle/settings.rs`、
`handle/settings_tests.rs`、`source.rs`、`mod.rs`。项目声明由 v1 显式保存为 v2，加入构建偏好和入口资源身份映射；
语言、authority、项目身份固定，SQLite schema 不变；另存为保留构建偏好并生成新身份。
设置原子同步发布，外部声明变化拒绝覆盖；脏主文件及未完成物化意图阻止入口切换。
格式决策记录在 [ADR 0054](../adr/ADR-0054-project-settings.md)，对应最小存储规范同步在 `docs/modules/storage.md`，没有编辑共享索引或 P1_UI。

### 协调者批准的范围例外

`crates/scholium-latex/src/session.rs` 新增 `SourceSession::with_engine`，
`crates/scholium-latex/src/tests/source.rs` 增加覆盖 magic comment 与默认检测的回归测试，
`docs/modules/latex.md` 记录显式选择优先级。没有改动 scholium-build 或引入依赖。
显式项目引擎覆盖已检测的引擎，不更改源码、草稿、路径或 revision；没有显式选择时保持现有检测。
app 在应用声明设置后重建 worker、关闭旧输出通道并清空派生页面，状态栏与预览路由跟随新引擎。

## 验证

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 通过 |
| `cargo test --locked --workspace` | 通过：692 passed、15 ignored、0 filtered |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | 通过 |
| `python3 scripts/check-rust-size.py` | 通过 |
| `python3 scripts/check-doc-links.py` | 通过 |
| `python3 scripts/check-ui-strings.py` | 通过 |
| `python3 scripts/check-layers.py` | 通过 |
| `cargo deny --locked check licenses bans sources` | 通过；保留已有未使用 license allowance 提示 |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | 通过 |
| `python3 scripts/window-checks.py --only start-page-dialogs --locale zh-Hans en --out target/t45-dialogs-complete` | 2/2 通过 |
| `python3 scripts/window-checks.py --locale zh-Hans en --out target/t45-windows-complete` | 通过：58/58（zh-Hans 29、en 29） |

新增窗口流程覆盖最近项目点击/键盘打开/移除、新建默认语言及已有目录错误、Enter/Esc、设置持久化与引擎状态、
冲突差异/取消/另存为/重新载入、多入口选择、真实跨进程锁提示/重试、迁移原库字节保留、浅深主题及 480 px 对话框边界。
窗口仅在私有 Xvfb 运行，遵守机器级锁。前态中英检查 2/2 通过。
新 API 单测验证显式 LuaLaTeX / XeLaTeX / pdfLaTeX 覆盖注释或默认引擎，同时保留文本、路径、revision 和磁盘字节。
原先失败的应用单测及中英窗口检查均已通过；失败与修复日志保留作追溯。

[证据目录](evidence/2026-11-03-start-page-dialogs/) 保存前态 36 张、后态 72 张截图、门禁日志及来源清单。
前态没有项目设置，截图用 `project-settings-absent.png` 记录该事实；不是重建的假前态。
后态截图已使用应用显式引擎 API 的二进制刷新。系统选择器尚未实现，不计作已验收系统功能。
没有改动 Typst 编辑面、fork、适配器或独立参考，stock 对照不适用；既有 parity 与全量窗口检查仍运行。

## 待办与规格边界

1. 系统 portal 实现按任务留作后续；当前验证实际回退与模拟系统返回值共用校验契约，不声称已验收系统选择器。
2. 参考文献偏好已保存；bibliography 构建流水线不在当前 API 中，本批没有伪造其执行结果。
3. 15 个 opt-in/原生工具链测试保持 ignored；完整 Rust 门禁没有过滤测试，窗口环境按任务使用缺失 rtex 的配置验证编辑与持久化。
4. Linux 本机窗口与持久性已测；macOS/Windows 的系统选择器和原生持久性留给对应平台验收。
5. 按任务只推送分支，由协调者起草 PR；未创建 PR、未轮询 CI。
