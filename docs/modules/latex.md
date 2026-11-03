# scholium-latex

## 职责

当前实现源码权威 session 与无 UI 的 live 预览监督。其余解析/转换接口为后续规划：提供容错、无损 LaTeX token/CST，项目 include graph，符号与引用索引，常用论文子集到转换 IR 的投影、
IR 到 LaTeX 的导出生成、语义编辑降级为最小源码 patch、TeX 日志位置解析与实时预览适配。

## 非职责

不实现完整 TeX 展开或排版，不执行宏，不直接运行 TeX，不下载 package，不擅自规范化用户源码。

## 内部结构

```text
src/
├── lexer.rs        控制序列、字符、注释、空白、数学切换
├── parser.rs       group、argument、environment、error recovery
├── cst.rs          lossless nodes/token ranges
├── verbatim.rs     声明式 verbatim 环境识别
├── project.rs      input/include/bibliography/resource graph
├── symbols.rs      label/ref/cite/command/environment 定义与使用
├── semantics.rs    常见命令/环境 capability
├── projection.rs   CST → IR + coverage
├── generate.rs     转换 IR → LaTeX + package requirements
├── session.rs      源码权威、引擎识别、UTF-8/path 校验
├── live.rs         公共异步会话与有界事件订阅
├── supervisor.rs   心跳、重启、文档重同步与故障恢复
├── transport.rs    仅经 build 沙箱启动与能力探测
├── protocol.rs     私有、有界 JSON-lines 适配
├── event.rs        Scholium 自有事件（无 rtex 类型）
├── edit.rs         rename/insert/wrap/math patch
└── log.rs          TeX log → Diagnostic
```

## 解析策略

lexer 不展开 catcode；首发假定常见 LaTeX catcode 语义。遇到可能改变词法规则的命令，标记后续区域为
`lexically_uncertain`，仍保留 token。parser 使用同步点 `}`、环境结束、段落和行边界恢复错误。

未知命令节点保存命令 token 与词法可辨认参数，不推测可选参数数量。`\newcommand` 只索引名称、参数数
和定义范围，默认不展开；白名单纯宏可在独立受限投影器中展开，且保留 provenance 链。

## 语义编辑

- rename label：从 workspace symbol index 获取定义与引用，生成跨文件 patch。
- insert citation：依据当前命令风格生成 `\cite{}` 或配置模板。
- wrap selection：验证组/环境边界后包裹，不跨 verbatim 或不平衡 error node。
- replace math：只在确认的数学范围内替换；保留周围 delimiter 风格。
- add resource：选择相对路径并按项目风格生成引用，不复制文件。

每个 patch 保存 precondition hash；CST 过期即拒绝。

## 项目图

识别 input/include、bibliography/addbibresource、常见图片命令和 documentclass/usepackage。解析路径时保留原
字符串与解析结果。循环边形成诊断但不递归。动态拼接路径标记 unresolved，不能假装缺失或存在。

## 公共接口

已实现的 session/live 合约见下节；以下 CST、projection、generate、log 接口为后续规划。

- `parse(resource, revision, text) -> LatexCst`；`reparse(previous, edits) -> LatexCst`。
- `symbols(cst) -> SymbolIndex`；`dependencies(cst) -> Vec<ResourceDependency>`（input/include、bibliography、
  图片、documentclass/usepackage）。
- `project(cst, selection) -> Projection { ir, coverage, diagnostics }`。
- `generate(ir, profile) -> GeneratedArtifact + PackageRequirements`。
- `lower_edit(cst, SemanticEdit) -> PatchResult`；每个 patch 携带 precondition hash。
- `parse_log(text) -> Vec<Diagnostic>`；预览适配层输出段落更新、页面更新、定位与诊断事件。

## 不变量

