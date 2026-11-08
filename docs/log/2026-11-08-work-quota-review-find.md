# T51：总输出配额、宿主边界与共享源码查找

## 问题与变更

构建输出此前只有单文件限制。实现增加了内核执行的 tmpfs byte/inode 双配额，
把常驻状态和字体缓存副本放入同一会话卷，宿主只读取或复制有界普通输出。
裁决见 [ADR 0058](../adr/ADR-0058-output-quota-decoder-boundary.md)。

## 验证记录

Part A 先加入零预算和拷贝预算/链接拒绝回归，确认失败后实现。
真实 tmpfs 超字节与超 inode 对照通过；原 Typst profile 的文件/网络隔离、rlimit/cgroup、
输出超限、整树取消和 wall timeout 原生回归通过。

## 所有权交叉

`full.rs` 只新增 quota 错误到统一诊断的映射；`supervisor.rs` 只新增终止 quota 事件分支。
`typst_source/worker.rs` 只新增基础设施 quota 诊断。整编/导出和 live 调度其它行为归各任务所有。
