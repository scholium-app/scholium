# SPK-0028：左侧替换为 Typst 编辑画面

- 日期：2026-09-18
- 结论：**Pass（下述限定窗口场景）**；完整共同验收未关闭
- 关联：阶段 0 原生编辑与 Typst 映射；[ADR 0013](../adr/ADR-0013-typst-editor-scene.md)
- 平台：当前 Linux / niri Wayland / fcitx5-Rime / AT-SPI；egui 0.36.2、Typst 0.15.1

## 实现

左侧默认替换为 Typst 栅格化页面。隔离 helper 一次编译输出页面图片和 glyph/shape 的源码 span、
位置、advance 与墨迹框；UI 从编辑投影中的源范围反查 NodeId 和 UTF-8 byte。
图片、光标、命中、选区、IME 和无障碍字符框使用同 revision 坐标，不再叠加旧自绘光标。
后台只保留最新待处理请求，过期结果不采纳；编译中保留旧画面并显示版本，禁用旧坐标定位。
键盘编辑仍作用于当前 SDG，不阻塞 UI 等待编译。

helper 使用既有 `toolchain-sandbox.sh`，保留隔离网络、项目只读、输出目录可写、时间和资源限制。
UI 不链接 Typst。左右显示共用编辑投影：真正行内数学、两列矩阵与受控数学字体；空槽位有编译器
定位的方框。该方框不写入模型或 Source Studio 的可写源码。

## 真实窗口验证

最终证据：[结果 JSON](evidence/SPK-0028/final/results.json)、
[左右画面](evidence/SPK-0028/final/visual-comparison.png)、
[包含根号的整节点选区](evidence/SPK-0028/final/slot-selection-highlight.png)。

七组通过：

1. 真实 Rime 预编辑、取消、提交及焦点切换。
2. 分子/分母导航、包裹根式、解除结构、继续输入和撤销。
3. AT-SPI 选择、焦点与数学语义。
4. 跨正文/公式拖选、剪切/撤销、长文本换行与水平滚动。
5. ZWJ emoji / 组合字符剪贴板及字素删除。
6. 整节点选择、替换、撤销、删除后继续输入。
7. 左右显示均采纳当前 revision，人工检查截图确认公式和选区几何。

测试脚本等待当前 Typst revision 后再读取几何，不再使用固定 0.3 秒假定排版已完成。
旧滚动断言要求长文本永不换行，与页面排版冲突；新夹具包含单词间空格，检查换行后的字符 Y
坐标并继续验证水平滚动。没有将长不可断行串的溢出视为已修复。

失败证据保留在 `first/`、`edit/`、`recheck/`、`scroll/`：
数学字符串初版丢失 glyph source span（画面有字、交互缺字），改为 `#text(...)` 后通过；
分子边缘初版误命中相邻底数，改为实际框距离与垂直位置排序后通过。
旧长行断言先失败，随后明确更换为换行/滚动场景。

## 自动验证与复现

- egui 常规回归（含新增旧结果门）：35 项通过；常规运行 4 项 ignored，其中两项新编译测试另行执行。
- 新增独立旧结果门测试，以及两项需要真实沙箱的编译测试：标准夹具每个文本叶子均有坐标、空叶子可定位、
  转义字符/空格/多字符数学内容的投影重建不丢字。两项沙箱测试显式运行通过。
- egui Clippy `--all-targets -- -D warnings` 通过。
- 旧字体布局相关 headless 测试显式调用 `new_layout_probe`，只用于保留已有回归，不声称验证新后端。
  新后端通过独立编译测试和上述真实窗口场景验证。

```sh
# 构建 helper 与桌面程序并启动（无需 Node/WebView）。
bash spikes/native-ui/candidate-egui/scripts/run-typst-editor.sh

CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" cargo test --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" cargo test --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml typst_editor::tests -- --ignored --nocapture
SCHOLIUM_TYPST_BIN="$PWD/spikes/mixed-build/out/packages/toolchain/typst" /usr/bin/python spikes/native-ui/candidate-egui/scripts/native-edit-acceptance.py /tmp/scholium-typst-editor ime math_edit accessibility pointer_selection slot_selection unicode_clipboard visual_comparison
```

## 剩余边界

每次编辑仍启动新编译进程，尚未做持久 World 或瓦片更新，键入到新画面的延迟可感知；本轮没有
建立端到端延迟分位数，不承诺固定毫秒预算。页面宽度当前固定，极长无空格文本可能溢出。
完整双向文本、合字内部插入位、全部结构变体、图像/链接/裁剪/变换、所有大文档场景未验收。
helper 明确拒绝不支持的 frame 类型。

Source Studio 仍使用原有 reconcile 投影，显示投影已统一，但发布源码中的矩阵行列/行内规则等
旧差异尚需单独修正；本轮不把显示投影冒充发布源码一致性验证。结构变体继承原 spike 的未完成边界。
右侧是 Typst 快速显示，不是 LaTeX 最终输出。阶段 0 不升级。