- parse 后所有 token 拼接等于输入字节。
- 未修改节点输出直接使用原 source slice。
- error/unknown/uncertain 区域不能执行破坏性语义重构。
- 日志映射失败时保留原日志位置，不猜测到错误文件。
- 构建能力和解析能力分离：能构建不代表能理解。

## 失败处理

- catcode 可能改变词法时标记 `lexically_uncertain`，并拒绝该区域的破坏性语义重构。
- 未知命令保留命令 token 与词法可辨认参数，不推测可选参数数量。
- include 循环、缺失资源、根外路径和动态拼接路径产生诊断；动态路径标记 unresolved，不假装存在或不存在。
- CST 过期或 precondition hash 不匹配时拒绝 patch，重算或进入冲突预览。
- 日志位置无法映射时保留原日志位置，不猜测到其他文件。
- fuzz 输入不得 panic，且 token 拼接仍等于输入字节。

## 测试门禁

真实论文夹具、嵌套组、注释吞换行、escaped percent、数学 delimiter、verbatim、自定义环境、损坏输入、
catcode 警告、include 循环、动态路径、label rename 和 log 映射。fuzz 要求不 panic 且 token 全覆盖。

## 项目语言合约

本模块属于 LaTeX 内核，创建/导入时固定项目语言，以 .tex 源码为权威；预览与最终输出使用同一 LaTeX 引擎。
实时预览适配层采用 LuaLaTeX + 沙箱内的 rtex（[ADR 0043](../adr/ADR-0043-rtex-live-preview.md)），对上只暴露段落更新/页面更新/定位/诊断事件；pdfLaTeX/XeLaTeX 标明整编预览。
引擎与常驻管道须经 build 的 OS 沙箱、运行时白名单及资源预算约束，未隔离不得接入。
跨语言只经转换或导出及逐节点报告；混合组件、跨引擎桥接与团队语言切换已推迟。
共同要求见 [项目语言、跨格式转换与文件操作](../MIXED_SOURCE_EDITING.md) 与 [ADR 0041](../adr/ADR-0041-separate-latex-typst-kernels.md)。

## 候选与参考实现

Rust 容错解析器配 Rowan CST 待验证；Tree-sitter 仅作可选高亮/导航，不承担完整宏语义。纯解析/投影优先兼容 WASM。参考[Mogan 源码调研](../research/MOGAN_LATEX.md)拆分上下文、模板适配、宏包规划、序列化和 source provenance；独立实现，不复制其源码。不确定区域仍遵守本模块保留原文规则。

## 上游跟进

- 实时预览候选锁定于 [经评审的 upstream main commit](../../spikes/rtex-eval/PIN)，只作为 `rtex` OS 沙箱中的独立子进程；不加入 Scholium Rust 依赖图。
  repository、branch main 与精确 commit 由该 pin 记录唯一维护，评估 checkout 和构建留在仓库外。
- 每周一 03:37 UTC，`.github/workflows/rtex-upstream.yml` 调用 `scripts/rtex-upstream-drift.py`；也可手动指定上游 ref。
  它比较 upstream main 与 pin 的精确 commit，报告 pin 之后的提交、锁定构建、未过滤的 cargo-deny 结果，以及 `rtex serve` 启动/编辑/段落更新/布局冒烟。
  只有当前 Ubuntu runner 的真实 bubblewrap namespace 探测通过时，才运行 TeX 冒烟、E8 范围/路径拒绝子集与 E2 外部文件读取对照；
  否则将 TeX 检查标为未运行。此 job 只作外部监测，不在 push/PR 触发、不阻塞 PR、不自动开 issue；表格写入 job summary，原始日志作为 artifact。
- 定时监测不替代本地 E2 全负对照、E3/E5/E6 回归、完整 E8 故障注入或真实窗口 L1 验收。上游变化须按 ADR 0043 复验后才移动 pin。
- `python3 scripts/rtex-pin.py [完整 commit SHA]` 一条命令在仓库外 clone/build、顺序跑 E1/E2/完整 E8，输出 Pass/Fail；无参数取当前 main head。
  仅三项均通过才原子更新 PIN，失败保留旧 pin；不提交或推送。E3/E5/E6、文档评审与仓库门禁仍在提交前执行。
  main pin 用于保留尚未发布的上游合并补丁（math owners、unicode-math 空根指数），并不自动追踪每次 main 变更；构建和 driver 均按完整 commit 取源码。

