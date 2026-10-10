# SPK-0059：普通启动切换为 Typst Content 编辑器

- 日期：2026-10-10；结论：**Pass（默认启动切换；旧编辑器删除另批）**。
- 对应：[主程序接入计划 E](../plan/TYPST_MAIN_APP_INTEGRATION.md)、
  [ADR 0036](../adr/ADR-0036-typst-default-editor.md)、[阶段 0 Typst 映射](../plan/ROADMAP.md)。
- 基线：分支 `ation_ciger/feat/typst-vertical-navigation`（报告 0058）之上；分支
  `ation_ciger/feat/typst-default-editor`。源码与二进制指纹见
  [source-hashes.json](evidence/SPK-0059/source-hashes.json)。

## 问题

普通启动（`cargo run -p scholium-app`）走旧页面编辑器：击键先由 egui 文字回显绘制，全文
Typst 编译完成后再换成页面像素，已提交正文出现两套绘制。无回显的 Typst Content 路径
只在 `dev/typst-editor` 加 `--typst-editor` 时可用。阻止默认切换的依赖通告在 main 的
stock 构建中同样存在：同为 7 项（RUSTSEC-2024-0320、2024-0436、2025-0141、2026-0192、
2026-0194、2026-0195、2026-0206），见
[main-stock-advisories.log](evidence/SPK-0059/main-stock-advisories.log)。

## 实现

根 workspace 以 `[patch.crates-io]` 统一使用跟踪的 `vendor/typst-edit` 13 个库；锁文件只
删去这些库的 registry 来源/校验和并加入 app 直接依赖，版本不变。`scholium-app` 默认启用
`typst-editor`，普通启动进入 Typst Content 会话；旧编辑器只经显式 `--legacy`，默认失败
报错不回退。依赖旧界面的 ribbon/体验/冒烟脚本改为显式 `--legacy`。

未设 `SCHOLIUM_SESSION_FILE` 时文档为 XDG `scholium/document.v1.sqlite`。目标不存在且
同目录旧版会话有已保存 head 时，经 `migrate_legacy_file` 一次性迁移（旧文件只读打开），
页头提示迁移与原串公式数；已有 v1 不被覆盖，空旧库不迁移，启动不创建旧库。显式环境变量
路径仍只接受 v1。界面去掉“开发候选”字样。

代码评审后补充：旧文件改由存储层 `legacy_has_head` **只读**探测（原 `SessionStore::open` 会
写 WAL/补表）；迁移或打开失败时窗口显示原因、文件路径与恢复方式（`--legacy` 或移走旧文件），
不运行编辑器（原为 stderr 后退出，GUI 用户看不到）；并发首启时若目标已被另一实例发布则以其
为准；`--no-default-features` 构建去掉未使用变量告警。迁移只发生一次，此后 `--legacy` 的
修改不会同步进默认文档，已写入 ADR 与 README。

## 环境与复现

Linux x86_64 / X11/Xvfb 1280×900，Rust/Cargo 1.96.0，eframe 0.36.2，Typst 0.15.1 fork。

```sh
cargo +1.96.0 fmt --all -- --check
cargo +1.96.0 clippy --locked --workspace --all-targets -- -D warnings
cargo +1.96.0 test --locked --workspace
RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace --no-deps
python3 scripts/check-rust-size.py
cargo +1.96.0 build --locked -p scholium-app
cargo deny --locked --config deny.toml check licenses bans sources
cargo deny --locked --config deny.toml check advisories
xvfb-run -a -s '-screen 0 1280x900x24' \
  python3 spikes/typst-main-editor/native-check.py target/debug/scholium-app /tmp/scholium-default-native
xvfb-run -a -s '-screen 0 1280x900x24' \
  python3 spikes/typst-main-editor/default-launch-check.py target/debug/scholium-app /tmp/scholium-default-launch
```

开发 manifest 的 clippy/test/build 同时重跑，见证据目录 `dev-*.log`。

## 结果

| 检查 | 结果与证据 |
|---|---|
| 根回归 | **210 passed / 0 failed / 32 ignored**，编辑器测试现属默认构建，新增 6 项启动迁移与 1 项只读探测用例；[root-tests.log](evidence/SPK-0059/root-tests.log) |
| 开发 manifest | **132 passed / 0 failed / 32 ignored**；[dev-tests.log](evidence/SPK-0059/dev-tests.log) |
| gates | 根/开发 fmt、clippy（含根 `--no-default-features`）、build、rustdoc、文件规模；根 licenses/bans/sources 通过（fork 为路径依赖） |
| 普通启动实机 | 不带参数启动根二进制，原 33 步流程全通过，112 audit frames、48 pending，committed egui Text draw **0**；[summary.json](evidence/SPK-0059/native/summary.json) |
| 坏旧库 | 随机字节冒充旧会话：窗口显示“无法打开默认文档”、路径与恢复方式，不启动编辑器，旧文件 sha256 不变、未建新库；[截图](evidence/SPK-0059/startup-error/corrupt-legacy.png)、[日志](evidence/SPK-0059/startup-error/corrupt-legacy.log) |
| 默认路径迁移 | 无环境变量：`--legacy` 新建并保存正文后，普通启动创建 `document.v1.sqlite`，旧文件 sha256 不变，Typst 呈现迁移正文且 egui 文字绘制 0；[summary.json](evidence/SPK-0059/default-launch/summary.json)、[截图](evidence/SPK-0059/default-launch/default.png) |
| advisories | 根构建 **Fail**，与 main stock 构建编号完全相同的 7 项；[root-advisories.log](evidence/SPK-0059/root-advisories.log) |

启动迁移用例覆盖：无旧文件不建库、空旧库不迁移且字节不变、有 head 迁移一次且旧文件字节
不变、已有 v1 不被覆盖、非旧版 schema 报错并点名文件、错误页显示原因与 `--legacy`；存储层
只读探测覆盖 0 字节、空库、有 head 与 v1 文件。

## 未覆盖与下一步

旧路径独有的分页与纸张、生成源码工作区与最终预览、正文无障碍文本和示例文档暂不在默认
路径；迁移后的旧公式为 RawMath，新编辑器报布局诊断、转换前不可编辑；冷启动 debug 构建
首帧需等字体扫描。依赖通告未处理。下一批删除旧编辑器与回显代码、合并开发 manifest，
并按需移植上述能力；阶段判定不升级。
