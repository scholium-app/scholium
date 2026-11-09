# ADR 0058：构建总输出配额与宿主解码边界

- 状态：Accepted
- 日期：2026-11-08

## 问题

单文件 RLIMIT_FSIZE 不能约束许多小文件。常驻 rtex 需要跨编译保留辅助文件；
PDF 的页数和页面尺寸也不能约束压缩流、字体与图片解码的时间和内存。

## 决定

修订（2026-11-08）：所有挂载仅由已有 AppArmor profile 的 bubblewrap 执行。
Ubuntu 24.04+ 的 `kernel.apparmor_restrict_unprivileged_userns=1` 使未获授权的
`unshare` 子 namespace 不具备挂载能力；bwrap 子进程也不能假定保留 CAP_SYS_ADMIN。
因此 helper 不挂载、不进入 namespace，不启动 unshare/nsenter，不要求关闭此内核限制。

每个执行树由 bwrap `--size N --tmpfs /work`（worker 为 `/out`）建立独立 tmpfs，
内核执行总分配字节上限。`ResourceLimits` 默认 512 MiB、32768 个目录项（含根），
包含可写字体缓存副本；允许大型论文的章节辅助文件、字体缓存、日志与多个
64 MiB 内的 PDF 共存，但不是无限制历史保留。零预算拒绝。
目录项预算由 20 ms 监督扫描与复制前检查执行，不声称是内核精确 `nr_inodes` 上限。
采样间新增 inode 的内存、tmpfs 页面和其它会话分配受原有 cgroup
`MemoryMax`（TeX 2 GiB、Typst 4 GiB、PDF 1 GiB）及 `MemorySwapMax=0` 限制。

Linux 官方 [cgroup v2 memory 文档](https://docs.kernel.org/admin-guide/cgroup-v2.html#memory)
明确列出 dentries/inodes 的内核记账；`memory.stat` 的 `file` 包含 tmpfs，
`shmem` 包含 tmpfs 页面，`slab_reclaimable` 包含 dentries/inodes；`memory.max`
是 cgroup 硬内存上限。该文档的 Memory Ownership 还说明按首次分配记账：
由沙箱进程生成的页面和 inode 计入会话，宿主初始化复制的已有文件不宣称计入子 cgroup，
而由可信输入、总输出预算与 tmpfs 字节容量限制。此设计不把 memcg 说成完整无遗漏的记账系统。
原生回归在实际会话 cgroup 读取 `memory.stat`，验证 32 MiB tmpfs 写入与
12000 个空文件分别增加 shmem 与 slab；具体测量见开发日志。

受信 `scholium-sandbox-fs` 只在 bwrap 完成挂载并丢弃能力后运行：
设置 NO_NEW_PRIVS 与不可撤销、跨 exec 继承的 seccomp filter，禁止 mount/umount、
unshare/setns 及新 mount API；检查 native ABI，拒绝 compat ABI。
随后经私有、只读绑定的 Unix socket 用 SCM_RIGHTS 交付输出目录 fd，
等待有界宿主初始化 ACK，关闭 fd/socket 并 exec 固定受信 argv。
helper 随应用安装；不接受项目指定路径或可执行程序，不增加特权工具或宽泛运行时挂载。
原有独立 PID/网络、清空环境、只读项目白名单、能力丢弃、rlimit、scope 取消保持生效。

宿主持有目录 fd，通过自身 `/proc/<pid>/fd/<fd>` 取得 capability 路径，
从而在沙箱退出后仍可读私有卷，无需进入 namespace 或读取受 AppArmor 限制的子进程 root。
rtex 同一次 serve 会话保留同一卷，live PDF 经 capability 目录句柄有界读取。
单次命令结束后杀掉后代，再按字节和目录项双预算复制普通输出到宿主临时目录；
拒绝链接和设备文件，省略 socket/FIFO，递归深度最多 128 层。
后续 pass 恢复这些有界文件。字体缓存副本也在卷内，文档结果不回写持久缓存。

字节空间耗尽由内核拒绝写入；目录项超预算或字节耗尽被采样后终止整个 scope，
宿主报告 `OutputQuota` 和统一 Diagnostic，界面按稳定 code 本地化。
采样延迟可能使目录项数短暂超过配置值，但不能填满宿主磁盘；内存硬边界始终生效。
会话整体内存耗尽仍走既有 OOM/worker 退出诊断，不伪称为精确 inode quota 事件。

PDF 解码采用受信、受限子进程；宿主只接收经过页数、尺寸、总像素与精确帧长度验证的 RGBA。
页数/尺寸校验留在 decoder 内作为提前拒绝；CPU、内存和 wall deadline 约束解压炸弹。
具体宿主边界与 hostile-input 回归清单见同主题开发日志。

## 替代方案

- 只扫描宿主目录并 kill：在采样之间仍可填满宿主磁盘，拒绝。
- unshare helper 挂载 `size,nr_inodes` 后 nsenter：初版采用，现拒绝。
  Ubuntu AppArmor 限制开启时，即使可创建 user namespace，也没有挂载能力，导致所有编译失效。
  为 allocator 添加 AppArmor profile、setuid 或关闭系统限制会增加部署特权，不采用。
- 只用 bwrap sized tmpfs、不限制内存或复制：无法约束 inode/slab 与宿主输出，拒绝。
  采用 sized tmpfs + 原有 memcg + 目录项监督/双预算复制组合。
- 通过 `/proc/<sandbox-pid>/root` 交付 live 文件：依赖子进程寿命和跨 AppArmor ptrace 检查，
  选择一次 SCM_RIGHTS 交付目录 fd，避免退出和进程标识复用竞态。
- 有界宿主可写 bind 目录加监督：采样间仍可写宿主磁盘，现有私有 tmpfs 更直接。
- loop/ext4 或主机 filesystem quota：依赖特权或特殊文件系统，不符合当前非特权 Linux 合约。
- 每轮清空 tmpfs：破坏 rtex 常驻增量状态，拒绝。
- 宿主 hayro 加尺寸检查：不能限制解析/解压期间的资源，拒绝。

## 后果

不再依赖 unshare/nsenter，只保留既有 bwrap、prlimit、systemd 与同版本无挂载 helper。
缺少 namespace、seccomp、cgroup 或 helper 时失败关闭；不提供无隔离降级。
最后一个目录/挂载持有者退出后 tmpfs 回收。host 初始化与复制始终受有界输入/输出检查，
沙箱分配由 memcg 限制。精确内核 inode 数上限改为监测策略，硬内存隔离没有放宽。