## 源码会话与实时监督（当前实现）

`SourceSession::open(root, entry)` 保持 `.tex` 为权威，识别前 20 行的 `% !TeX program = ...`，默认 LuaLaTeX；
XeLaTeX/pdfLaTeX 标为 `PreviewMode::FullCompilation`，本 crate 不执行整编。入口与通过 `load` 加载的资源
保存在同一源码会话，`edit` 只修改内存快照并递增宿主 revision，不改磁盘。未保存源码可由 `snapshot` 取回。

`LiveSession::start(source, Arc<Sandbox>, work, LiveConfig)` 消费该权威，只通过 `scholium-build` 沙箱启动
`rtex serve`。公共接口为 edit、snapshot、request_layout、locate、next_event、cancel；私有假 launcher 仅用于测试。
`LiveEvent` 提供 Paragraph、Pages、Location、Diagnostic、Engine，自有 outcome/state，不公开协议 JSON 或 rtex 类型。
Paragraph/Pages 携带宿主 process generation 和 `Option<revision>`；未匹配确认的版本为 None，不能当作当前预览。
本批只传布局元数据，不栅格化或转发 display list；绘制、hayro 回退与整编/导出另行接入。

宿主不变量：

- 发送前检查规范化相对路径：拒绝绝对路径、空组件、`.`/`..`、平台歧义形式；canonicalize 后必须仍在项目内。
  每次编辑/重同步重新检查 symlink；范围使用 UTF-8 byte offset，须有序、不越界且两端落在字符边界。
- 独立 actor 按 interval 发 status，只有有效 status 回复证明心跳。EOF、写失败或心跳截止触发整树取消与有预算重启，
  重启后重新发送所有已加载资源的最新源码并请求布局。宿主 revision 不回退；终止预览失败后仍保留可取回的源码。
- 收到 EngineState Restarting/Failed 立即请求布局，恢复 Ready 时再请求一次，补回缺失的在途段落更新。
  响应心跳的引擎 Failed 不触发宿主重启，缺宏包原因通过事件报告，等待源码修复。
- 启动时在同一沙箱探测 `serve --help`；分别检测 `--compile-timeout-ms`/`--pass-timeout-ms`，仅对支持的参数传值。
  缺能力发诊断并保留上游默认；后台超时/被杀结果保留 Failed。没有宿主单轮编译看门狗。
- 每个 process generation 的 stdout/stderr 共用字节预算（默认 64 MiB），单行默认最多 16 MiB，以容纳既有大布局。
  受限 reader 在解析前检查限额；未知字段/事件可忽略，畸形 JSON/已知字段产生诊断，超限停止进程而不循环重启。
  协议队列和公共事件队列均有界，消费者落后导致显式终止。stderr 单独关闭不等于会话死亡。
- 宿主用 monotonic 时间记录编辑转发到段落更新、layout request/background scheduling 到结果的耗时；这不是 UI 呈现时延或编译 watchdog。

CI 故障注入无需 rtex/TeX/bwrap：私有 tests/support 假进程覆盖崩溃、挂起、丢失更新、引擎 Restarting/Failed、
上游 pass 超时、缺宏包、偏移/路径拒绝、畸形/超长/累计输出与启动失败。ignored 原生测试使用与 build 相同的
三个 `SCHOLIUM_RTEX_*` 配置变量，在本机验证启动—编辑—段落更新、杀/停 serve 及输入拒绝；环境不可用时明确失败，
报告“未运行”及原因。实际测试记录属于开发日志，不作为本节实现依据。
