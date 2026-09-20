# ADR 0019：恢复实验的构建与产物探针进入隔离 worker

- 状态：Accepted（Linux 阶段 0 spike）
- 日期：2026-09-19
- 证据：[报告 0036](../spikes/SPK-0036-recovery-worker-isolation.md)
- 前序：[ADR 0012](ADR-0012-typst-worker-isolation.md)

## 决策

旧路径仅隔离目录，无法限制引擎读取或计算。候选为直接复用 xelatex 固定轮次、整个恢复套件外包一层沙箱、
或单独构建 worker。选择后者以保留 latexmk 收敛与宿主 WAL 强杀夹具语义，同时缩小不可信计算边界。

recovery 的公开 build 入口只传递有界源码快照和受限 job name。受信 harness 复制自己的二进制，
通过统一 Linux launcher 启动 worker；不读取项目提供的命令、worker 或沙箱清单。
Typst 编译、SVG 内容签名及 LaTeX 编译/PDF 页数和文本提取均在 worker 内完成，不在父进程降级执行。
主输入只读挂载，既有输出目录单独可写，以保留多轮中间文件与毒饵隔离判据。

保留 latexmk 的多轮逻辑。新增 recovery 清单，在 full 的受信工具/资源上增加 env、perl、latexmk
与 Perl 模块目录；其它清单不扩大。编译和版本查询都传 `-norc`，编译额外禁用 shell escape。
版本查询也可能执行配置，不能视为天然无害。

每次 worker 墙钟 10 秒、地址空间 4 GiB、CPU 30 秒、单文件 64 MiB、文件描述符 128；
网络/PID/挂载命名空间隔离，清空环境，无 capabilities。超时终止沙箱，失败不返回成功产物。
源码上限 8 MiB；父进程读产物前拒绝符号链接、目录等非普通文件、超过 256 个文件或合计 64 MiB 的输出。
请求路径由受信夹具管理，此接口不是任意项目导入 API。

## 签名与取舍

Typst 内部 PagedDocument/Library Hash 包含进程内身份，不能用于独立 worker 的稳定签名。
改用锁定版本的 typst-svg 0.15.1 对编译后的逐页 SVG 求 SHA-256；不以源码哈希冒充排版结果。
typst-svg 为 Apache-2.0，已存在依赖图，只增加直接依赖。Typst 产物仍为 `.out` 文本摘要，不冒充 PDF。

## 保留边界

发布前的总大小检查不是运行时总磁盘配额；临时目录、输出文件数、进程总数尚无硬性总体配额。
运行时整目录只读挂载仍是根外例外。内部 build-worker 必须由 launcher 调用，直接运行它不会自动隔离。
此决策不关闭所有研究 helper、跨平台隔离、原生解析器漏洞或宿主图片解码边界。

后续可替换为生产 BuildProfile/平台 launcher，但必须重跑同一双轮恢复与恶意输入正负对照；
不能以删除旧判据或退回父进程编译来绕开隔离失败。
