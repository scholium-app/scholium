# Spike 0038：团队源码输入法切换屏障

- 日期：2026-09-19；基线 `43febe0` 加本轮修改。
- 范围：ADR 0016 的单窗口三成员模拟；不扩展真实网络协调协议。
- 结论：限定场景通过；阶段整体仍未关闭，见 [0012](SPK-0012-exit-criteria.md)。

## 行为

源码 TextEdit 持有组合输入时，暂停成员选择、团队控制命令、源码语言选择、应用及重新生成。
预编辑不进入正文/历史；选词完成或取消的事件仍交给原 TextEdit 处理，当前帧保持屏障，下一帧释放。
显式提交方法也拒绝组合中的草稿，防止将预编辑文本发布到正文。

## 验证

- 默认候选测试 **47 passed / 11 ignored**，新增 2 项通过真实 egui 帧注入 Preedit/Commit，
  验证取消、提交、结束帧屏障、正文/历史与 actor/epoch 不变：[tests.log](evidence/SPK-0038/tests.log)。
- all-target Clippy 与修改 Rust 文件格式通过：[clippy.log](evidence/SPK-0038/clippy.log)。无新增依赖。
- 原团队语言门禁窗口回归通过；真实 fcitx5/rime 输入 `nihao`，组合期间向禁用控件发送 AT-SPI 动作，
  成员、语言、草稿及正文均未被绕过修改。取消恢复原草稿；选词后正文仍不变；Bob 不含 Alice 的新输入，
  切回 Alice 保留中文，显式应用后只接受一次：[最终结果](evidence/SPK-0038/desktop-final/results.json)。
- 人工核对组合中与提交后截图：[组合中](evidence/SPK-0038/desktop-first/team-ime.png)、
  [提交后](evidence/SPK-0038/desktop-final/team-ime.png)。

## 失败与边界

首次 headless 驱动未清空 TexturesDelta，触发 egui 析构断言；修正测试驱动后通过。
首次桌面测试依赖 AT-SPI ENABLED 标志而失败：AccessKit 适配器对部分 disabled 按钮角色仍报告 enabled。
最终改为实际发送辅助功能动作并检查状态，不将错误标志当成行为越权，也不声称无障碍状态完整正确。
保留[首次桌面结果](evidence/SPK-0038/desktop-first/results.json)。
这是本机主动切换屏障，不是服务器发起 epoch 变更时的远端 IME drain，也不是三客户端真实 CRDT 协作。
团队草稿持久化、协作撤销、完整读屏、正式项目恢复、大源码桌面规模和安全总资源配额仍需独立验证。
当前环境未发现 Orca 可执行文件；本轮 AT-SPI 操作不能代替完整读屏验收。

```sh
SCHOLIUM_TYPST_BIN=/tmp/scholium-typst-toolchain/bin/typst /usr/bin/python \
  spikes/native-ui/candidate-egui/scripts/native-edit-acceptance.py \
  /tmp/scholium-team-ime team_ime_barrier team_language_gate
```
