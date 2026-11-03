## 问题

既有 Loro 选型缺少真实增量协作的性能证据，结构编辑也没有正式的共享身份、Action payload、并发修复和 actor undo 实现。首轮全历史合并很慢，不能直接据此推断在线协作表现。

## 改动

- 比较 Loro 1.16.0/1.17.1、Yrs 0.27.4/0.28.0，覆盖十万动作吞吐、体积、内存、快照、batch 1/10/100、离线 1k/10k/50k 每端及七类操作分解，保留逐批原始数据与可复现 runner。
- Loro 1.17.1 达到预先约定的继续标准：正式三轮最坏 batch-100 p99 9.270 ms（上限 16 ms），10k/端离线两向合并最坏 105.555 ms（上限 500 ms）；精确锁定该版本，不实现生产 Yrs adapter。
- 新增层 2 `scholium-sdg::Replica`：稳定 NodeId↔TreeID、逐节点文字容器、StructuralEdit/effects 降低、版本化 Action/snapshot、幂等接收、确定性结构修复报告、actor undo/redo，以及保留因果 pending 的快照与重放。
- 修订 ADR 0009，新增 ADR 0053，更新 collab/history/document/model 规范。model 的最小扩展为 NodeId 16-byte 往返及仅 WASM 目标的 UUID 随机源；其余共享文件只追加依赖、层级和索引。相对最新 main 未修改 app/session/storage/build/latex。

## 验证

- 合并最新 main `0648a0e` 后，workspace 645 通过、0 失败、13 项既有忽略；含 adapter 17 项、两个 benchmark example 共 5 项。覆盖四类结构竞争、并发 kind/wrap、远端文字穿插 undo/redo、身份稳定、畸形输入原子拒绝、乱序/重复/pending replay、全部节点形状和固定种子三 actor 随机交错。
- fmt、workspace all-targets clippy、rustdoc `-D warnings`、四个静态脚本、cargo-deny、app/storage binaries、完整 adapter WASM 构建均通过；两个独立 benchmark manifests 的 fmt/clippy/test/doc/deny 均通过。
- 系统字体与 bundled-only stock Typst 对照各 31/31 像素一致，未修改参考和容差。
- 最新 main 全量双语窗口 53/54；唯一英文 sharp-raster 遇 Xvfb SIGKILL，定向复验前两次同样连接中断，第三次 1/1 通过。54 个组合均有通过记录，原始失败全部保留，不将初轮伪报为全通过。没有修改脚本、超时或参考；另保留误用旧输出目录后中止的记录。
- 完整测量边界、命令、原始输出和历史负面结果见开发日志 `docs/log/2026-11-03-loro-sdg-adapter.md` 及本 evidence 目录。

## 待办

- 后续在 `EditorCore::run` / 成功 `apply_joined` 的原子命令边界采用 Replica 单一权威，完整 `Action::encode()` bytes 交给 `ProjectStore::append_action`，durable sequence 确认后发布保存成功，并在已提交序号保存快照、恢复后重放 Actions；失败回滚覆盖 adapter/undo/receipts。
- 本批未接入会话、存储、网络、协作 UI 或历史 DAG；设备 undo 栈不跨重启保存，projection repair 需向用户显示诊断。
- 50k/端离线合并是已知边界，纯结构 wrap/unwrap 可达秒级；长离线同步需工作线程和分批接收。基准不含 adapter 全文规划/预演、投影、存储及渲染，不能外推为产品逐键延迟预算。receipt/pending/payload 有明确上限，尚无 GC/迁移器。
