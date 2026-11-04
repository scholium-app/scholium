# scholium-latex

## 职责

当前实现源码权威 session、无 UI 的 live 预览监督、内核图元栅格化、整编预览与统一日志诊断，app 将其接入项目工作台。其余解析/转换接口为后续规划：提供容错、无损 LaTeX token/CST，项目 include graph，符号与引用索引，常用论文子集到转换 IR 的投影、
IR 到 LaTeX 的导出生成、语义编辑降级为最小源码 patch。

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
├── mailbox.rs      有界事件合并、布局重取标记与生命周期背压
├── toolchain/      bundle 发现/校验、后台状态与版本探测
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

已实现的 session/live 合约见下节；以下 CST、projection、generate 接口为后续规划；日志诊断已实现。

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
  main pin 用于保留尚未发布的上游合并补丁（math owners、unicode-math 空根指数、快照文件链接及 serve 超时参数），并不自动追踪每次 main 变更；构建和 driver 均按完整 commit 取源码。

## 源码会话与实时监督（当前实现）

`SourceSession::open(root, entry)` 保持 `.tex` 为权威，识别前 20 行的 `% !TeX program = ...`，默认 LuaLaTeX；
XeLaTeX/pdfLaTeX 使用 `PreviewMode::FullCompilation` 与 `full::compile`。入口与通过 `load` 加载的资源
保存在同一源码会话，`edit` 只修改内存快照并递增宿主 revision，不改磁盘。未保存源码可由 `snapshot` 取回。
app 工作台以存储层 SourceProject 为唯一可写权威；入口来自项目声明，编辑保存 draft，显式 Save 才物化源文件。
本模块的 SourceSession 是派生预览快照：重开时先载入声明入口，再应用恢复的 draft；宿主初始 revision 与 UI revision
显式映射，恢复草稿也能采纳初始预览。存储错误、外部冲突与文件操作由 app 的既有项目文件 UI 处理，
重新载入/另存为时重建预览会话，不能用旧页面冒充新项目的最新结果。

`LiveSession::start(source, Arc<Sandbox>, work, LiveConfig)` 消费该权威，只通过 `scholium-build` 沙箱启动
`rtex serve`。公共接口为 edit、snapshot、diagnostics、publish_full_diagnostics、request_layout、locate、next_event、cancel；私有假 launcher 仅用于测试。
`LiveEvent` 提供 Paragraph、Pages、LayoutRequired、Location、Diagnostic、Engine，自有 outcome/state，不公开协议 JSON 或 rtex 类型。
Paragraph/Pages 携带宿主 process generation 和 `Option<revision>`；未匹配确认的版本为 None，不能当作当前预览。
Paragraph/Pages 携带内核自有不透明 `Drawing`，由 raster worker 的 `Renderer` 消费；不公开上游 JSON。

宿主不变量：

- 发送前检查规范化相对路径：拒绝绝对路径、空组件、`.`/`..`、平台歧义形式；canonicalize 后必须仍在项目内。
  每次编辑/重同步重新检查 symlink；范围使用 UTF-8 byte offset，须有序、不越界且两端落在字符边界。
- 独立 actor 按 interval 发 status，只有有效 status 回复证明心跳。EOF、写失败或心跳截止触发整树取消与有预算重启，
  重启后重新发送所有已加载资源的最新源码并请求布局。有效心跳连续健康达到 `restart_stability_window`（默认 30 秒）后，
  清零重启次数；每次重启重新计健康期，短时间故障仍受 `max_restarts`（默认 3）约束。宿主 revision 不回退；终止预览失败后仍保留可取回的源码。
- 收到 EngineState Restarting/Failed 立即请求布局，恢复 Ready 时再请求一次，补回缺失的在途段落更新。
  响应心跳的引擎 Failed 不触发宿主重启，缺宏包原因通过事件报告，等待源码修复。
- 启动时在同一沙箱探测 `serve --help`；分别检测 `--compile-timeout-ms`/`--pass-timeout-ms`，仅对支持的参数传值。
  当前 pin 已支持两个参数，无需库配置退路；旧二进制缺能力时仍发诊断并保留上游默认。后台超时/被杀结果保留 Failed。没有宿主单轮编译看门狗。
- 每个 process generation 的 stdout/stderr 共用字节预算（默认 64 MiB），单行默认最多 16 MiB，以容纳既有大布局。
  受限 reader 在解析前检查限额；未知字段/事件可忽略，畸形 JSON/已知字段产生诊断，超限停止进程而不循环重启。
  协议队列与公共事件队列均有界。公共队列最多 32 条事件、一个合并的 `LayoutRequired` 标记和一条溢出汇总诊断：
  同 generation 的 Pages 只保留最新，同 generation、同段落的 Paragraph 只保留最新；溢出时淘汰非 Engine 事件，
  淘汰预览更新则标记需要调用 `request_layout` 补齐。Engine 状态与原因按序保留；队列全为 Engine 时等待消费者腾出空间，
  等待时间不计入心跳截止或健康期，不因消费落后永久失败。此时 actor 的命令处理也会等待消费恢复。
  Diagnostic/Location 淘汰数用饱和计数器累计，队列排空后输出各自丢失条数及重新查询提示，不能静默丢失。stderr 单独关闭不等于会话死亡。
