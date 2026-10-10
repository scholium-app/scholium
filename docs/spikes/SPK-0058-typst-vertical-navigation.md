# SPK-0058：Typst 主程序候选的视觉行导航与几何键有序排队

- 日期：2026-10-10；结论：**Pass（C 的限定视觉行导航批次）**。
- 对应：[主程序接入计划 C](../plan/TYPST_MAIN_APP_INTEGRATION.md)、
  [阶段 0 Typst 映射](../plan/ROADMAP.md)、[ADR 0035](../adr/ADR-0035-typst-page-selection.md)。
- 基线：main `07f6a59`（含 C2 选区与纯文本剪贴板）；分支 `ation_ciger/feat/typst-vertical-navigation`。
  源码与运行二进制指纹见 [source-hashes.json](evidence/SPK-0058/source-hashes.json)。
  无依赖、锁文件、fork 补丁或存储 schema 变化。

## 判据与实现

实际 `scholium-app` 显式 Content 入口的上下方向键与 Home/End 只读**当前** Scene 的
精确正文 caret（padded Frame pt）推出视觉行。实测正文 caret 纵向范围为字形顶到基线，
行内分数的分子/分母与正文行纵向重叠，因此数学槽位不参与划行。上下键取纵向最近的
相邻行，再取最接近目标 x 的停靠点；连续上下在光标与期望 SceneStamp 都未变时保留
目标 x，任何编辑、恢复或其他移动自然使其失效。软换行同一字节有两个停靠点：Upstream
属上一行末、Downstream 属下一行首；等 x 时取逻辑较后者，使行末落在尾随空格之后。
无相邻行时停在本行首/末；空段落是停靠行。

数学槽内上下先在包含光标的最内层分数的分子/分母间切换，取目标槽中最近停靠点；
否则以整公式 bounds 代表当前行离开公式，落到相邻行正文。槽内 Home/End 保持槽内
首末。Ctrl+Home/End 按逻辑顺序到正文首末，不需要几何。Shift 上下/Home/End 以同一规则
扩展正文端点选区；数学端点的 Shift 移动明确拒绝。导航不产生语义动作。

排版未就绪时不读旧几何：几何键与其后的全部输入按到达顺序进入有界队列（256），
Scene 当前后依次执行，遇到下一个需要更新几何的键再等待。Ribbon 意图、撤销/重做与
保存在队列清空前保持请求状态，不越过先到的按键；排队期间鼠标按下被拒绝（其坐标
指向将被排队输入改变的画面）；超限文字进入可复制草稿，IME 与 Escape 不受上限丢弃；
布局失败只放弃排队的几何键并报告，其后输入继续执行；关闭窗口时排队输入按未保存处理；
新建文档清空针对旧文档的队列。

## 验证中发现并修复

1. **未处理按键重置 affinity。** 实机 Shift+Up 前出现一帧把行末 `Upstream` 改成
   `Downstream`：`key()` 对未识别按键也统一设置 Downstream，把软换行行末移到下一行首，
   Shift+Up 因此从错误行出发。先写失败回归（未处理键后光标不变、Shift+Up 离开该段），
   再改为未处理键直接返回。
2. **空叶使选框几何失败。** 选区跨过公式后与其他行内内容相邻的空 Text 叶时，Typst 不为它
   成 cluster，`selection_quads` 报几何不支持并拒绝替换（C2 既有问题，Shift+左右同样触发）。
   该叶没有可绘范围，选框跳过无 caret 的空叶，语义范围仍包含它；补回归后替换一次提交。
3. **代码评审。** 目标 x 原只按光标与 SceneStamp 判定，Shift+左右回到同一停靠点后会
   复活旧目标 x：改为任何非上下键或 `move_to` 都清除。队列满时 IME 取消/Escape 也被丢弃
   可能卡在组合态：二者改为不受上限。排队输入未进快照，关闭时却显示已保存：计为未保存。
   三项均先写失败回归再修复。
