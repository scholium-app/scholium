# scholium-build

## 职责

提供层级 1、无 UI 的受控工具链执行路径。当前实现 Linux `BuildProfile::Rtex`：固定运行时白名单、
bubblewrap 文件/网络隔离、systemd 用户 cgroup、会话进程树取消、受信字体缓存预热与每会话副本。
Typst 接入、构建规划、产物缓存、整编与导出属于后续扩展。

## 非职责

不修改项目源码，不解析文档语义，不下载 package，不接受文档提供的工具/挂载清单，不拼接 shell，
不管理协作状态，不实现 LaTeX 单轮编译看门狗。

## 内部结构

- `sandbox.rs`：`BuildProfile`、可信工具链配置、固定 runtime、隔离探针与 argv 启动。
- `process.rs`：stdio 管道、bounded setup 操作、scope/进程树取消与回收。
- `cache.rs`：固定可信文档预热、原子发布、拒绝链接的独立缓存复制。

## 公共接口

- `BuildProfile::Rtex` 标识固定策略；新增工具链以新的 profile 明确扩展。
- `RtexToolchain { binary, tex_directory, font_cache }` 只由受信宿主配置；rtex 按评审 pin 在仓库外构建。
- `Sandbox::prepare(toolchain, ResourceLimits)` 探测隔离和 cgroup 并完成可信预热，失败返回 `BuildError`。
- `Sandbox::launch(project, work, argv)` 返回 `SandboxProcess`；argv 使用沙箱内路径，没有 shell 解释。
- `Sandbox::run(..., deadline)` 只执行有界的 setup/能力探测命令；stdout/stderr 各最多 1 MiB。
- `SandboxProcess` 提供一次性获取 stdin/stdout/stderr、宿主 PID 与异步 `cancel`；Drop 也触发整树清理。

## 不变量

- 项目、构建目录、持久字体缓存必须分离且不重叠。项目只读挂在 `/project`，构建目录可写挂在 `/work`。
- `--unshare-all --clearenv --cap-drop ALL` 隔离进程、网络与环境；HOME 指向沙箱 `/tmp`，宿主 HOME 不可见。
- 仅挂载 rtex、其支持树、prlimit/LuaLaTeX/kpsewhich、解析出的动态库、固定 TeX/fontconfig/font/locale 路径；
  没有 shell，没有整个 `/usr`、`/etc` 或宿主根挂载；项目不能扩展清单。
- 会话 cgroup 默认 `MemoryMax=2G`、`MemorySwapMax=0`、`TasksMax=64`、`OOMPolicy=kill`。
  常驻进程不设墙钟、`RLIMIT_CPU` 或逐进程地址空间限制；单文件限制 64 MiB、沙箱 `/tmp` 限制 256 MiB。
- 可信固定 fontspec 源码预热每用户缓存；成功后才原子发布 `ready`。每个会话取得独立可写副本，
  不把持久缓存挂给文档，不接收文档执行结果回写，不跟随缓存 symlink/特殊文件。
- 取消杀整个 scope 和宿主进程组；清理完成前不释放会话缓存。进程在宿主退出时受 bubblewrap parent/PID 边界约束。
- 后台 pass 超时由上游配置，宿主监督会话心跳与单次操作计时；不把整个会话截止时间当作编译超时。

资源机制的决策见 [ADR 0050](../adr/ADR-0050-linux-live-session-cgroups.md)。累计 `/work` 磁盘配额尚未实现；
单文件限制不代表累计配额。其他平台与 Typst profile 不提供无隔离降级。

## 失败处理

`prepare` 在同一 cgroup/profile 内运行实际探针；缺少原生工具、用户管理器、资源限制或 namespace 时，
以 `BuildError::Unavailable` 或带原因的 IO 错误报告，绝不在宿主处理文档。可信预热/能力探测有宿主截止时间，
输出超限、非零退出与超时均失败。取消回收失败向调用者报告。协议消息限额由调用方 latex 负责，不能无界读取管道。

## 测试门禁

常规测试验证缓存复制隔离和 symlink 拒绝；latex 的 CI 假工具测试验证无原生依赖的监督逻辑。
ignored 原生测试读取 `SCHOLIUM_RTEX_BIN`、`SCHOLIUM_RTEX_TEXDIR`、`SCHOLIUM_RTEX_FONT_CACHE`：
真 rtex + LuaLaTeX + 产品 launcher 执行 E2 文件读取、绝对 input、shell、Lua/openout 写出及网络对照，
每项要求先在宿主生效、再在沙箱阻断；检查实际 cgroup/rlimit 值。shell/网络使用仅测试内部的放大 helper，
产品没有对应环境变量或配置入口。缺工具或沙箱失败必须输出“未运行”及原因，并以失败退出，不计通过。
