# scholium-build

## 职责

提供层级 1、无 UI 的受控工具链执行路径。当前实现 Linux `BuildProfile::Rtex`、`FullCompilation` 与 `TypstPreview`：固定运行时白名单、
bubblewrap 文件/网络隔离、systemd 用户 cgroup、会话进程树取消、受信字体缓存预热与每会话副本。
Typst 编译、LaTeX 整编与 rtex 共用一个 Sandbox 产品执行路径；构建规划、产物缓存与导出属于后续扩展。

## 非职责

不修改项目源码，不解析文档语义，不下载 package，不接受文档提供的工具/挂载清单，不拼接 shell，
不管理协作状态，不实现 LaTeX 单轮编译看门狗。

## 内部结构

- `sandbox.rs`：`BuildProfile`、可信工具链配置、profile 生命周期与执行入口。
- `launch.rs` / `runtime.rs`：所有 profile 共用的 scope 启动、namespace/挂载构造器与固定动态库发现。
- `typst_preview.rs`：固定 Typst 预算、可信 worker/request 配置和隔离探针。
- `process.rs`：stdio 管道、bounded setup 操作、scope/进程树取消与回收。
- `cache.rs`：固定可信文档预热、原子发布、拒绝链接的独立缓存复制。

## 公共接口

- `BuildProfile::Rtex` 标识固定策略；新增工具链以新的 profile 明确扩展。
- `RtexToolchain { binary, tex_directory, font_cache }` 只由受信宿主配置；rtex 按评审 pin 在仓库外构建并随应用打包，由 latex 的发现入口生成此配置。
- `Sandbox::prepare(toolchain, ResourceLimits)` 探测隔离和 cgroup 并完成可信预热，失败返回 `BuildError`。
- `Sandbox::launch(project, work, argv)` 返回 `SandboxProcess`；argv 使用沙箱内路径，没有 shell 解释。
- `Sandbox::run(..., deadline)` 执行有界的一次性命令（含 setup、能力探测与 Typst 预览）；stdout/stderr 各最多 1 MiB。
- `SandboxProcess` 提供一次性获取 stdin/stdout/stderr、宿主 PID 与异步 `cancel`；Drop 也触发整树清理。

## 不变量

- 项目与构建目录必须分离且不重叠。项目只读挂在 `/project`，构建目录可写挂在 `/work`（TeX）或 `/out`（Typst）。
  TeX 的持久字体缓存还须与这两者分离；Typst 无此缓存。
- `--unshare-all --clearenv --cap-drop ALL` 隔离进程、网络与环境；HOME 指向沙箱 `/tmp`，宿主 HOME 不可见。
- TeX profile 仅挂载 rtex、其支持树、prlimit/LuaLaTeX/kpsewhich、解析出的动态库、固定 TeX/fontconfig/font/locale 路径；
  没有 shell，没有整个 `/usr`、`/etc` 或宿主根挂载；项目不能扩展清单。
- TeX 会话 cgroup 默认 `MemoryMax=2G`、`MemorySwapMax=0`、`TasksMax=64`、`OOMPolicy=kill`。
  常驻进程不设墙钟、`RLIMIT_CPU` 或逐进程地址空间限制；单文件限制 64 MiB、沙箱 `/tmp` 限制 256 MiB。
- 可信固定 fontspec 源码预热每用户缓存；成功后才原子发布 `ready`。每个会话取得独立可写副本，
  不把持久缓存挂给文档，不接收文档执行结果回写，不跟随缓存 symlink/特殊文件。
- 取消杀整个 scope 和宿主进程组；清理完成前不释放会话缓存。进程在宿主退出时受 bubblewrap parent/PID 边界约束。
- 后台 pass 超时由上游配置，宿主监督会话心跳与单次操作计时；不把整个会话截止时间当作编译超时。

资源机制的决策见 [ADR 0050](../adr/ADR-0050-linux-live-session-cgroups.md)。累计 `/work` 磁盘配额尚未实现；
单文件限制不代表累计配额。其他平台不提供无隔离降级。

## 失败处理

`prepare` 在同一 cgroup/profile 内运行实际探针；缺少原生工具、用户管理器、资源限制或 namespace 时，
以 `BuildError::Unavailable` 或带原因的 IO 错误报告，绝不在宿主处理文档。可信预热/能力探测有宿主截止时间，
输出超限、非零退出与超时均失败。取消回收失败向调用者报告。协议消息限额由调用方 latex/app 负责，不能无界读取管道。

## 测试门禁

