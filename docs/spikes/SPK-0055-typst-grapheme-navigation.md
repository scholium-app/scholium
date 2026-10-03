# SPK-0055：候选输入的字素边界与逻辑叶导航

- 日期：2026-10-02；结论：**Pass（C 的限定输入批次）**。
- 基线：`e852ffd1f51f4db72d39c879b8a6421fd97ac41e`，其根与候选
  [CI 36894295585](https://github.com/scholium-app/scholium/actions/runs/36894295585) 均通过。
- 分支：`codex/fix/typst-grapheme-navigation`；本报告同提交的源码与实际二进制指纹见
  [source-hashes.json](evidence/SPK-0055/source-hashes.json)。
- 对应[阶段 0 Typst 映射](../plan/ROADMAP.md)、[主程序接入计划 C](../plan/TYPST_MAIN_APP_INTEGRATION.md)、
  [ADR 0034](../adr/ADR-0034-typst-main-app-candidate.md)；完整阶段判定仍见报告 0012。

## 问题与判据

输入/删除后的原 byte 可能不再是新文本的字素边界：在 `👩🔬` 中间插入 ZWJ 后形成
`👩‍🔬`；删除 `🇦x🇧` 的 x 后形成一个区域指示符字素。旧逻辑保留字素内部 byte，后续
输入被核心拒绝。候选左右方向键还停在当前叶边缘，无法自然进入邻接正文或数学叶。

先添加三个回归，基线结果为 8 个原用例通过、3 个新用例失败，见
[before-fix.log](evidence/SPK-0055/before-fix.log)。判据是合并字素后继续输入不被拒绝、
光标保持合法 UTF-8 字素边界、左右键跨文字/数学叶但不首尾循环，导航不追加正文动作。

## 实现边界

接受编辑后在唯一 LocalSession 的新投影中寻找目标 byte 之后的首个字素边界，避免把下一
次文字请求送到字素内部。左右键按稳定叶 ID 的逻辑顺序移动；范围内使用字素边界，跨叶
进入下一叶开头或上一叶末尾。此层不计算排版坐标，也不使用旧 Scene 判断模型位置。

只修改正式主程序的输入适配，未改变模型/schema、Typst fork、Content adapter 或排版
资源。普通启动仍走 legacy；显式开发候选仍仅用 Typst 画已提交正文。
**这不代表完整 C 通过**：拖选、Shift 选区、跨 inline/块原子替换、复制剪切、多行粘贴、
视觉 bidi/上下行导航、未知结构与完整竞态集仍须后续验收。区域指示符夹具只验证字素，
不评价 emoji 字体覆盖；Unicode 剪贴板不冒充实机 IME。

## 环境与复现

Linux x86_64，Rust/Cargo 1.96.0，eframe 0.36.2，锁定 Typst 0.15.1 fork。来源与字体环境
沿用[报告 0054](SPK-0054-typst-main-app-candidate.md)，没有变更库版本或打包字体。
验证实际 `scholium-app`，X11/Xvfb 1280×900；窗口脚本使用 xdotool、xclip、ImageMagick。

```sh
cargo +1.96.0 fmt --all -- --check
cargo +1.96.0 clippy --locked --workspace --all-targets -- -D warnings
cargo +1.96.0 test --locked --workspace
python3 scripts/check-rust-size.py
cargo +1.96.0 clippy --locked --manifest-path dev/typst-editor/Cargo.toml --all-targets -- -D warnings
cargo +1.96.0 test --locked --manifest-path dev/typst-editor/Cargo.toml -- --test-threads=1
cargo +1.96.0 build --locked --manifest-path dev/typst-editor/Cargo.toml --bin scholium-app
xvfb-run -a -s '-screen 0 1280x900x24' \
  python3 spikes/typst-main-editor/native-check.py \
  dev/typst-editor/target/debug/scholium-app /tmp/scholium-c1-native
```

本机额外使用 `--target-dir target/typst-editor` 复用缓存。窗口证据目录必须为空。脚本
在 B 的十步闭环后，用方向键进入邻接正文，插入 ZWJ 后继续键入、删除字符使 RI 合并后
继续键入，再保存正常退出并重开核对完整快照。每次截图检查当前 cursor 在同一 Scene
中有 caret；pending 仍检查正文 Text draw 为 0。日志仅规整行尾/EOF 空白。

## 结果

| 检查 | 结果与证据 |
|---|---|
| 回归 | 原 8 + 新 3 个候选用例均通过；导航不改变 revision，两个合并字素场景可继续输入 |
| 开发 gates | fmt/clippy/build 通过；91 passed、0 failed、32 个旧 ignored；[dev-tests.log](evidence/SPK-0055/dev-tests.log) |
| 根 gates | fmt/clippy/tests/文件规模通过；145 passed、0 failed、32 个旧 ignored；[root-tests.log](evidence/SPK-0055/root-tests.log) |
| 实际窗口 | 13 步流程通过，保存重开后的完整 Unicode 快照相同；[native-check.log](evidence/SPK-0055/native-check.log) |
| 绘制与 caret | 每个记录的 current cursor 有同 Scene caret，全部已提交正文 egui Text 为 0；[summary.json](evidence/SPK-0055/native/summary.json) |

截图：[合并 Emoji 后继续输入](evidence/SPK-0055/native/11-joined-emoji.png)、
[删除后继续输入](evidence/SPK-0055/native/12-joined-regional-indicators.png)、
[保存重开](evidence/SPK-0055/native/13-unicode-reopened.png)。

C 下一批应形成结构范围的原子替换合约，再连接拖选与复制剪切；不能用连续单叶请求假装
选区替换是一次动作。分页/显示时延仍属于 D；七项既有依赖通告未处理，默认替换仍未完成。
