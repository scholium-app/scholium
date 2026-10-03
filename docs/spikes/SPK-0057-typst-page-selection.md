# SPK-0057：Typst 主程序候选的同源选区与原子多行替换

- 日期：2026-10-02；结论：**Pass（C2 的限定正文选区批次）**。
- 对应：[主程序接入计划 C](../plan/TYPST_MAIN_APP_INTEGRATION.md)、
  [阶段 0 Typst 映射](../plan/ROADMAP.md)、[ADR 0035](../adr/ADR-0035-typst-page-selection.md)。
- 基线：C1 `8cc23f57d5127d9b1623a063e324a694716b9efd` 合入已发布的范围内核 main
  `ee7c9e5757cabfaa0e94c7b9a5910ff45c8ff152`，本地合并 `7cc8414`（完整指纹见证据）。
- 分支：`codex/feat/typst-page-selection`；源码与运行二进制指纹见
  [source-hashes.json](evidence/SPK-0057/source-hashes.json)。无依赖、锁文件或存储 schema 变化。

## 判据与实现

实际 `scholium-app` 的显式 Content 入口从同一 padded Frame 读取正文 shaped cluster
stops、数学根结构 bounds，以非打印多边形显示选区。正文 Text 端点存稳定 ID 与 UTF-8
字素 byte，按逻辑顺序归一化；跨叶/段落允许完整覆盖公式，数学内部端点不在本批。
鼠标拖选与 Shift 左右/Home/End 共用这组语义端点，覆盖范围不转成可写 markup。

输入、删除、Enter、多行 Paste 调用一次 ReplaceBodyRange。新增 StructuralOutcome 由
核心规划器返回实际存活叶和合法字素光标，包括拆段创建的新叶；bool API 保持兼容。
撤销保存两端和完整快照/请求日志，restore 换 epoch。Raw/容量拒绝不改正文或两端，
输入保留可复制草稿；选区格式/结构命令明确拒绝。

指针按输入事件顺序处理，只接收当前完整 SceneStamp、已知精确 caret。拖选绑定 stamp
和页面 placement；先接受文字再到来的旧指针、恢复后相同 revision、缩放/位置变化、
未知或比例合字坐标都不能成为新命中。pending 不画旧选框或 caret，但允许按合法语义
端点继续键盘编辑；包含 Raw 的范围即使 pending 也拒绝。

首轮选区测试暴露整公式查询了 inline 包装身份，其 Frame 没有 bounds；改用数学根
NodeId 后，分数横线与上下槽位完整包含，针对性回归通过。初次窗口截图有先打印 audit
后 GPU 呈现的时序差；脚本在截图前留 120 ms 呈现间隔，第二次完整流程截图一致。
此间隔不作为输入延迟测量。原失败和首次取证摘要保留在证据目录。

## 环境与复现

Linux x86_64 / X11/Xvfb 1280×900，Rust/Cargo 1.96.0，eframe 0.36.2，Typst 0.15.1
fork。真实主程序源码由隔离 manifest 编译；不使用 standalone spike 编辑器。工具为
xdotool、xclip、ImageMagick 与标准 X11 关闭消息。完整字体/来源沿用报告 0054；本批
重新从 Git 对象和四份已跟踪补丁重建核对 438 文件，source digest 不变。

```sh
cargo +1.96.0 fmt --all -- --check
cargo +1.96.0 clippy --locked --workspace --all-targets -- -D warnings
cargo +1.96.0 test --locked --workspace
RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace --no-deps
python3 scripts/check-rust-size.py
cargo +1.96.0 fmt --manifest-path dev/typst-editor/Cargo.toml --all -- --check
cargo +1.96.0 clippy --locked --manifest-path dev/typst-editor/Cargo.toml --all-targets -- -D warnings
cargo +1.96.0 clippy --locked --manifest-path dev/typst-editor/Cargo.toml -p scholium-typst --all-targets -- -D warnings
cargo +1.96.0 test --locked --manifest-path dev/typst-editor/Cargo.toml -- --test-threads=1
cargo +1.96.0 build --locked --manifest-path dev/typst-editor/Cargo.toml --bin scholium-app
xvfb-run -a -s '-screen 0 1280x900x24' \
  python3 spikes/typst-main-editor/native-check.py \
  dev/typst-editor/target/debug/scholium-app /tmp/scholium-c2-native
python3 dev/typst-editor/verify-fork.py /path/to/pinned/upstream/git/checkout
cargo +1.96.0 run --locked --manifest-path dev/typst-editor/Cargo.toml --bin typst-main-fork-check -- /tmp/c2-fork-pixels
cargo +1.96.0 run --locked --manifest-path spikes/typst-main-editor/reference/Cargo.toml --target-dir dev/typst-editor/target -- /tmp/c2-stock-pixels
python3 spikes/typst-main-editor/compare.py /tmp/c2-fork-pixels /tmp/c2-stock-pixels
cargo deny --manifest-path dev/typst-editor/Cargo.toml --locked --config deny.toml check licenses bans sources
cargo deny --manifest-path dev/typst-editor/Cargo.toml --locked --config deny.toml check advisories
```

