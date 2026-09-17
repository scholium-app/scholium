# Spike 0023：结构排版修正与多页桌面复测

- 日期：2026-09-17；基线 `bec69f7` 加本轮改动。
- 环境：Linux / niri 26.04 / Wayland，egui 0.36.2，fcitx5/Rime，AT-SPI 开启。
- 结论：限定桌面流程通过；完整共同验收与阶段 0 尚未关闭。

## 测量口径

左侧是 core 最小结构布局器加 egui 自绘，不调用 Typst。右侧是异步 Typst 编译及页面栅格化结果。
`frame.info().cpu_usage` 测量整个原生窗口的 CPU 帧处理，含三个面板，不是左侧单独耗时，
也不包含 GPU/合成器呈现；它不能代表输入到屏幕显示的完整延迟。

用户本轮明确取消未经验证的固定 15/16 ms 验收门槛。脚本保留原始耗时，只校验采样存在；
预览集成测试继续验证异步完成和当前 revision，不再断言 p95 小于 16 ms。
当前路线图同步修改，旧报告中的固定阈值仅作为历史记录。

## 排版修正

- 数学槽位改为排列全部子节点，修复分数、根式、上下标和定界符只显示首个子节点的问题。
- 根号字形对齐被开方内容基线；负顶部坐标归一化时同步增加布局高度。
- 数学相邻结构增加间距，分数线增加左右留白。
- egui 使用实际字体 galley 的基线放置文字，修复估算 ascent 与 CJK 字体实际度量不一致造成的偏移。

[修正后选区截图](evidence/SPK-0023/updated/slot-selection-highlight.png) 可与
[修正前截图](evidence/SPK-0023/slot-selection-highlight.png) 对比。
仍有限制：字宽、光标和无障碍字符框使用近似度量；选区外框按文本后代推算，未覆盖所有根号笔画；
根号/括号没有按嵌套内容伸缩。此轮不是排版器完成验收，也未将左侧替换为 Typst。

## 验证结果

- core：47 项通过，新增多子节点槽位及根号内分数基线回归。
- egui：30 项通过、2 ignored；全 targets Clippy `-D warnings` 通过。
- 修正前 debug：8 个限定桌面场景通过；原固定 CPU 阈值判 Fail，记录保留在本报告证据根目录。
- 修正后 release：[首轮结果](evidence/SPK-0023/updated/results.json) 中 6 个桌面场景通过，IME 提交超时、整节点场景焦点变化后停止注入。
  [独立复测](evidence/SPK-0023/recheck/results.json) 两项均通过；不删除首轮失败，也不据此宣称自动化已无偶发问题。
- 多页窗口：256 段，20 页，前后翻页、缩放 1.5、定位点跳回正文通过。
- [修正后 CPU 测量](evidence/SPK-0023/updated/frame-cpu.json)：5 组、每组 120 样本，组内 p95 为 7.608–9.126 ms；最大单帧 17.013 ms。
  Typst 编译加栅格化为 2154 ms。二者不相加、不混作一个性能指标。

证据目录中的 `release/` 为布局修正前的 release 对照；`updated/` 为修正后完整运行；
`recheck/` 为两项失败的定向复测。debug 对照期间存在 release 编译负载，不用于精确比较性能倍率。

## 复现

```bash
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"
cargo test --offline --manifest-path spikes/native-ui/core/Cargo.toml
cargo test --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
cargo build --release --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
SCHOLIUM_SPIKE_BINARY=spikes/native-ui/candidate-egui/target/release/scholium-spike-egui \
  /usr/bin/python3 spikes/native-ui/candidate-egui/scripts/native-edit-acceptance.py /tmp/scholium-layout-review
```

仍需真实字体宽度与光标统一、完整结构外框、输入端到端延迟及 GPU 呈现测量。
大源码、团队语言门禁、文件恢复和完整屏幕阅读器验收仍未完成；出口判定见报告 0012。
