# Spike 0041：共享输出 tmpfs 原型拒绝接入

- 日期：2026-09-20；基线：9fd93ac。
- 结论：正常路径可行，但超时破坏输出路径，拒绝接入默认沙箱；条件 6 不升级。

为支持长驻 Typst worker 的实时场景文件，试验在外层可信 mount namespace 建立 512 MiB tmpfs，
把宿主输出目录临时重定向到 `/proc/<keeper>/root/mnt`，内层只绑定该 tmpfs 为 /work。
正常退出复制回普通目录；原型通过挂载白名单、临时空间/任务/OOM 对照和连续输入预览。

否决性对照：输入目录、已有 `output/prior`，1 秒墙钟预算，worker 写 `current` 后 sleep 5 秒。
实际结果为退出 124、`output_symlink=True`、`output_exists=False`，原目录仍在 `output.before-<pid>`。
原因是 keeper 与 worker 同受墙钟/cgroup 终止，清理 trap 不保证执行。恢复消费者无法访问预期输出目录。
正常路径成功不能抵消此缺陷，故撤下全部目录替换实现；保留现有 host-bind 与明确未关闭的总磁盘边界。

未来实现必须让工作目录寿命独立于 worker、隔离终止也能由外层监督者回收，且不能靠库析构/trap
保证 SIGKILL 清理。备选为受管有容量上限的文件系统，或全 worker IPC/流式产物传输；均需验证
嵌套重建、长驻场景、取消、OOM、日志文件描述符、旧产物保留和 symlink 攻击后再接入。

本次另外发现 systemd-run 默认展开 argv 中环境变量，会改变受信 shell 脚本的数据语义。
保留独立修复 `--expand-environment=no`，增加沙箱内赋值/读取变量对照，不让外层用户环境改写脚本。
原型未提交为产品入口；9fd93ac 的已验收能力继续保留。

验证：`bash spikes/verify-stage0.sh security` 在 argv 修复后通过 7/7，覆盖不可信输入、
混合编译、Typst worker、PDF 探针、运行时挂载、整树资源限制和恢复编译隔离。
本机日志为 `/tmp/scholium-argv-security.log`，分项日志为 `/tmp/scholium-stage0.NdURtO/`。