## 结果

| 检查 | 结果与证据 |
|---|---|
| 根回归 | **157 passed / 0 failed / 32 ignored**，包括 12 项范围核心测试；[root-tests.log](evidence/SPK-0057/root-tests.log) |
| 候选回归 | **103 passed / 0 failed / 32 ignored**，新增 12 项页面选区用例；[dev-tests.log](evidence/SPK-0057/dev-tests.log) |
| gates | 根/候选 fmt、clippy、build、rustdoc、文件规模与 licenses/bans/sources 通过；完整 advisories 单列 Fail |
| fork 来源 | 上游对象 + 四补丁逐文件重建一致；[fork-source.json](evidence/SPK-0057/fork-source.json) |
| 独立 stock | full/styles/heading 三组相同大小、逐像素一致；[stock-comparison.json](evidence/SPK-0057/stock-comparison.json) |
| 实际窗口 | 24 步全通过：既有 Unicode/数学流程，再整公式选择替换、撤销恢复选区、反向拖选、多行粘贴、跨段 Shift 替换及继续输入、撤销重做、保存和重开；[native-check.log](evidence/SPK-0057/native-check.log) |
| 绘制与竞态 | 74 audit frames，34 pending、9 有选区；全部 committed egui Text draw **0**、source reads **0**，记录的当前 cursor 具有同 Scene caret；[summary.json](evidence/SPK-0057/native/summary.json) |

选区回归覆盖 Unicode/literal、整分数 bounds、反向跨段、多行/空段落、删除/Enter 单
动作、新叶继续输入、撤销恢复两端、新 epoch 同 revision、同帧旧指针、placement 变化、
未知/比例 caret、容量拒绝、pending Raw 保留及当前 cluster 边界/pending 隐藏。

已目检截图：[整公式](evidence/SPK-0057/native/14-whole-formula-selection.png)、
[反向拖选](evidence/SPK-0057/native/17-reverse-drag.png)、
[跨段 Shift 选区](evidence/SPK-0057/native/19-cross-paragraph-shift-selection.png)、
[保存重开](evidence/SPK-0057/native/24-range-reopened.png)。区域指示符字体显示不作为字形
覆盖证明；模型字素与保存前后完整快照由脚本断言。窗口日志仅规整 EOF/行尾空白。

完整 advisories **Fail**，仍是 bincode、paste、quick-xml 两项、rustybuzz、ttf-parser、
yaml-rust 七项既有错误；[advisories.log](evidence/SPK-0057/advisories.log)。不新增 ignore，
不把许可通过或候选可用当成安全门禁通过，依照 ADR 0034 保留开发入口隔离。

## 未覆盖与下一步

普通启动仍为 legacy，不能把本批当成默认去回显已经完成。完整 C 尚缺复制/剪切与
结构化剪贴板、数学内部跨槽位选区、选区格式、视觉 bidi/上下行和完整能力竞态集。
旧 Raw 转换、分页、物理延迟、实机中文 IME 与无障碍另需验收；规划、历史和栅格仍有
全文成本，不证明任意文档逐键增量。阶段判定不升级。

下一批接入有明确降级说明的纯文本复制/剪切，继续复用同一原子范围和语义端点；安全
门禁需独立修复和重验，不能把本 C2 草稿合入默认发行路径。