- 宿主用 monotonic 时间记录编辑转发到段落更新、layout request/background scheduling 到结果的耗时；这不是 UI 呈现时延或编译 watchdog。

CI 故障注入无需 rtex/TeX/bwrap：私有 tests/support 假进程覆盖崩溃、挂起、丢失更新、引擎 Restarting/Failed、
上游 pass 超时、缺宏包、偏移/路径拒绝、畸形/超长/累计输出与启动失败。ignored 原生测试使用与 build 相同的
三个 `SCHOLIUM_RTEX_*` 配置变量，在本机验证启动—编辑—段落更新、杀/停 serve、后台超时后源码修复恢复、快照链接跨挂载可读且不可写回项目，以及输入拒绝；环境不可用时明确失败，
报告“未运行”及原因。实际测试记录属于开发日志，不作为本节实现依据。

## 图元与 PDF 预览

`view::Drawing` 在监督层解码，页面按 generation 累积完整图元，之后才进入可合并 mailbox，避免空增量覆盖前一页。
`view::Renderer` 在独立栅格线程保存页面与段落 placements，以横坐标和基线共同识别旧行；失败更新回滚场景并请求完整布局。
栅格为 300 dpi（300/72 px/bp），scaled point 通过 65536×72.27/72 换算到 bp。
OpenType 字形经 skrifa；TFM 通过固定宿主 kpsewhich 的 pdftex.map/encoding 解析 Type 1。
字体路径须 canonicalize 到受信系统字体/TeX 树，单文件 ≤64 MiB，face/glyph 缓存条数有界。
未知图元、degraded 页面、缺失字形或图片使用当前 layout PDF 经 hayro 0.8 渲染；缺当前 PDF 时保留上次预览并请求布局。
单页 ≤2400 万像素，场景 ≤9600 万像素，PDF ≤64 MiB、≤256 页；超限失败，不截断为正常预览。

`full::compile` 接收不可变 SourceSession，在独立临时目录复制正规资源（拒绝 symlink/特殊文件，≤1 万项、64 MiB），
覆盖已加载资源的未保存文本，再通过 `BuildProfile::FullCompilation` 编译。pdfLaTeX 直接输出 PDF；
XeLaTeX 使用 `-no-pdf` 输出 XDV，再显式执行 xdvipdfmx，避免 shell。最多两轮，所有子进程共用宿主 deadline，
栅格完成后也检查 deadline。结果携带 source revision、固定引擎、整编来源和耗时，旧 revision 不采纳。
此接口生成预览，不写回源码、不实现导出/文件菜单或 bibliography 工具链。

`highlight` 返回 UTF-8 byte ranges 与控制序列、注释、数学切换、花括号、普通文本类别；不展开宏或声称 CST 语义。
绘制边界与回退裁决见 [ADR 0052](../adr/ADR-0052-latex-view-workbench.md)。

## 随应用打包与工具链状态

