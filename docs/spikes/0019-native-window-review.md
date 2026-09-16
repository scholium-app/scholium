# Spike 0019：真实原生窗口复核

- 日期：2026-09-16；基线 `fff6078` 加本轮修复。
- 结论：**Fail（完整共同验收）；已测源码与预览闭环通过。** 阶段 0 条件 1、3 不升级。
- 平台：当前 Arch Linux / niri Wayland 会话，egui/eframe 0.36.2；窗口逻辑尺寸 844 × 1011，截图 1266 × 1517。
- 范围：真实窗口、OS 键盘与剪贴板、AT-SPI 操作/取证；不以 headless 帧时间冒充实际呈现时间。

## 实测与修复

| 项目 | 结果 |
|---|---|
| 窗口存在与显示 | niri 以进程 PID 确认，仅截取该窗口 |
| 源码键盘输入 | 聚焦真实 TextEdit，Ctrl+Home 后键入 CHECK；从 AT-SPI Text 读回确认 |
| 原生剪贴板 | Shift+Left 选 CK，Ctrl+C、右移、Ctrl+V；读回 CHECKCK，未通过程序直接改源码缓冲 |
| 源码应用 | AT-SPI 调用“应用源码”按钮；正文 revision 18 → 19，历史动作 17 → 18，正文读回含 CHECKCK |
| 后台预览 | 调用真实 checkbox；显示 `Some(19) / 正文 19`，首页 PNG 真实呈现；本轮编译/栅格化 1248 ms |
| 窄窗口布局 | 初版预览控件横向溢出；改为三列垂直布局，结构工具栏自动换行。844 宽度回归先失败（内容右边界 991.3）后通过 |
| 失焦后的正文可访问内容 | 初版仅聚焦时发布正文 Label，切源码后正文消失；改为始终发布，真实窗口复测确认 |
| 完整结构编辑无障碍 | **Fail**：正文是 Label，没有 Text/Selection 接口，不能读文本光标/选区或数学语义；对象树存在不等于共同验收通过 |
| 中文 IME | **Blocked（本轮环境）**：`fcitx5-remote` 返回 0，无可用输入上下文；未观察中文 preedit/commit，不否定原报告用户实机结论 |
| 跨节点拖拽删除/剪切 | 本轮没有真实指针操作证据，保留既有 headless 覆盖，不提升结论 |
| 实际呈现 p95 | 未测 GPU/合成器呈现时刻；本轮截图和预览耗时不能关闭 16 ms 帧判据 |

截图仍暴露另一缺口：长结构正文在窄列内裁切，当前临时布局没有换行/横向滚动。
本轮修复的是面板和控件溢出，不是正式结构布局；不能把截图说成完整编辑质量通过。

## 证据与复现

[窗口截图](evidence/0019/window.png)、[AT-SPI 对象与接口](evidence/0019/a11y.json)、
[交互后的状态](evidence/0019/interaction.txt)、[核心事件日志](evidence/0019/app.log)。
键盘输入通过 ydotool，每次注入前核验焦点 PID；按钮通过当前测试应用的 AT-SPI Action 操作。

```bash
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"
cargo build --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
python3 spikes/native-ui/candidate-egui/scripts/window-acceptance.py /tmp/scholium-ui-acceptance
cargo test --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
cargo clippy --offline --all-targets --manifest-path spikes/native-ui/candidate-egui/Cargo.toml -- -D warnings
```

桌面脚本需要 niri、python-atspi、ydotool 服务和 uinput 权限，使用当前会话。
运行时临时打开两个无障碍开关和输入服务，finally 恢复原状态；不会持久启用服务。
脚本只证明上述源码/剪贴板/应用/预览闭环，不是完整 IME 或无障碍门禁。
本轮结束时两个无障碍开关已恢复 false，ydotool 恢复 inactive。

候选非 ignored 测试共 17 个通过（含新增窗口宽度回归），Clippy 全 targets 通过。
未重跑完整阶段 0 脚本；没有改动构建、隔离或核心语义算法。

## 下一步

1. 为正文实现可访问文本、光标、选区及数学结构语义，再重跑 AT-SPI 用户动作。
2. 补结构视口滚动/布局与真实跨节点拖拽、复制/剪切/撤销验证。
3. 在可用 fcitx5 会话补中文合成、取消、焦点切换；不要将自动注入无输入法结果写成框架失败。
4. 添加呈现侧时间证据，再评估大文档后台预览，不用 UI 逻辑时长替代完整帧。