4. **实机脚本。** 原第 11 步用叶内 Home 定位公式后的正文叶，改为等价的 Left；脚本内叶
   身份统一使用审计 Debug 形式。观察到画布焦点过滤未捕获 Escape，egui 会在 Escape 时释放
   页面焦点、后续按键不再进入画布；这是既有行为，本批未改，脚本改用 Ctrl+Home 折叠选区。

## 环境与复现

Linux x86_64 / X11/Xvfb 1280×900，Rust/Cargo 1.96.0，eframe 0.36.2，Typst 0.15.1 fork；
工具为 xdotool、xclip、ImageMagick。fork 补丁与锁文件未变，来源核对沿用报告 0057，
由 CI 的 `verify-fork.py` 重跑。

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
  dev/typst-editor/target/debug/scholium-app /tmp/scholium-c4-native
cargo deny --manifest-path dev/typst-editor/Cargo.toml --locked --config deny.toml check licenses bans sources
cargo deny --manifest-path dev/typst-editor/Cargo.toml --locked --config deny.toml check advisories
```

## 结果

| 检查 | 结果与证据 |
|---|---|
| 根回归 | **157 passed / 0 failed / 32 ignored**（候选 feature 不进根 workspace）；[root-tests.log](evidence/SPK-0058/root-tests.log) |
| 候选回归 | **126 passed / 0 failed / 32 ignored**，新增 20 项行导航/排队用例；[dev-tests.log](evidence/SPK-0058/dev-tests.log) |
| gates | 根/候选 fmt、clippy、build、rustdoc、文件规模与 licenses/bans/sources 通过；完整 advisories 单列 Fail |
| 实际窗口 | **33 步全通过**：既有 24 步，再长段软换行、第二行 Home、Up 回首行并 Down 原位返回、软换行 End、Shift+Up 跨段选区、一次替换、撤销恢复、“k↓m”无等待连按保序、保存重开；[native-check.log](evidence/SPK-0058/native-check.log) |
| 绘制与竞态 | 121 audit frames，48 pending、1 帧观察到排队输入、12 帧有选区；committed egui Text draw **0**；[summary.json](evidence/SPK-0058/native/summary.json) |

行导航回归覆盖：最近行优先于水平距离、等 x 逻辑序、长段两行+短行+长段的目标 x 保持与
原位返回、首行 Up/末行 Down 停在行首/末、空段落停靠、软换行 End/Home 的 affinity 与
插入位置、分子↔分母及离开公式、槽内 Home、Shift+Down 跨段一次替换、数学 Shift 上下拒绝、
Ctrl+Home/End、pending 几何键后输入保序、队列上限草稿、布局失败放弃几何键、撤销与
Ribbon 意图等待队列、排队期间鼠标拒绝、未处理键保留 affinity、空叶选框、水平移动清除
目标 x、上限时 IME 取消仍入队、排队输入关闭确认。

已目检截图：[软换行行末](evidence/SPK-0058/native/28-end-at-soft-wrap.png)、
[Shift+Up 选区](evidence/SPK-0058/native/29-shift-up-selection.png)、
[保序输入](evidence/SPK-0058/native/32-type-down-type-order.png)。01–24 步截图与报告 0057
同一流程，本批只保留新增阶段。

完整 advisories **Fail**，仍是 bincode、paste、quick-xml 两项、rustybuzz、ttf-parser、
yaml-rust 七项既有错误，与报告 0057 编号一致；[advisories.log](evidence/SPK-0058/advisories.log)。
不新增 ignore，依照 ADR 0034 保留开发入口隔离。

## 未覆盖与下一步

普通启动仍为 legacy。只有公式而无正文 caret 的行不是上下停靠行（经左右键/Tab 进入）；
上下键不在相邻行进入公式内部；目标 x 不跨编辑保留；视觉 bidi、分页翻页跟随、PageUp/
PageDown、实机中文 IME 与无障碍未验收。Escape 释放页面焦点的既有行为待单独裁决。
完整 C 仍缺结构化剪贴板、数学内部跨槽位选区、选区格式和完整能力竞态集；阶段判定不升级。
