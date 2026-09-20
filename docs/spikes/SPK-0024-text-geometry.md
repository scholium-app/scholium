# Spike 0024：原生文字与交互几何统一

- 日期：2026-09-17；基线 `2bb2441` 加本轮改动。
- 结论：本轮文字交互路径通过；完整结构排版与阶段 0 尚未关闭。
- 环境：Linux / niri / Wayland，egui 0.36.2，AT-SPI 开启，release 桌面构建。

## 改动范围

左侧文字绘制、光标/IME 锚点、点击命中、文本选区及 AT-SPI 字符框共用 egui galley 的实测几何。
几何使用文档局部逻辑像素，绘制与无障碍发布时再叠加滚动后的窗口原点。
UTF-8 byte offset 映射至 galley 的 Unicode scalar 光标索引；指针最终只接受 core 的字素边界。

按 revision 和屏幕缩放缓存 galley，正文变化或缩放变化后同时失效无障碍缓存；
按 NodeId 索引文本几何，避免发布每个字符框时扫描全部文本叶子。
滚动区域覆盖实际文字外框。替换测试文档时强制重算布局，避免不同文档 revision 相同而误用旧布局。

本轮没有重写 core 的数学结构布局：结构之间的定位、分数线宽仍使用近似度量，
完整结构选区外框、可伸缩根号/括号、多行文本选区仍未完成。
当前几何索引依赖一个文本叶子对应一个图元；未来分行拆图元时需要改为按来源区间检索。

## 验证

- egui 常规测试 32 通过、2 ignored；全 targets Clippy `-D warnings`、格式和 diff 检查通过。
- 新回归覆盖宽窄字符 `Wii` 的实测光标和点击位置、组合字符/ZWJ emoji 的字素边界、
  非法 UTF-8 中间偏移、缓存复用、输入/撤销与屏幕缩放后的缓存更新。
- [桌面首轮](evidence/SPK-0024/results.json)：IME、双源码、数学编辑、AT-SPI、鼠标拖选/滚动、
  Unicode 剪贴板、整节点替换/撤销七组通过。已检查[整节点选区截图](evidence/SPK-0024/slot-selection-highlight.png)。
- 首轮多页预览失败：默认 `/tmp/scholium-typst-toolchain/bin/typst` 已不存在；
  窗口显示 `No such file or directory`，失败证据保留。
- 显式指定已有生成工具包中的 Typst 0.15.1 后，[多页复测](evidence/SPK-0024/preview-recheck/results.json)通过：
  256 段 / 20 页、翻页、缩放及段落定位正常。
- [CPU 采样](evidence/SPK-0024/preview-recheck/frame-cpu.json)：5 组，每组 120 帧，组内 p95 为
  7.546–8.523 ms，最大单帧 19.893 ms。Typst 编译加栅格化 1644 ms。
  CPU 帧耗时覆盖整个窗口，非左侧独立耗时，不含 GPU/合成器；不按固定 15/16 ms 门槛判定。

## 复现

```bash
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"
cargo test --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
cargo clippy --offline --all-targets --manifest-path spikes/native-ui/candidate-egui/Cargo.toml -- -D warnings
cargo build --release --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
SCHOLIUM_TYPST_BIN="$PWD/spikes/mixed-build/out/packages/toolchain/typst" \
SCHOLIUM_SPIKE_BINARY=spikes/native-ui/candidate-egui/target/release/scholium-spike-egui \
  /usr/bin/python3 spikes/native-ui/candidate-egui/scripts/native-edit-acceptance.py /tmp/scholium-text-geometry
```

Typst 路径是本机已有生成产物，干净机器需先准备 0.15.1 工具并指定 `SCHOLIUM_TYPST_BIN`。
未重跑其它 UI 候选、core、安全、团队或包重建测试；本轮未修改这些实现。
下一步将结构布局的文字尺寸接入真实度量，再完善完整结构外框；当前阶段判定见报告 0012。
