# Spike 0042：数学槽位真实读屏

- 日期：2026-09-20；基线：17cebba。
- 环境：既有 Linux/niri/egui 0.36.2 候选；Orca 50.2、Speech Dispatcher 0.12.1。
- 结论：标准数学夹具的槽位导航播报通过；条件 1 仍部分满足。

## 问题与实现

旧实现把完整公式描述作为独立 Math 对象名称发布，但正文导航只有普通字符通知。
真实 Orca 回归在进入分子后找不到槽位播报而失败，见 [修复前结果](evidence/SPK-0042/before.json)。
只检查树里存在“分子”不能证明编辑过程可被理解。

新增稳定的 polite live Status 节点，从当前权威树和焦点计算外到内的槽位路径。
覆盖分子/分母、底数/下标/上标、根式、定界结构和矩阵单元格；空内容明确读作“空”。
矩阵槽位插入光标与实际单元格分开命名，不把末尾插入位置当成存在的单元格。
只有正文获得焦点时发布内容，不插入正文 TextRun，不改变系统字符偏移；普通同槽字符移动
没有上下文变化时不重复通知。编辑和撤销后播报直接取当前模型，不依赖编译完成。

## 验证

- 真实窗口 5/5：数学编辑、辅助功能遍历、普通读屏、数学读屏、连续输入，见
  [结果](evidence/SPK-0042/desktop.json)。
- 补空槽/撤销后的数学读屏再次通过，见 [结果](evidence/SPK-0042/empty.json) 和
  [Orca 实际 speech output](evidence/SPK-0042/orca-math-speech.txt)。
- 路径包含分子→分母→分子、包裹根式→进入→输入 Q→撤销、删除 a→空→撤销、
  底数→下标→上标、矩阵第 1→2→3 单元格。读屏启动前聚焦本窗口；键盘注入逐次核验焦点。
- 默认 UI 测试 56 passed / 11 ignored；Clippy all-targets / -D warnings 通过。
  新单元回归校验嵌套槽位路径、空内容和非数学位置；既有结构描述次序回归保留。

```bash
CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" cargo test --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" cargo clippy --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml --all-targets -- -D warnings
SCHOLIUM_SPIKE_BINARY=spikes/native-ui/candidate-egui/target/release/scholium-spike-egui \
SCHOLIUM_TYPST_BIN=/tmp/scholium-typst-toolchain/bin/typst \
/usr/bin/python spikes/native-ui/candidate-egui/scripts/native-edit-acceptance.py /tmp/scholium-math-reader math_screen_reader screen_reader math_edit accessibility continuous_typing
```

## 边界

这是标准夹具和已实现槽位的 Linux 系统播报证据，不代表人工听感验收、任意数学语音语法、
MathML 导航器或 Windows/macOS 读屏兼容。矩阵目前按模型的单元格次序描述，不虚构行列维度。
选区位置仍以正文 Text 接口提供，槽位状态反映活动端点。未引入新依赖或持久化字段。
共享 SDG、正式恢复/schema 与构建总输出资源边界不属于本报告的通过范围。
