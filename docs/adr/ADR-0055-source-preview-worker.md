# ADR 0055：源码项目的受限 Typst 预览 worker

- 状态：Accepted
- 日期：2026-11-04
- 影响模块：build、app、typst
- 关联：[ADR 0012](ADR-0012-typst-worker-isolation.md)、[ADR 0041](ADR-0041-separate-latex-typst-kernels.md)、[ADR 0052](ADR-0052-latex-view-workbench.md)

## 背景

T46 为普通 .typ 源码项目接入真实预览。原生项目的 PreviewCompiler 只处理应用生成的可信源码；
直接复用它求值外部程序无法执行超时与内存限制。共享构建底座已有 TeX 沙箱，Typst 源码预览通过同一产品路径扩展固定 profile。

## 决策

复用当前锁定的 Typst 编译、World 诊断映射和栅格实现，通过 `scholium-build::BuildProfile::TypstPreview` 调用固定 app worker。
UI 从不求值外部源码，也不接收 PagedDocument；内核接受显式资源快照并只返回诊断与页面数据。
此入口是 T46 允许的窄内核 API 扩展，不引入外部 Typst CLI、第二种引擎或新依赖。

Typst profile 与 rtex、LaTeX 整编复用同一个 Sandbox launcher、挂载构造器、动态库解析、fail-closed 探针和 SandboxProcess 整树取消。
app 只负责 worker 协议、请求组装与结果检查，不启动 bwrap/systemd，也不实现隔离策略。
Linux 宿主用 bwrap 关闭网络并隔离全部命名空间，以 systemd 用户 scope 限制 4 GiB 内存、零 swap 和 64 个任务。
只读挂载当前受信可执行文件、固定 prlimit 及其动态库、输入请求和项目资源；只写临时输出目录。
字体固定为应用打包与 Typst embedded 集合，不挂载用户主目录、系统配置、任意工具或包缓存。
worker 不下载资源，不读取系统时间。缺少所需隔离能力即报不可用；其他平台暂不运行外部源码预览。

每次请求由临时目录隔离，协议版本为 1，绑定源码 revision。输入最多 16 MiB、512 个普通资源；
跳过隐藏目录、构建目录和 symlink，只收集源码、图片、数据、书目与字体扩展名白名单。
World 只能读取该显式 map。源码入口以当前私人草稿覆盖磁盘副本，不修改磁盘文件。
主进程对元数据限 1 MiB，RGBA 总量限 64 MiB，检查尺寸乘法与实际长度；最多 100 页，单边最多 8192 px。

worker 墙钟预算由 build 的 `run` 强制最多 10 秒；地址空间 4 GiB、CPU 30 秒、单文件 64 MiB、128 个 fd。
prepare 在相同 profile 中运行固定 prlimit 探针，缺少隔离能力即拒绝启动。stdout/stderr 各最多 1 MiB，超限或超时通过共享进程所有者清理整树；
临时输出目录是唯一持久可写宿主挂载，RGBA 总量和元数据预算继续由内核及 app 协议检查执行。单文件限制不等同于累计磁盘配额。
去抖 150 ms 后编译；新编辑、关闭或超时取消整个 scope，不在 UI 线程等待求值。
失败时保留上次成功页面，当前诊断绑定失败 revision；旧 revision 的输出不发布。

## 后果

源码项目可使用真实 Typst 语法与项目内依赖，并获得准确诊断。用户字体、包缓存、动态网络和其他平台沙箱另行接入。
每次请求重新启动 worker，暂不保留跨进程编译缓存；共享源码编辑器的增量语法解析与逐行布局缓存不受影响。
预算失败显式可见，不截断输出，也没有进程内 fallback。三个 profile 共享 build 的唯一执行路径；Typst 不挂载 TeX 运行时或字体缓存，不改变既有 TeX 构建接口。
