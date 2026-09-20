# Spike 0037：恢复隔离改动验收

- 日期：2026-09-19；验收基线 `b427565`，含本轮修复。
- 范围：[报告 0036](SPK-0036-recovery-worker-isolation.md) 的 recovery 公开构建入口、恢复判据及安全护栏。
- 不包含原生窗口综合验收；阶段判定仍以 [0012](SPK-0012-exit-criteria.md) 为准。

## 发现与修复

新增真实 LaTeX 夹具生成 260 个文件，触发 256 文件接收上限。基线明确返回错误，
但 `main.pdf` 留在输出目录：输出验证的 `?` 提前返回，绕过只在非零 worker 状态下执行的清理。
复现见 [output-rejection-first.log](evidence/SPK-0037/output-rejection-first.log)。

改为统一收集启动、退出与产物验证结果；返回错误时删除当前入口产物。
清理只 unlink 该路径，不跟随符号链接，不删除其它作业输出。清理本身失败仍报告错误。
补强双引擎超时夹具：每次攻击前重新成功构建并断言产物存在，避免此前其它拒绝测试已删除产物而造成假阳性。

## 复测

| 内容 | 结果 | 原始记录 |
|---|---|---|
| 恢复完整判据 | 两轮各 15/15，逐用例一致 | [recovery.log](evidence/SPK-0037/recovery.log) |
| 构建安全 | 8/8，新增超量输出拒绝/清理 | [security.log](evidence/SPK-0037/security.log) |
| 护栏单测 | 3/3 | [unit.log](evidence/SPK-0037/unit.log) |
| 清单与 runner 自测 | 通过 | [mounts.log](evidence/SPK-0037/mounts.log)、[runner.log](evidence/SPK-0037/runner.log) |
| 格式与 all-target Clippy | 通过 | [clippy.log](evidence/SPK-0037/clippy.log) |

复现命令沿用报告 0036。本轮没有新增依赖或变更运行时挂载。
超量输出仍是运行结束后的拒绝，不是执行期间总磁盘配额；资源树例外、独立研究 helper、原生解析与宿主解码边界继续保留。
