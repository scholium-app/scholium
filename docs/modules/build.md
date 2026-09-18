# scholium-build

## 职责

根据入口和 profile 规划 Typst/LaTeX/转换构建、准备隔离工作目录、执行受控工具链、取消/超时、收集
artifact、解析日志、缓存结果并为快速/最终预览提供 revision 对应关系。

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
- `observe() -> BuildChanged`；`map_diagnostics(tool_output) -> Vec<Diagnostic>`，经 SourceMap 映射回节点或组件源位置。
- 后台构建只消费不可变快照，不持有可变文档句柄。

## 不变量

- 不经 shell 解释 argv。
- 默认无网络、无 shell escape、无项目根外读取。
- 旧 revision 构建不能替换当前预览；Typst 快速和 LaTeX 最终 artifact 状态分开。
- 日志截断必须显式标识，不能悄悄丢尾部。
- artifact 展示前验证 MIME/魔数并隔离 active content。

## 失败处理

- 工具链缺失、版本不符或平台不支持时返回能力诊断，不伪造产物。
- 超时、内存或输出超限与用户取消都终止完整进程树；取消结果不进入缓存。
- 未授权根外路径、网络访问或 shell escape 请求直接拒绝构建，并给出安全诊断。
- 输入 revision 已过期时结果标记 stale，不替换当前预览。
- 日志被截断必须显式标识；日志行无法映射时保留原位置。
- 缺组件、模板冲突或引用不收敛阻止正式输出，并保留上次成功产物。

## 测试门禁

安全 LaTeX/Markdown 构建、shell escape、路径逃逸、超时、内存/输出炸弹、取消进程树、stale result、缓存
失效、日志位置、恶意 HTML/PDF。平台隔离能力不足时该平台不得宣称安全执行不可信项目。

## 团队源码语言与混合项目合约

增加 mixed planner、component worker、symbol bridge 与有界多轮宿主排版。计划包含完整组件/模板/字体/资源
快照和工具链版本；独立 Typst/LaTeX 任务可并行，活动编辑语言不限制编译器调度。
模板页面策略由宿主决定；正文/跨页表格、脚注和双向引用通过布局/符号合约处理。最终页码与链接从宿主或装配
结果验证，引用不收敛、缺组件、作用域/模板冲突阻止正式输出。缓存键包含桥接版本与每轮实际输入。
输出标准目标工具链包或双工具链重建包，分别进行干净环境测试。外语原文保留不代替可构建/可编辑验证。
新增混合全范围夹具、轮数/组件总预算、沙箱桥接、半成品发布失败和两后端并行测试。

共同要求见 [混合源码与团队编辑](../MIXED_SOURCE_EDITING.md) 与 [ADR 0001](../adr/ADR-0001-mixed-source-team-editing.md)。

## 工具链候选与宿主

LaTeX 按 TeX Live 所需引擎（pdfLaTeX/XeLaTeX/LuaLaTeX）→ Tectonic 验证，实际兼容矩阵决定后端，不能假定 Tectonic 完全替换全部引擎。参考[Mogan 工具探测](../research/MOGAN_LATEX.md)报告路径、版本及能力。原生流程使用受控进程；浏览器不能调用本机进程，Typst Worker 与本地 LaTeX WASM 分别验证。远程构建需用户显式选择，缺少后端返回能力诊断而非伪造最终产物。见[WASM](../plan/WASM.md)。

## 阶段 0 实验进展

[报告 0017](../spikes/SPK-0017-mixed-build-isolation.md) 验证普通混合构建与包重建的 LaTeX 共用沙箱入口，
输入只读、输出分离，失败不保留当前入口旧 PDF。它是独立 spike，不是本模块的生产实现；
该轮尚未覆盖的 Typst worker 与运行时挂载已由报告 0018 补充；完整资源树和产物探针隔离仍待实现。

当前 mixed-build spike 的 Typst 编译、内省和 PDF/SVG 导出已进入独立沙箱 worker，
见[报告 0018](../spikes/SPK-0018-typst-worker-isolation.md)与[ADR 0012](../adr/ADR-0012-typst-worker-isolation.md)。
每次新进程不保留增量缓存；临时 JSON 不是正式存储或网络协议。