`toolchain::discover().await` 在 blocking worker 中寻找并校验与编译期 PIN 匹配的完整 bundle，返回
`RtexToolchain` 或 `Issue`；app 的 LaTeX worker 使用此入口，用户无需设置 rtex 路径。布局、manifest 与许可见
[ADR 0043 的打包修订](../adr/ADR-0043-rtex-live-preview.md#修订2026-11-03随应用打包)。
开发先运行 `python3 scripts/bundle-rtex.py`，输出默认是 `target/rtex-bundle/`；发现依据可执行文件，不依据 CWD。

`ToolchainDetector::new(DetectionConfig)` 立即启动专用后台线程；`cached()` 只读上次不可变 `Arc<ToolchainStatus>`，
首次检测前为 None；`refresh()` 合并待处理请求，重检期间保留旧结果，完成后原子替换。线程不包含 UI 对象；
宿主在关闭时 drop detector，活动检测结束后退出。bundle 检查等待最多 3 秒，六个版本探测并行且每个最多 2 秒，
每路 stdout/stderr 最多 16 KiB，整个检测最多等待 6 秒；文件系统查询也在线程池中执行，避免阻塞总 deadline。
本地版本探测不编译文档、不预热字体、不下载资源。超时返回可重检问题。
路径查找仅使用捕获的 PATH 中绝对目录，忽略相对路径；`kpsewhich --version` 识别 kpathsea/MiKTeX，结合引擎输出识别 TeX Live/MacTeX/MiKTeX，
版本保留发行版行或首行，最多 512 字符。检测状态不保证当前 Linux 固定 `/usr/bin` 沙箱 profile 支持自定义安装位置。

`ToolchainStatus` 包含：bundle 的 present/valid/commit/version/directory；是否启用开发覆盖；TeX 发行版 found/version/
kpsewhich；lualatex、xelatex、pdflatex、biber、bibtex 的路径和版本；build 受信 marker 的 font_cache_warm；issues。
未响应的工具保留已找到的路径，version 为 None。字体冷缓存是可执行提示，不等于错误，预览时自动受信预热。
`DetectionConfig::from_environment()` 捕获真实配置；`for_executable()` 为无全局环境修改的测试/宿主注入入口。
`cargo run --locked -p scholium-latex --example toolchain-status` 可输出同一 API 的 JSON，便于诊断。

所有问题及动作均为 `Message { id, arguments }`，由 app 本地化；原始版本、路径与工具名仅作为数据。
稳定键及含义：

| message id（前缀 `toolchain.`） | action id（同前缀） | 含义/参数 |
|---|---|---|
| bundle-missing / bundle-invalid | reinstall | rtex 包缺失/损坏，重新安装应用；开发环境重新打包 |
| bundle-pin-mismatch | reinstall | 包与应用 PIN 不匹配，expected/actual 为 commit |
| bundle-check-timeout | refresh | 包校验超时，重新检测 |
| executable-unavailable | reinstall | 无法定位应用可执行文件 |
| detection-failed | refresh | 后台检测失败，重新检测 |
| tex-missing | install-tex | 未发现发行版，安装 TeX Live / MacTeX / MiKTeX 并检查 PATH |
| program-unavailable | install-tex-component | 工具缺失或版本命令失败/超时，program 为工具名 |
| font-cache-cold | warm-on-preview | 字体缓存尚未预热，首次预览自动准备 |
| dev-override | remove-dev-override | 已启用仅供测试/开发的路径覆盖 |
| override-invalid | fix-dev-override | 开发覆盖不完整或对应文件缺失 |

CI 的假 bundle/假版本程序测试不需要 TeX。常规窗口检查显式使用不存在的开发覆盖来稳定验证不可用状态；
本机 `SCHOLIUM_LATEX_NATIVE_WINDOW=1` 的额外真实窗口验收使用正常发现路径。新 UI 消费者负责把上述消息键加入其目录。

## 统一编译诊断

`log::parse_log` 返回 `scholium_model::diagnostic::Diagnostic`：括号文件栈识别相对文件（含引号路径）与
明确的 `/project/` 沙箱挂载；其它绝对路径或无法识别的上下文不映射到父文件。支持 `!` 错误与后续 `l.N`、
LaTeX/宏包 warning/error、换行警告中的 `on input line N`、未定义引用/引文及 overfull/underfull 信息。
日志 header 的原始行号和 byte range 始终保留；TeX 未提供列时使用 1，未提供 end/bytes 时留空。
解析是容错的，未识别日志保持在完整 transcript 中，不通过推测创建源码位置。

`full::compile` 成功返回 `FullPreview.diagnostics`（最后一轮完整日志的版本化结果）；TeX 返回非零且留下可解析
日志时返回 `LatexError::Compilation(Box<FullFailure>)`，其中仍有 revision、完整 log 和 diagnostics。
沙箱/超时/资源错误继续按原错误分类返回，不能伪装成编译诊断。失败不生成新预览，调用方保留旧页。

`LiveSession::diagnostics()` 查询当前 host revision 的完整问题列表，不依赖 UI 是否及时消费事件。
`publish_full_diagnostics(batch)` 合并同版本独立整编结果，旧或未来版本返回 false；每个来源替换而非累积，空组清除，
源码版本前进时清除旧版本来源。相同条目合并去重。rtex 类型仍只存在于私有协议适配器。
锁定 rtex 的 `background` Diagnostics 不带 revision，先暂存并与随后 LayoutUpdate 的 source_revision 配对；
没有 acknowledgement 映射或已过期的结果丢弃。带版本的诊断事件按已确认映射接入。
无版本的 fast/standby 引擎故障只保留在 supervision 消息中，不能猜测成当前源码诊断。
重启时丢弃尚未配对的旧进程诊断。源码和诊断在离线 actor 中仍可查询与保存。

CI 使用 fake-rtex 验证事件→适配→当前列表、整编合并、修复清除和旧版本拒绝；真实 LuaLaTeX/XeLaTeX transcript
夹具与任意 Unicode 输入检查覆盖日志解析。`real::native_undefined_control_sequence_reaches_revision_bound_diagnostics`
及 `diagnostics_native` 是显式 ignored 的真实沙箱验收，必须在具备工具链的宿主另行运行。
