# Spike 0022：槽位端点选区与整节点替换

- 日期：2026-09-17。
- 结论：**Pass（本轮核心与 headless 编辑路径）；真实窗口共同验收未重跑。**
- 阶段 0 的当前判定仍以[报告 0012](SPK-0012-exit-criteria.md) 为准，本报告不升级出口条件 1。

## 实现与失败复现

新增 `core/tests/slot_selection.rs` 后先在旧实现运行：3 项中 2 项失败。
反向槽位选区的 `ordered` 返回原方向；文本到父槽位末尾的复制被 `selection needs text endpoints` 拒绝。

本轮修复：

- 使用从文档根构建的临时位置索引，将每个槽位边界排在相应子树前后；文本位置仍为叶内 UTF-8 字节偏移。
  验证槽位、子节点插入位置、字素边界及根可达性，不接收已脱离子树的后代。
- 文本/槽位混合端点和双槽位端点共用删除规划；完整覆盖的子树（含空数学结构）可摘除，部分覆盖的结构保留。
  覆盖整个数学槽位时至少保留一个可编辑子节点，避免多个兄弟同时删除后留下无光标目标的槽位。
- 纯文本复制沿文档叶子顺序连接原文；不承诺结构化剪贴板或数学格式序列化，空数学结构复制结果是空串。
- 替换规划将删除、必要的新文本叶/段落和输入合为一个 batch。删除后即使只剩槽边界或空文档，也能继续输入。
  规划只允许应用于同一 revision；候选当前为同步本地调用，不新增持久化或协作协议。
- 新文本/段落的 batch 撤销仅在节点为空时移除；后续远端文本使其保留，原被删节点按身份重新挂接。
  没有实现任意结构动作逆、远端并发移动或完整 CRDT 产品适配。
- egui 正文工具栏增加“选择整节点”：对当前焦点节点生成外层槽位选区，复制、剪切、替换沿用核心动作。
  包裹后可直接选择整个包装结构；鼠标命中仍产生文本位置。

## 验证

| 检查 | 结果 |
|---|---|
| core 全部测试 | 44/44，通过；包括新增 6 项槽位回归 |
| egui 全部常规测试 | 30 通过、2 ignored；新增整节点替换/单动作撤销测试 |
| source-reconcile 事务测试 | 3/3，通过 |
| egui 全 targets Clippy `-D warnings` | 通过 |
| egui 格式与本轮 core 文件 rustfmt | 通过 |
| core 完整 Clippy `-D warnings` | 仍被报告 0021 记载的 18 个既有问题阻断 |

新增 6 项核心测试覆盖：反向槽边界/空数学结构删除和身份恢复、文本到父槽末尾、非法槽索引与已脱离后代、
整文档替换后撤销保留远端文本、无远端输入时恢复原树，以及数学槽位多子节点全选后的占位保留。
原跨节点删除和远端插入保留测试也全部通过。

仅为检查新增警告，另以命令行临时放行既有的 `missing_errors_doc`、`missing_panics_doc`、
`needless_range_loop` 和 `filter_next` 跑 core Clippy，通过；**不把它计为完整 Clippy 门禁通过**，未修改仓库 lint 配置。

```bash
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"
cargo test --offline --manifest-path spikes/native-ui/core/Cargo.toml
cargo test --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
cargo test --offline --manifest-path spikes/source-reconcile/Cargo.toml
cargo clippy --offline --all-targets --manifest-path spikes/native-ui/candidate-egui/Cargo.toml -- -D warnings
cargo fmt --manifest-path spikes/native-ui/candidate-egui/Cargo.toml -- --check
```

## 剩余范围

本轮未重跑真实 IME、AT-SPI 或鼠标桌面脚本；新增按钮及槽位端点仍需真实窗口验证。
完整结构外框选中反馈、空结构高亮、槽位光标几何与无障碍选区映射尚未补齐。
长源码输入、完整预览定位、团队/恢复窗口集成及屏幕阅读器验收继续保留。
未重跑安全、混合构建、包重建或实际 GPU 呈现性能；不得据本轮测试关闭其它 P0 出口。
