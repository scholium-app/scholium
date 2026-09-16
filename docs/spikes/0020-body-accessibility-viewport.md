# Spike 0020：正文可访问文本、选区与滚动视口

- 日期：2026-09-16；基线 `56acce6` 加本轮修改。
- 平台/版本：Arch Linux / niri Wayland，eframe/egui 0.36.2、AccessKit 0.24.1；未新增依赖。
- 结论：**Pass（本报告文本叶子接口与视口回归范围）；Fail（完整编辑器共同验收）。**
- 阶段 0 仍为 6 项在范围内满足、4 项部分满足，不升级条件 1、3。

## 改动与证据

正文从只读 Label 改为带 TextRun 子节点的 MultilineTextInput；始终发布文本，即使源码持有焦点。
AT-SPI 的 Text 接口可读取文本、光标、选区并请求 SetTextSelection。
接口字符位置按 Unicode scalar 转为核心 UTF-8 byte offset；非有效字素边界的请求被拒绝。
选区经现有核心编辑/历史通道执行，未增加第二份可写正文。

文本按模型前序排列，不能沿用数学图元绘制顺序：后者先画上标，模型则先遍历下标。
回归曾读出 `abx21123y`，修复后为 `abx12123y`，与选区删除所用顺序一致。
合成分数线、根号等暂不冒充文本叶子；这仍不是数学语义朗读接口。

| 验证 | 结果 |
|---|---|
| Unicode 字符位置 | 中文 + 四字节 emoji 的字符位置 2 映射到 byte 7；拒绝组合字符内部位置 |
| AccessKit 树与编辑 | TextRun 只有一个父节点；跨叶选区剪切、撤销恢复原文 |
| 视口 | 双轴 ScrollArea；长文本横向滚动改变画布原点，绘制、命中和 IME 共用该原点 |
| 窄窗口 | 原有 844 逻辑像素宽度回归继续通过 |
| 原生源码流程 | 键盘输入 CHECK、复制 CK、粘贴得到 CHECKCK，应用正文并生成预览 |
| 真实正文选区 | AT-SPI 选取偏移 0–15，跨正文文本到分子 a；读回同一范围 |
| 原生正文剪切/撤销 | Ctrl+X 后正文剩余后续叶子；Ctrl+Z 恢复全部可访问文本；核心历史追加动作 |
| 后台预览 | 等待预览 revision 与正文一致，剪切/撤销后再等待追平，不硬编码初始 revision |

[选区与前后文本](evidence/0020/body-selection.json)、[AT-SPI 接口](evidence/0020/a11y.json)、
[事件日志](evidence/0020/app.log)、[最终窗口](evidence/0020/window.png)、
[状态文本](evidence/0020/interaction.txt)。截图仅包含被测进程的窗口。

## 复现

```bash
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"
cargo fmt --manifest-path spikes/native-ui/candidate-egui/Cargo.toml -- --check
cargo test --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
cargo clippy --offline --all-targets --manifest-path spikes/native-ui/candidate-egui/Cargo.toml -- -D warnings
cargo build --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
python3 spikes/native-ui/candidate-egui/scripts/window-acceptance.py /tmp/scholium-ui-0020
```

脚本临时启用会话无障碍开关与 ydotool，退出恢复原状态；每次键盘注入前核验焦点 PID。
共享桌面可能收到其它真实键盘输入，因此预览匹配当前正文 revision，而非固定数值。
20 个非 ignored 测试通过（新增 3 个），另 1 个既有预览集成测试保持 ignored；真实桌面流程单独验证预览。
Fmt、Clippy all-targets、构建通过；两个无障碍开关已恢复 false，ydotool 恢复 inactive。
本轮不重复无关的完整阶段 0 构建与隔离验证。

## 剩余边界与下一步

1. 文本叶子扁平连接，没有数学角色、分子/分母/上下标语义和分块分隔；需结构化朗读与屏幕阅读器验收。
2. 双轴滚动有 headless 输入回归；尚未增加真实指针拖拽滚动/跨节点拖选证据，也没有光标自动跟随。
3. 文本框仍用 spike 近似度量；未提供逐字符屏幕几何、双向文本、正式换行与字体排版。
4. 本轮没有中文 IME 实际合成、取消或焦点切换证据；保留报告 0019 的环境限制。
5. GPU/合成器呈现 p95 尚未测量，headless 性能测试不能替代它。

下一步优先补数学结构语义与真实指针选区/滚动验收，然后处理 IME 会话及呈现侧采样。
