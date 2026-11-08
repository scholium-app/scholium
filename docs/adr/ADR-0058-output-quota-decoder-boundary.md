# ADR 0058：构建总输出配额与宿主解码边界

- 状态：Accepted
- 日期：2026-11-08

## 问题

单文件 RLIMIT_FSIZE 不能约束许多小文件。常驻 rtex 需要跨编译保留辅助文件；
PDF 的页数和页面尺寸也不能约束压缩流、字体与图片解码的时间和内存。

## 决定

每个执行树使用独立的 Linux tmpfs，内核同时执行 `size` 和 `nr_inodes`。
`ResourceLimits` 默认 512 MiB、32768 个 inode，包含可写字体缓存副本；
这允许大型论文的章节辅助文件、字体缓存、日志与多个 64 MiB 内的 PDF 共存，
但不是无限制历史保留。调用方可显式调整预算；零预算拒绝。

受信 `scholium-sandbox-fs` 在新的非特权 user/mount namespace 内挂载 tmpfs，
通过 SCM_RIGHTS 交付目录和 namespace fd，然后退出。宿主持有 fd 保持卷存活，
`nsenter` 只进入该私有 namespace，原 bubblewrap 白名单、独立 PID/网络、
清空环境、丢弃能力和 systemd cgroup 继续生效。没有特权挂载、主机挂载修改或无沙箱降级。
安装必须把 allocator 放在应用旁边；CI 的 workspace bins 构建包含它。

文档只写 tmpfs；rtex 的同一次 serve 会话保留同一卷。单次命令结束后杀掉后代，
再按字节和 inode 双预算复制普通输出文件到宿主临时目录，拒绝链接和设备文件，
省略会话 socket/FIFO（不打开、不复制）；
宿主复制深度最多 128 层，防止不可信目录形状驱动无界递归。
后续 pass 在新卷中恢复这些有界文件。live PDF 经 capability 目录句柄有界读取，
不暴露可写宿主输出目录。独立字体缓存副本也在卷内，文档结果不写回持久缓存。

空间/inode 耗尽时内核立即拒绝写入；20 ms 监督采样终止整个 scope，
宿主报告 `OutputQuota` 和无源码位置的统一 Diagnostic；界面按稳定 code 本地化。
文件自身的写入失败仍由引擎报告。采样影响停止延迟，不影响内核硬配额。

PDF 解码采用受信、受限子进程；宿主只接收经过页数、尺寸、总像素与精确帧长度验证的 RGBA。
页数/尺寸校验留在 decoder 内作为提前拒绝；CPU、内存和 wall deadline 约束解压炸弹。
具体宿主边界与 hostile-input 回归清单见同主题开发日志。

## 替代方案

- 只扫描宿主目录并 kill：在采样之间仍可填满宿主磁盘，拒绝。
- 只用 bubblewrap 的 `--size --tmpfs`：未暴露 inode 配额，且 live 输出不易交付，拒绝。
- loop/ext4 或主机 filesystem quota：依赖特权或特殊文件系统，不符合当前非特权 Linux 合约。
- 每轮清空 tmpfs：破坏 rtex 常驻增量状态，拒绝。
- 宿主 hayro 加尺寸检查：不能限制解析/解压期间的资源，拒绝。

## 后果

增加两个 util-linux 工具（unshare、nsenter）与同版本的受信 helper；它们与已有 prlimit 来自同一 Linux 工具栈。
缺少 namespace 或 helper 时失败关闭。tmpfs 由最后一个句柄/namespace 持有者退出而回收，
容量仍受构建 cgroup 的 MemoryMax/零 swap 约束；宿主复制另受输出预算约束。
