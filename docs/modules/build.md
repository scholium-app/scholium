# scholium-build

## 职责

根据入口和 profile 规划 Typst/LaTeX/转换构建、准备隔离工作目录、执行受控工具链、取消/超时、收集
artifact、解析日志、缓存结果并为预览/最终输出提供 revision 对应关系。

## 非职责

不修改源文件，不解析文档语义，不下载任意依赖，不让 UI 拼接 shell 命令，不管理协作状态。

## 内部结构

```text
src/
├── profile.rs      declarative BuildProfile
├── planner.rs      entrypoint + dependency graph → steps
├── sandbox.rs      文件、网络、资源与进程边界
├── runner.rs       无 shell 的 argv 执行、取消进程树
├── cache.rs        input/tool/profile hash
├── artifact.rs     PDF/HTML/assets/report
├── diagnostics.rs  工具日志适配器
├── synctex.rs      可选源—PDF 映射
└── export.rs       目标生成后的构建步骤
```

## BuildProfile

声明 entry、toolchain id、目标、允许输入、环境变量白名单、网络策略、超时、内存、输出上限和诊断解析器。
profile 不接受任意 shell 字符串。高级用户命令属于明确的高权限自定义 profile。
运行时工具与挂载清单由受信调用方选择，不能来自文档内容；选择较小清单不得取消进程隔离、
只读输入、输出边界或资源限制。未知清单必须拒绝。

## 构建生命周期

1. 捕获 SDG generation 或所有源码 resource 的 revision/hash。
2. 规划依赖闭包，拒绝未授权根外路径。
3. 创建隔离目录并只读提供输入。
4. 以 argv 启动工具；流式收集有上限的 stdout/stderr。
5. 取消时终止进程树；超限标记明确原因。
6. 验证产物类型/大小，移动到内容寻址缓存。
7. 发布带 input revisions 的结果；项目已变化时标记 stale，不覆盖新预览。

## 缓存

key 包含入口内容、依赖内容 hash、profile、工具链版本和安全策略。失败结果只短期缓存；用户取消不缓存。
缓存是可删除派生物，不能放未物化源文件。

## 公共接口

- `plan(profile, input_snapshot) -> BuildPlan`；`start(plan) -> BuildHandle`；`cancel(build_id)`。
- `BuildProfile { entry, toolchain_id, target, allowed_inputs, env_allowlist, network, timeout, memory, output_limit, diagnostic_parser }`，
  不接受任意 shell 字符串。
- `artifact(build_id) -> Option<ArtifactRef>`，与输入 revision 绑定并放入内容寻址缓存。
- `observe() -> BuildChanged`；`map_diagnostics(tool_output) -> Vec<Diagnostic>`，经 SourceMap 映射回节点或源码位置。
- 后台构建只消费不可变快照，不持有可变文档句柄。

## 不变量

- 不经 shell 解释 argv。
- 默认无网络、无 shell escape、无项目根外读取。
- 旧 revision 构建不能替换当前预览；预览与最终输出使用项目语言的同一引擎，分别记录 artifact 状态。
- 日志截断必须显式标识，不能悄悄丢尾部。
- 持久编译会话必须同时限制单请求等待与会话累计资源，监督进程随应用退出；
  超时/进程退出清空会话派生缓存，不发布半成品，不在 UI 进程内重试编译。
- artifact 展示前验证 MIME/魔数并隔离 active content。
- 宿主读取 worker 产物必须对同一个已打开的普通文件检查类型与大小，并限制实际读取字节；
  不跟随最终符号链接、不阻塞在管道上。场景坐标须在缩窄后的宿主数值类型中仍有限，文本范围须校验溢出和 UTF-8 边界。
- 长驻 worker 的响应队列和短命工具的 stdout/stderr 接收同样有界；文件接收上限不等于工作目录总配额。

## 失败处理

