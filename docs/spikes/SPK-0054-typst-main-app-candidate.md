# SPK-0054：Typst Content 接入真实主程序的开发候选

- 日期：2026-10-01；结论：**Pass（B 的限定主程序闭环）**。
- 基线：`c28fd33b95d829399dc3664f3a0ee19da3d81386`；分支 `codex/feat/typst-main-editor-candidate`。
  本报告同提交的源码为验证对象，指纹见 [source-hashes.json](evidence/SPK-0054/source-hashes.json)。
- 对应[接入计划 B](../plan/TYPST_MAIN_APP_INTEGRATION.md)、[ADR 0034](../adr/ADR-0034-typst-main-app-candidate.md)，
  回链[阶段证据入口的条件 3](SPK-0012-exit-criteria.md#证据入口)，不修改其阶段判定。
- 环境：Linux x86_64，Rust/Cargo 1.96.0，eframe 0.36.2，Typst 0.15.1；
  [字体与环境指纹](evidence/SPK-0054/environment.json)。
- 终端日志仅规整行尾空白和多余 EOF 空行；JSON、PNG、源码和像素比较不做内容规整。

## 实际交付边界

验证窗口是 **`scholium-app`**，由开发 manifest 编译现有 main.rs；不是
`typst-edit-window` 或另一个 Editor。SessionBridge 互斥选择同一个 LocalSession 类型的
结构候选，Ribbon/视图命令与 SessionStore 继续作为主程序入口。正文权威没有可写副本互拷。

**候选入口的已提交正文/公式只画 Typst 纹理，不画 egui 临时正文。普通根 workspace
启动仍保留原来的 legacy 回显路径，默认替换未完成。** 开发入口要显式选项和隔离 v1
文件；旧库拒绝而不自动迁移。源码按钮暂显示只读结构 JSON，不声称已接入完整源码工作区。

新建、中英文/空格、inline Hole、输入数学文字、创建和嵌套分数、字素删除、Tab、单机
undo/redo、保存重开已接入。整叶文字样式、标题、正文拆段使用结构请求。
分数由一个核心动作创建 wrapper/空分母，填槽另属文字动作。Row/Symbol 可由 adapter
布局，但 UI 尚未提供专门插入命令。RawMath 逐字保留，布局报节点诊断；没有执行原串或
猜其光标。跨叶选区/替换、复制剪切、多行粘贴、旧公式转换、分页、实机输入法和无障碍
继续进入 C–E；多行粘贴明确拒绝并保留可复制草稿。

## 来源与默认依赖隔离

跟踪的 `vendor/typst-edit` 从上游 v0.15.1 提交
`9dfd3a08500b7896045f907433cf7b4b02434fad` 与四份已有补丁重建。
保留 Apache-2.0 LICENSE；额外变更只有上游 workspace 成员/默认成员裁剪到 13 个库。
重建读 Git 对象，不复制被忽略的 checkout 中未记账的工作区改动。
逐文件结果见 [fork-source-manifest.json](evidence/SPK-0054/fork-source-manifest.json)。

开发 Cargo.patch 统一指向这 13 个同版本库。根 workspace 与独立 stock reference 的
对应库均为 registry，未混用 fork/stock 类型；见
[dependency-origins.json](evidence/SPK-0054/dependency-origins.json)。正式代码没有依赖
spike 可执行文件或忽略目录。独立 reference 只共享之前的受信 World/固定尺寸 helper，
自己构造 stock Content，不使用正式 adapter、editor origin 或 fork patch。

## 输入、布局与场景

文字/结构输入先按顺序提交 LocalSession，之后立即发完整不可变快照给常驻 CPU worker；
没有全文 markup 重解析、源码生成/编译去抖。worker 根据稳定 ID 和语义子树维护
Content，只重建变化路径。未改快照重复布局的 built=0、reused>0，像素不变。
**这仍是完整快照传输、子树比较和全 Frame/全栅格，不是节点级 delta 协议或增量分页。**
完整快照使 mailbox 合并时不需要猜遗漏 base，后续协议增量化须另测 base/重同步。

两个 mailbox 分别只保留一份待布局快照/完成结果；被覆盖的是派生工作，LocalSession
中已接受的动作不会丢。字体扫描、Content 布局、几何和栅格在后台，完成唤醒 UI；UI 不
运行 Typst 或 join worker。raster/geometry 取同一 padded Frame，命中/caret/IME 共用 pt
到窗口逻辑像素 placement。整套 SceneStamp 比较 document/epoch/request/revision/
profile/resources；新建、恢复、undo/redo 清理旧场景并换 epoch。

等待或布局失败仅保留标明画面 revision 的旧图，禁用旧命中/光标/IME；不切回 egui。
由模型保存必填 Hole，页面提示/preedit 不写正文；strict filled 检查仍拒绝 Hole。
受控 flow 宽 420 pt、高预算 2000 pt、padding 10 pt、2 px/pt 栅格；高超限拒绝，不截尾。
单机 undo 最多 512 步，每步连同其完整请求日志恢复，redo 不从缩短日志补猜请求身份。
dirty 比较完整快照，相同 revision 的另一份内容不能假报已保存。

## 可重复命令

从仓库根运行，指定新隔离文件：

```sh
SCHOLIUM_SESSION_FILE=/tmp/scholium-typst-candidate.sqlite \
  cargo +1.96.0 run --locked --manifest-path dev/typst-editor/Cargo.toml \
  --bin scholium-app -- --typst-editor
```

输入方式：`$` / Ctrl+M 进入/离开公式，Ctrl+/ 包成分数并进入空分母，Tab / Shift+Tab
换槽，Ctrl+Z / Ctrl+Shift+Z 撤销/重做，Ctrl+S 保存。根构建的 `--typst-editor` 给明确开发
manifest 提示并退出；它不能悄悄打开另一份模型。

```sh
cargo +1.96.0 fmt --all -- --check
cargo +1.96.0 clippy --locked --workspace --all-targets -- -D warnings
cargo +1.96.0 test --locked --workspace
RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace --no-deps
python3 scripts/check-rust-size.py
cargo deny check licenses bans sources

cargo +1.96.0 clippy --locked --manifest-path dev/typst-editor/Cargo.toml --all-targets -- -D warnings
cargo +1.96.0 test --locked --manifest-path dev/typst-editor/Cargo.toml -- --test-threads=1
cargo +1.96.0 build --locked --manifest-path dev/typst-editor/Cargo.toml --bin scholium-app
xvfb-run -a -s '-screen 0 1280x900x24' \
  python3 spikes/typst-main-editor/native-check.py \
  dev/typst-editor/target/debug/scholium-app /tmp/scholium-native-evidence

cargo +1.96.0 run --locked --manifest-path dev/typst-editor/Cargo.toml --bin typst-main-fork-check -- /tmp/fork-pixels
cargo +1.96.0 run --locked --manifest-path spikes/typst-main-editor/reference/Cargo.toml -- /tmp/stock-pixels
python3 spikes/typst-main-editor/compare.py /tmp/fork-pixels /tmp/stock-pixels
```

本次构建额外指定 `--target-dir target/typst-editor` 复用本机缓存；源与 lockfile 相同。
窗口证据目录必须为空。脚本用 Xvfb/xdotool/X11 剪贴板发送真实事件，以当前场景 caret
坐标点击，标准 WM_DELETE_WINDOW 关闭后重开。审计开关显式开启，记录的是双许可测试
夹具，不在普通启动记录用户正文。Xvfb 没有窗口管理器，不能用直接销毁 Drawable 冒充
正常关闭；测试脚本已改用窗口管理器协议，正常退出码为 0。

## 验证结果

| 检查 | 结果与证据 |
|---|---|
| 根 gates | fmt/clippy/tests/rustdoc/文件规模通过；145 passed、0 failed、32 个旧 ignored 不变；[root-tests.log](evidence/SPK-0054/root-tests.log) |
| 开发 gates | fmt/clippy/tests/rustdoc/build 通过；88 passed、0 failed、32 个旧 ignored；新增 8 个候选用例；[dev-tests.log](evidence/SPK-0054/dev-tests.log) |
| 原生主程序 | 十步新建→中英文→空分母→嵌套→undo/redo→保存正常退出→重开→继续编辑通过；[native-check.log](evidence/SPK-0054/native-check.log) |
| 窗口观测 | 37 条变更审计状态，其中 22 条 pending；全部 page_text_draws=0、已出场景 source_reads=0；[summary.json](evidence/SPK-0054/native/summary.json) |
| 实际绘制检查 | headless 使用真实 body painter，检查 epaint Shape：current/pending 均有 Typst Mesh、无 Text；IME commit 与同帧重复 Text 只接受一次 |
| 身份/存储 | 同 revision 旧 epoch 结果拒绝、六个 stamp 字段分别门控；redo 恢复请求身份，v1 重开 Hole/IDs/日志不变；legacy open_structured 字节不变且缺失文件不创建 |
| stock 对照 | full 嵌套分数、styles、heading 三组的尺寸及全部 RGBA 字节相同；[stock-comparison.json](evidence/SPK-0054/stock-comparison.json) |
| 许可/来源/bans | 根与开发 cargo-deny licenses/bans/sources 通过；[dev-deny-policy.log](evidence/SPK-0054/dev-deny-policy.log) |
| 完整 advisories | 根与开发均失败，仍是 7 项既有通告，没有增加 ignore；[dev-deny-advisories.log](evidence/SPK-0054/dev-deny-advisories.log) |

原生首轮抓出独立空公式的 `math hole requires a structural slot identity`：之前把根 Hole
的 slot 设为 None，而快速一次填满的单元夹具跳过了这个中间态。保留
[修复前观察](evidence/SPK-0054/empty-hole-before-fix.json)，补失败回归后将根 Hole slot
标为自身必填槽；再次逐步停留在空公式、空分母及完整窗口流程通过。

窗口例图：[空分母](evidence/SPK-0054/native/04-empty-denominator.png)、
[嵌套分数](evidence/SPK-0054/native/05-nested-fraction.png)、
[重开继续编辑](evidence/SPK-0054/native/10-continued.png)。

## 未通过和下一出口

advisories 是 bincode `RUSTSEC-2025-0141`、paste `2024-0436`、quick-xml `2026-0194/0195`、
rustybuzz `2026-0206`、ttf-parser `2026-0192`、yaml-rust `2024-0320`。
默认发行后端替换仍受阻；没有升级依赖、添加例外或把根 CI 的许可证通过说成安全通过。

本报告不宣称 K3/K4：快照/历史复制、子树比较、完整 flow 与栅格仍有规模成本；没有
物理 60 Hz、p95≤32 ms、大文档或分页 checkpoint 证明。X11 剪贴板中文只验证 Unicode
文字输入，headless IME 事件只验证分层/去重，不能代替 fcitx5 实机输入法或无障碍验收。
完整候选兼容、选区/clipboard/竞态、源码投影、Raw 转换、flow 分页和正式输出仍需 C–E。
下一批 C 应先补日常结构编辑与版本竞态，再进入 D 的分页/显示性能。
