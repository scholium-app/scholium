# ADR 0050：Linux 常驻构建会话使用 systemd 用户 scope

- 状态：Accepted
- 日期：2026-11-03
- 影响模块：build、latex
- 前序：[ADR 0043](ADR-0043-rtex-live-preview.md)

## 背景

rtex 和多个 LuaLaTeX 子进程构成一个常驻会话。逐进程内存限制不能约束整棵树，累计 CPU 秒数和
整个会话的墙钟上限会杀死正常长期编辑。Linux 用户 cgroup v2 的直接委派并非所有桌面发行版都具备。

## 决策

`scholium-build` 通过无 shell 的 `systemd-run --user --scope` 创建独立命名的瞬态 scope，设置
`MemoryMax`、`MemorySwapMax=0`、`TasksMax`、`OOMPolicy=kill`。默认内存 2 GiB、进程/线程数 64。
不设置常驻会话墙钟上限、`RLIMIT_CPU` 或逐进程地址空间上限。一次性字体预热及能力探测有宿主截止时间；
LaTeX 后台 pass 超时使用上游 `pass_timeout`，宿主只计时，不另建编译看门狗。

scope 内执行固定 bubblewrap profile：项目只读、独立构建目录可写、所有 namespace 隔离、清空环境、
无宿主 HOME、固定 TeX/字体/动态库白名单。库清单通过可信工具链的 `ldd` 解析，不挂载整个 `/usr` 或宿主根。
`prepare` 先在同一 scope/profile 中执行探针；systemd 用户管理器、cgroup 限制或 namespace 不可用时返回明确错误，
没有宿主执行或仅 namespace 的降级路线。

显式取消向 scope 全部成员发送 SIGKILL，再杀宿主进程组并回收直接子进程。Drop 同时触发进程组和 scope 清理，
bubblewrap 的 `--die-with-parent` 与 PID namespace 继续约束退出时的后代。测试 launcher 只存在于 latex 的私有测试代码。

字体缓存使用私有的每用户目录：仅固定受信 fontspec 文档预热，成功后原子发布；文档会话使用独立可写副本，
拒绝缓存内的 symlink 和特殊文件。失败预热不发布缓存；并发预热只接受完整发布的缓存。

## 替代方案

- cgroup v2 直接委派：可减少 systemd 工具调用，但需要桌面宿主提供统一委派入口，留后续平台适配。
- 只用进程组或 RLIMIT：不能提供会话累计内存与进程数边界，不采用。
- 常驻会话使用 `timeout` / CPU 秒数：会终止正常长会话，不采用。

## 后果与验证

Linux 产品路径要求 bubblewrap、systemd 用户管理器和 TeX Live。macOS/Windows 的等价隔离属于后续平台工作，
不声称可用。CI 的监督故障注入使用假 rtex；本机 ignored 测试验证真实 scope、namespace、资源值、进程树取消及 E2。
当前限制包括单文件 64 MiB 与 `/tmp` tmpfs 256 MiB；构建目录累计磁盘配额仍是共享底座的后续工作，不能把
单文件限制称为累计配额。绘制、整编与导出不属于本次内核骨架。
