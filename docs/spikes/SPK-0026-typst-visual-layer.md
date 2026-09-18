# Spike 0026：Typst 视觉层替代近似结构拼接

- 日期：2026-09-18；基线 `9f3a397` 加本轮改动。
- 结论：**整页覆盖方案拒绝交付**。Typst 质量明显更好，但整页图像会与左侧交互坐标脱钩；仅保留为架构实验和对比证据。

## 发现

对比同一夹具截图后确认，左侧 core layout + egui 自绘与 Typst 的差异不是微调参数可以消除的：
分数、根号、上下标、数学间距、字体字形和行距分别来自两套排版系统。继续给自绘路径加常数只会制造
更多错位，不能作为产品视觉渲染。

## 改动

- 实验曾在当前 revision 有 Typst 结果时，把该 Typst 页图像绘制到左侧正文画布。
- 实验显示视觉质量改善，但图像缩放坐标与原生模型的命中、光标、选区没有同一映射。
- 该实现已撤回；左侧当前仍使用结构绘制，等待节点级 source map 映射方案。
- 右侧继续提供多页预览、翻页、缩放和定位点；两侧使用同一 revision 的 Typst 结果。

这证明“排版图像 + 原生交互覆盖层”只有在 source map、缩放和滚动坐标统一后才能交付，不能仅凭窗口脚本通过就宣称可用。下一步必须建立节点级 source map 几何映射。

## 验证

- egui 常规测试 34 项通过、2 ignored；Clippy `-D warnings` 和格式检查通过。
- 真实窗口的整节点选区和多页预览均通过；[复测截图](evidence/SPK-0026/recheck2/multipage-preview.png)
  显示实验视觉层的质量明显优于左侧拼接；但该方案因交互坐标脱钩不作为产品实现。
- 首次集成中把图片作为普通控件放在正文滚动区外，导致左侧空白；随后也确认直接 painter 绘制仍不能解决坐标映射，最终保留证据并撤回覆盖实现。该失败证据保留在同目录的 `recheck/` 之前结果中。
- 显式使用 `spikes/mixed-build/out/packages/toolchain/typst` 的 Typst 0.15.1；未依赖 `/tmp` 默认路径。

## 边界

Typst 图像是页面级视觉结果，本轮仅用于对比，不再用于左侧编辑器画布；长文档的完整页面交互仍在右侧预览。
左侧图像中的精确 caret、鼠标定位和结构选区尚未做到像素级映射；回退结构视图也保留近似排版。
阶段 0 当前判定仍以报告 0012 为准，原生共同验收、完整 source map 和文件恢复等缺口未关闭。

## 复现

```bash
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"
cargo test --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
cargo clippy --offline --all-targets --manifest-path spikes/native-ui/candidate-egui/Cargo.toml -- -D warnings
cargo build --release --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
SCHOLIUM_TYPST_BIN="$PWD/spikes/mixed-build/out/packages/toolchain/typst" \
SCHOLIUM_SPIKE_BINARY=spikes/native-ui/candidate-egui/target/release/scholium-spike-egui \
  /usr/bin/python3 spikes/native-ui/candidate-egui/scripts/native-edit-acceptance.py /tmp/scholium-typst-layer
```
