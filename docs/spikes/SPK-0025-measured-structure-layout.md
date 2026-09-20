# Spike 0025：数学结构实测布局与节点选框

- 日期：2026-09-18；基线 `d0cde89` 加本轮改动。
- 结论：本轮结构尺寸、节点选框和既有桌面交互路径通过；阶段 0 未关闭。

## 实现范围

core 布局参数新增可选 `TextMeasurer`，调用方提供文字宽度、高度、基线，核心不依赖字体库。
egui 使用绘制所用的 galley 提供这些尺寸，分数线宽、根号位置、上下标、矩阵单元格及相邻结构
据此排列。未传入测量接口的旧比较候选继续使用近似度量。

每个节点输出完整布局盒，平移、行内排列、分数、上下标和矩阵合成时保留后代盒。
整节点高亮使用节点盒，覆盖自身合成符号和线条，不再只取文本后代外框。
负顶部归一化同步移动节点盒；空文本在实测路径保留 6 个逻辑像素的可编辑宽度和字体行高。
矩阵布局拆为独立文件，修改的 Rust 文件均未超过 600 行。

已查看[整节点选区截图](evidence/SPK-0025/slot-selection-highlight.png)：左侧根号与上横线均处于选框内。
结构盒是逻辑布局范围，不承诺任意字体斜体字形的墨迹外溢范围；根号/括号仍未按嵌套内容伸缩，
自动换行、多行选区、数学字体 MATH 常量及跨平台排版尚未完成。
core 旧 `caret`/`hit_test` 仍属于近似比较路径；egui 继续用报告 0024 的 galley 几何进行交互。

## 验证

- core：47 项通过。
- egui：34 项通过、2 ignored；全 targets Clippy `-D warnings`、格式及差异检查通过。
- 新增回归验证：结构盒包含后代、实测文本宽度与布局一致、分数留白、根号外框包含符号及上横线、
  空分母保留可编辑盒和光标。
- [真实桌面结果](evidence/SPK-0025/results.json)：IME、双源码、数学编辑、AT-SPI、鼠标拖选/滚动、
  Unicode 剪贴板、整节点替换/撤销、多页预览八组全部通过；CPU 采样存在性检查通过。
- 环境：Linux / niri / Wayland、egui 0.36.2、AT-SPI 开启、release 构建，显式使用已有工具包的 Typst 0.15.1。
- 256 段 / 20 页窗口 CPU：6 组各 120 帧，组内 p95 为 8.901–10.543 ms，最大单帧 23.921 ms；
  编译加栅格化 4028 ms。仅记录测量，不以固定 15/16 ms 判定；不同轮次系统负载不同，不直接归因为代码性能变化。
  CPU 指标不含 GPU/合成器，也不是左侧单独渲染或端到端输入延迟。

## 复现

```bash
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"
cargo test --offline --manifest-path spikes/native-ui/core/Cargo.toml
cargo test --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
cargo clippy --offline --all-targets --manifest-path spikes/native-ui/candidate-egui/Cargo.toml -- -D warnings
cargo build --release --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
SCHOLIUM_TYPST_BIN="$PWD/spikes/mixed-build/out/packages/toolchain/typst" \
SCHOLIUM_SPIKE_BINARY=spikes/native-ui/candidate-egui/target/release/scholium-spike-egui \
  /usr/bin/python3 spikes/native-ui/candidate-egui/scripts/native-edit-acceptance.py /tmp/scholium-measured-layout
```

Typst 路径为本机生成产物，干净机器需准备相同版本并指定路径。未重跑其它 UI 候选、完整 core Clippy、
大源码 ignored 基准、安全与重建包验证。下一步仍需可伸缩数学符号与嵌套排版验证；共同验收其它缺口见报告 0012。