常规测试验证缓存复制隔离和 symlink 拒绝；latex 的 CI 假工具测试验证无原生依赖的监督逻辑。
ignored 原生测试读取 `SCHOLIUM_RTEX_BIN`、`SCHOLIUM_RTEX_TEXDIR`、`SCHOLIUM_RTEX_FONT_CACHE`：
真 rtex + LuaLaTeX + 产品 launcher 执行 E2 文件读取、绝对 input、shell、Lua/openout 写出及网络对照，
每项要求先在宿主生效、再在沙箱阻断；检查实际 cgroup/rlimit 值。shell/网络使用仅测试内部的放大 helper，
产品没有对应环境变量或配置入口。缺工具或沙箱失败必须输出“未运行”及原因，并以失败退出，不计通过。

## 一次性整编 profile

`Sandbox::prepare_full` 不需要 rtex 二进制/支持树，使用相同 namespace/cgroup、可信缓存副本和固定动态库解析。
额外白名单为 XeLaTeX/XeTeX、pdfLaTeX/pdfTeX、xdvipdfmx 以及发行版 libpaper 的 `/etc/paperspecs`、`/etc/papersize`（存在时只读）。
基础 LuaLaTeX 仍用于固定可信字体缓存预热，安装环境需具有这些引擎。
每个进程额外受 60 CPU 秒限制；调用者为完整操作计算剩余 wall-clock deadline，再传 `run`。
非零退出包含有界 stdout/stderr，超时通过 RAII 取消 scope 全树，不允许 shell、网络或项目写入。
资源快照与 revision 淘汰属于 latex，build 不识别 TeX 内容。

## rtex bundle 的构建与分发

先执行 `python3 scripts/bundle-rtex.py`（缓存默认 `~/.cache/scholium-eval/`，输出 `target/rtex-bundle/`）。
`--review` 可指定匹配的 rtex-pin E1 `run.json`；省略时查找已提交证据中的 `*/pin/run.json`，必须匹配 PIN 与 lock hash。
`--cache` 必须在仓库外，`--output` 可指定安装包 staging 位置。脚本对 cache 和目标加排他锁，保留增量构建，
不修改 PIN、不把上游源码/二进制提交进仓库。显式传 Cargo.lock/release/target-dir，固定 Rust 1.99.0，
屏蔽调用环境的 RUSTFLAGS/target/profile 覆盖；manifest 保留真实 rustc/target/profile 及文件 hash，依赖缓存仍由 Cargo 验证。

安装包复制完整目录，包括 `licenses/` 中 rtex 与依赖的许可/notice 及 `dependencies.json`。TeX Live 及其字体/包树不纳入
此 bundle；build 仍只挂载宿主已安装的受信运行时。正式升级 PIN 的流程与 manifest schema 见
[ADR 0043](../adr/ADR-0043-rtex-live-preview.md#修订2026-11-03随应用打包)。

`font_cache_is_warm(root)` 只读 build 自有的当前版本 marker，供 latex 状态检测共享，避免复制 marker 规则。
它不证明 TeX 安装版本变化后的缓存兼容，也不替代 `Sandbox::prepare` 的探针/预热。

## Typst 源码预览 profile

`Sandbox::prepare_typst_preview(executable, request)` 接受受信宿主选择的普通文件，规范化路径，
只发现该 worker 与固定 prlimit 的动态库；不挂载 TeX 运行时、系统字体或字体缓存。
prepare 在同一个 launcher/profile 中运行固定 `prlimit --version` 探针，namespace、用户 scope 或限制不可用即失败。
app 不持有 sandbox 实现，只组装请求、调用 build、检查 revision 与有界页面协议。

`TypstPreview` 固定为 4 GiB MemoryMax/地址空间、64 tasks、零 swap、CPU 30 秒、单文件 64 MiB、128 fd；
`run` 将调用者的 wall deadline 收紧到最多 10 秒。网络通过 `--unshare-all` 关闭，环境清空，
只读挂载 worker、prlimit、动态库、`/request.json` 与 `/project`；唯一可写宿主挂载为独立临时 `/out`，
另有受限 64 MiB `/tmp`。拒绝输出与资源/请求/运行时路径重叠，没有 shell 或宿主降级。

执行复用 `Sandbox::launch/run`、`SandboxProcess` 的进程组/scope 取消和有界 stdout/stderr（各 1 MiB）。
请求/页面协议不由 build 解析；app 与 Typst 内核保持输入 16 MiB、元数据 1 MiB、最多 100 页、
RGBA 总量 64 MiB、单边 8192 px 的检查。单文件限制不代表输出目录累计磁盘配额。
边界见 [ADR 0055](../adr/ADR-0055-source-preview-worker.md)。

常规单元测试无需真实 namespace，验证 Typst 的完整 argv、唯一挂载白名单、截止时间收紧、目录重叠拒绝，
以及 TeX profile 的原缓存/CPU 策略。ignored 原生测试
`typst_preview_enforces_mounts_budgets_and_cancels_descendants` 编译固定受信 fixture，验证真实文件/网络隔离、
cgroup 与 rlimit、stdio 超限、显式整树取消及 10 秒超时；缺少原生前提时失败并注明未运行，不计通过。
