# ADR 0022：Linux 构建进程树资源限制

- 状态：Accepted（阶段 0 Linux 实验）
- 日期：2026-09-20；执行负责人：Codex
- 前序：ADR 0008、0012、0017、0019。

> 阶段归属后续由 [ADR 0023](ADR-0023-stage0-feasibility-boundary.md)调整：/work 总配额仍未实现，
> 改为阶段 1 面向不可信项目交付前的必要门禁；下文技术边界与失败证据不变。

prlimit 的地址空间、文件大小与 CPU 限制是单进程/单文件边界，不能约束整个编译进程树。
统一沙箱入口在 bubblewrap 外使用 systemd 用户 transient scope，设置 MemoryMax=2 GiB、
MemorySwapMax=0、TasksMax=64、OOMPolicy=kill；缺少用户管理器或控制器时失败，不退回弱隔离。
保留墙上时限、PID/网络/挂载命名空间、无 capabilities 与清空环境。

临时目录使用 256 MiB tmpfs，总容量由内核限制。嵌套包重建继承外层 cgroup；受信入口在
不可由项目写入的 /run 路径挂只读策略标记，内部不暴露用户 D-Bus 或 systemd-run。
项目环境变量不能豁免隔离；外层与嵌套编译均继续执行原 bubblewrap 清单。
限制以 cgroup 内核文件和多个文件耗尽 tmpfs 的负对照验证，不能只检查命令行参数。

这一步不声称可写 /work 已有总磁盘配额：它仍是调用方目录的绑定挂载。
产物接收时的 64 MiB/文件数检查不能冒充运行时总配额。该项须在有界共享工作目录实现与
长驻编辑 helper 的取消/清理共同验证后关闭，不能为了关闭 P0 而删除此边界。
