# Spike 0026：Typst 视觉层替代近似结构拼接

- 日期：2026-09-18；基线 `9f3a397` 加本轮改动。
- 结论：真实窗口已验证 Typst 页面可作为左侧当前 revision 的视觉层；结构交互仍由原生模型提供。

## 发现

对比同一夹具截图后确认，左侧 core layout + egui 自绘与 Typst 的差异不是微调参数可以消除的：
分数、根号、上下标、数学间距、字体字形和行距分别来自两套排版系统。继续给自绘路径加常数只会制造
更多错位，不能作为产品视觉渲染。

## 改动

- 当后台 Typst 结果的 revision 等于当前文档 revision 时，左侧正文画布直接绘制该 Typst 页图像。
- 当前结果失效、编译中或失败时回退原生结构绘制，保留编辑可用性和失败诊断。
- 左侧文字命中、光标、选区、IME 和 AT-SPI 仍来自语义文档图与原生交互层；Typst 图像只负责视觉排版。
- 右侧继续提供多页预览、翻页、缩放和定位点；两侧使用同一 revision 的 Typst 结果。

这使左侧视觉质量与右侧 Typst 输出一致，同时明确它是“排版图像 + 原生交互覆盖层”，不是逐字可重排画布。
后续若要精确的图像内 caret/选区，需要使用 Typst generated source map 将节点位置映射到图像坐标。

## 验证

- egui 常规测试 34 项通过、2 ignored；Clippy `-D warnings` 和格式检查通过。
- 真实窗口的整节点选区和多页预览均通过；[复测截图](evidence/SPK-0026/recheck2/multipage-preview.png)
  显示左侧已经使用 Typst 页面视觉层，左/右排版来自同一工具链。
- 首次集成中把图片作为普通控件放在正文滚动区外，导致左侧空白；随后改为直接由正文画布 painter 绘制，
  定向复测通过。该失败证据保留在同目录的 `recheck/` 之前结果中。
- 显式使用 `spikes/mixed-build/out/packages/toolchain/typst` 的 Typst 0.15.1；未依赖 `/tmp` 默认路径。

## 边界

Typst 图像是页面级视觉结果，当前仅将第一页用于左侧编辑器画布；长文档的完整页面交互仍在右侧预览。
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