- 工具链缺失、版本不符或平台不支持时返回能力诊断，不伪造产物。
- 超时、内存或输出超限与用户取消都终止完整进程树；取消结果不进入缓存。
- 未授权根外路径、网络访问或 shell escape 请求直接拒绝构建，并给出安全诊断。
- 输入 revision 已过期时结果标记 stale，不替换当前预览。
- 日志被截断必须显式标识；日志行无法映射时保留原位置。
- 缺资源、模板错误或引用不收敛阻止正式输出，并保留上次成功产物。

## 测试门禁

安全 LaTeX/Markdown 构建、shell escape、路径逃逸、超时、内存/输出炸弹、取消进程树、stale result、缓存
失效、日志位置、恶意 HTML/PDF。平台隔离能力不足时该平台不得宣称安全执行不可信项目。

## 项目语言合约

构建沙箱属于共享底座，按固定项目语言运行对应内核的预览与最终构建，二者使用同一引擎。
跨语言只经一次性转换或导出，在不可变快照上由目标工具链验证；失败保留原项目与上次成功产物。
混合构建、跨引擎桥接与团队语言切换已推迟；取消、资源预算、缓存与产物验证要求保留。
共同要求见 [项目语言、跨格式转换与文件操作](../MIXED_SOURCE_EDITING.md) 与 [ADR 0041](../adr/ADR-0041-separate-latex-typst-kernels.md)。

## 工具链候选与宿主

混排载体与跨引擎装配已按 [ADR 0041](../adr/ADR-0041-separate-latex-typst-kernels.md) 推迟；
[ADR 0018](../adr/ADR-0018-vector-fidelity-bridge.md) 及相关阶段 0 报告只作追溯记录。
PDF 解析仍须位于受限 worker，不能在协调器/UI 解析；产物中的注释、动作与链接须验证。

LaTeX 按 TeX Live 所需引擎（pdfLaTeX/XeLaTeX/LuaLaTeX）→ Tectonic 验证，实际兼容矩阵决定后端，不能假定 Tectonic 完全替换全部引擎。参考[Mogan 工具探测](../research/MOGAN_LATEX.md)报告路径、版本及能力。原生流程使用受控进程；浏览器不能调用本机进程，Typst Worker 与本地 LaTeX WASM 分别验证。远程构建需用户显式选择，缺少后端返回能力诊断而非伪造最终产物。见[WASM](../plan/WASM.md)。

## 阶段 0 隔离证据边界

混合构建相关阶段 0 报告只作追溯记录，不恢复已推迟的产品要求。
LaTeX 与 Typst 的隔离证据见 [报告 0017](../spikes/SPK-0017-mixed-build-isolation.md)、
[报告 0018](../spikes/SPK-0018-typst-worker-isolation.md) 与 [ADR 0012](../adr/ADR-0012-typst-worker-isolation.md)。
独立 worker 的临时 JSON 不是正式存储或网络协议。

PDF 产物检查使用受信调用方固定选择的 `pdf-probe` 清单，见
[ADR 0017](../adr/ADR-0017-pdf-probe-runtime.md)：四个 Poppler 工具与字体资源在同一 OS 隔离/预算下执行，
不暴露 TeX 工具树。入口审查见[报告 0033](../spikes/SPK-0033-build-entry-audit.md)。恢复实验的编译与探针
按 [ADR 0019](../adr/ADR-0019-recovery-worker-isolation.md) 使用独立 worker；暂存目录本身不提供隔离。
工具版本查询同样禁用项目配置。跨进程产物签名必须来自稳定序列化的排版结果，不能使用内部对象身份哈希。
输出接收上限不得等同运行时总配额；声明的运行时只读资源是根外访问例外。
Linux 进程树和临时目录资源护栏见 [ADR 0022](../adr/ADR-0022-process-tree-resource-limits.md)：
缺用户 cgroup 管理能力即拒绝启动，嵌套 worker 继承上层总预算；/work 的接收检查不代替运行时磁盘配额。

同语言源码 include 使用本引擎的原生标签空间，编号/页码从最终整编获取；
块级与行内 include 按各引擎已验证的能力处理，未支持内容明确诊断。
阶段 0 双宿主取证与边界见[报告 0034](../spikes/SPK-0034-native-include-fidelity.md)。
