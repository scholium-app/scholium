# 实施路线图

> 本文件定义**交付与出口条件**（计划）；阶段 0 出口条件的当前判定只见[报告 0012](../spikes/SPK-0012-exit-criteria.md)，
> 时点状态叙述见 [log/stage0-progress.md](../log/stage0-progress.md) 与 [`log/`](../log/README.md) 下的开发日志。

路线按产品风险推进。阶段 0 已验证结构编辑、源码 reconcile、CRDT、构建恢复等否决性风险。依据
[ADR 0041](../adr/ADR-0041-separate-latex-typst-kernels.md)，项目语言固定为 Typst 或 LaTeX，由两个独立内核承担；
LaTeX 内核的实时预览采用沙箱内的 rtex（[ADR 0043](../adr/ADR-0043-rtex-live-preview.md)）。
从阶段 1 起，**两个内核按同一阶段同步推进**：每个阶段同时交付共享底座、Typst 轨与 LaTeX 轨，
三部分都通过才算阶段出口。要求见 [项目语言、跨格式转换与文件操作](../MIXED_SOURCE_EDITING.md)，
LaTeX 轨的细项见 [LaTeX 工作区计划](LATEX_WORKSPACE.md)。

## 两个内核同步推进的规则

1. **同一阶段，同一出口。** 阶段出口由共享底座、Typst 轨、LaTeX 轨三组条件组成，缺一不可；
   不允许一个内核单独宣布阶段完成，也不允许把另一个内核的对应项顺延到下一阶段了事。
2. **共享能力同阶段接入两边。** 存储、源码编辑器、文件命令、诊断、导出、构建沙箱等共享底座能力在引入的同一阶段
   接到两个内核。先接一边时，另一边的接入是同阶段的登记条目，不是“以后再说”。
3. **能力对位。** 每个阶段按下表让两个内核获得对应能力；两边实现方式不同（Typst 结构编辑、LaTeX 源码为主），
   但用户能做的事对齐。

   | 能力 | Typst 轨 | LaTeX 轨 |
   |---|---|---|
   | 编辑与预览（阶段 1） | 页面编辑 + `.typ` 最小 Source Studio，Typst 预览即输出 | 源码工作台 + rtex 实时段落预览 / 整编预览 |
   | 保存、恢复与 PDF（阶段 1） | 原生项目 Action + WAL，`.typ` 原格式保存 | `.tex` 原格式保存与草稿恢复，PDF 与命令行构建一致 |
   | 定位与诊断（阶段 1–2） | 页面 ↔ 源码预览定位、诊断回链 | 页面 ↔ 源码定位（L2）、构建日志诊断回链 |
   | 高效输入与文档结构（阶段 2） | Liii 式结构输入、结构对象、分页编辑面 | `\` 命令/环境补全、label/ref/cite 补全、大纲与多文件项目 |
   | 源码项目双向编辑（阶段 3） | 完整 Source Studio、`.typ` 可逆区域视觉编辑 | 页面内编辑正文与公式（L3） |
   | 历史 / 协作 / 产品化（阶段 4–6） | 结构动作与源码动作 | 源码动作；每位协作者的预览跟随本地 revision |

4. **一边受阻时显式降级。** 某一轨因上游或技术原因受阻（例如 rtex 尚未提供某个能力），先采用已登记的退路
   （如 LaTeX 整编预览），并以 ADR 或计划修订写明本阶段的降级范围；不得因此让另一轨单独出口。
5. **上游跟踪同步。** Typst fork 与 rtex 的锁定版本都有周期性漂移检查；升级各自重跑既定门禁
   （Typst：来源重建与 stock 对照；rtex：E2 沙箱负对照与 E8 故障注入），升级记录写入开发日志。
6. **排期同步。** 每一批实施同时安排两条轨的条目，共享底座优先；批次规格写明本批对另一轨的影响。

## 阶段 0：七项否决性验证（已收尾）

### 交付

1. 原生结构/源码编辑 spike：按 Iced → GPUI → C++ EUI-NEO → Slint 或 egui 依次验证，选定 egui
   （[原生 UI 验证计划](NATIVE_UI_VALIDATION.md)，报告 [0001](../spikes/SPK-0001-native-ui-iced.md)、
   [0002](../spikes/SPK-0002-native-ui-gpui.md)、[0003](../spikes/SPK-0003-native-ui-prescreen.md)、
   [0004](../spikes/SPK-0004-native-ui-egui.md)，[ADR 0006](../adr/ADR-0006-native-ui-framework.md)）。
2. Typst 映射 spike：20 页 SDG 生成 Typst、增量编译、NodeId ↔ glyph/preview 双向定位
   （[报告 0005](../spikes/SPK-0005-typst-mapping.md)）；编辑内核路线见 [Typst 编辑内核计划](TYPST_EDIT_KERNEL.md)，
   限定证据见报告 [0028](../spikes/SPK-0028-typst-editor.md)–[0030](../spikes/SPK-0030-editing-regression.md)、
   [0049](../spikes/SPK-0049-typst-content-layout.md)–[0054](../spikes/SPK-0054-typst-main-app-candidate.md)。
3. 源码 reconcile spike：受支持结构可靠映射回 SDG，未知语法变 Raw（[报告 0006](../spikes/SPK-0006-source-reconcile.md)、
   [0016](../spikes/SPK-0016-working-tree-status.md)）。
4. CRDT 赛马：两人离线编辑、本地 undo、快照与 10 万次动作（[报告 0007](../spikes/SPK-0007-crdt-race.md)）；
   引擎选定 Loro（[报告 0014](../spikes/SPK-0014-crdt-engines.md)、[ADR 0009](../adr/ADR-0009-crdt-engine.md)）。
5. 构建/恢复 spike：两种引擎隔离构建、WAL 与未提交草稿恢复、外部源码修改冲突（[报告 0008](../spikes/SPK-0008-build-recovery.md)）。
6. 团队语言协调 spike（[报告 0009](../spikes/SPK-0009-team-language.md)）。
7. 混合构建 spike（[报告 0010](../spikes/SPK-0010-mixed-build.md)、[0035](../spikes/SPK-0035-vector-fidelity.md)）。

按 ADR 0041，第 6、7 项及其对应出口条件的证据只作追溯，不再约束后续阶段。

### 出口条件

- 中文 IME、嵌套数学光标、结构选区和源码工作台在原生候选中通过共同验收；记录锁定版本及真实平台。
- 构建与运行不依赖 npm/Node.js/WebView；C/C++/Zig 依赖有明确 ABI、生命周期和构建记录，且许可证落入
  `AGENT.md` 许可证政策的允许清单，`cargo deny` 通过。
- Typst 快速预览在后台运行，编译期间编辑与导航可用，旧 revision 不闪回；分别记录 UI CPU 帧耗时、输入响应与编译/栅格化延迟，注明平台、构建模式及文档规模，不以未经验证的固定 15/16 ms 作为通过门槛。
- 受支持 LaTeX/Typst source edit 可 round-trip，Raw 不丢失。
- 两端收敛且本地撤销保留远端输入。
- 不可信 LaTeX/Typst 的正常入口必须运行在 OS 沙箱内，不能任意执行项目命令，不能读取项目输入、工作区与受信运行时白名单之外的宿主文件；白名单由受信代码固定，不由项目扩展（[ADR 0023](../adr/ADR-0023-stage0-feasibility-boundary.md)）。
- 同一共享分支无两种源码语言同时可写窗口；离线及被隔离输入可恢复，旧 epoch 不自动回灌。
- 六类混用内容各有可复现成功夹具及失败夹具；明确支持的模板/工具链/桥接矩阵。
- 两种宿主的最终引用、页码、链接有效；引用反馈有轮数上限，失败阻止正式输出。
- 标准目标包与混合重建包在声明的干净环境可构建，无法支持的组合被准确阻断。

> **以上共 10 条。** 当前逐条判定和最新复测只见[报告 0012](../spikes/SPK-0012-exit-criteria.md)。
> 自动门禁修复历史见[报告 0015](../spikes/SPK-0015-verification-review.md)；历史通过数不替代当前出口判定。

## 阶段 1：两个内核的打开—编辑—预览—保存—输出闭环

应用壳与工作区按 [P1 排版工作区规划](P1_UI.md)及其评审检查实施；菜单、格式栏与快捷键来自统一命令注册表，
界面文字进入可本地化目录（[ArtCraft / PhotoCraft / WordCraft 借鉴计划](ARTCRAFT_REFERENCE.md)、[界面多语言计划](I18N.md)）。
工作区按项目语言装配内核：Typst 项目默认进入页面编辑，LaTeX 项目默认进入源码与预览。

正式实现前置（[ADR 0023](../adr/ADR-0023-stage0-feasibility-boundary.md)，报告 0012 “阶段 1 接手门禁”）：
存储开工前比较 SQLite/rusqlite → redb 并裁决 WAL/快照/内容寻址和永久 schema；源码理解扩展前裁决 parser；
shared SDG adapter 验证树身份、结构并发与 actor undo。本阶段打开外部 `.typ`/`.tex` 项目即是面向不可信输入，
因此 /work 总配额与宿主解码/IPC 安全审查在本阶段内完成。跨平台辅助功能在对应平台交付前验证。

### 共享底座

1. **项目与文件命令**：新建时选择项目语言；打开、最近项目、另存为、导出对两种项目都可用；工作区按语言装配内核，
   两个内核互不调用。
2. **正式存储**：按裁决实现 WAL、快照、原子物化与恢复；原生项目的共享正文写入全部产生 Action，
   源码项目（`.typ`/`.tex`）的未提交草稿有独立持久化记录；外部修改冲突不静默覆盖；实验会话文件迁移后退役。
3. **可写源码编辑器**：编辑、撤销、选区、IME、查找、大文件滚动、诊断标记与语法高亮接口，供 Typst Source Studio
   与 LaTeX 源码工作台共用；高亮与诊断内容由各内核提供。
4. **构建沙箱产品路径**：会话级 cgroup（内存、进程数）、单次编译由宿主计时、固定运行时白名单、/work 总配额；
   每用户字体缓存的可信预热与每会话可写副本。Typst 编译、LaTeX 整编与 rtex 常驻会话共用这一路径。
5. **PDF 导出与诊断**：两种项目共用导出入口与报告；诊断面板接收两个内核的诊断并回链源码位置。
6. **上游跟踪**：Typst 周期漂移检查保留；rtex 建立同类检查，在 v0.0.6 上重跑 E8 后升级锁定版本（ADR 0043 修订）。

### Typst 轨

1. 页面编辑最小集补齐：段落、heading、粗体/强调、行内/独立数学、分数、根式、上下标、定界符；
   TreeCursor、selection、focus、undo；空段落、IME 预编辑与锚点、块边界删除等日常编辑缺陷关闭。
2. shared SDG adapter（Loro）接入编辑会话：树身份、结构并发与 actor undo；所有共享正文写入产生 Action。
3. 预览定位：点击预览定位到正文/源码位置，正文光标在预览中可见。
4. `.typ` 最小 Source Studio：打开、同格式保存、语法错误时仍可保存、外部冲突提示、PDF 输出。

### LaTeX 轨（[LaTeX 工作区计划](LATEX_WORKSPACE.md) L1）

1. `latex` 内核的 session、live、build、view 最小实现：`rtex serve` 子进程在沙箱内，适配层只向上暴露事件。
2. ADR 0043 的宿主兜底：会话心跳与重启、`EngineState Restarting/Failed` 后补发布局、编辑转发前校验、
   后台编译使用上游 `pass_timeout`；每一条都有故障注入测试。
3. 绘制：display list 按不低于 2 倍像素密度栅格化，无法描述的页面（如 TikZ）以 hayro 渲染 PDF 回退。
4. 非 LuaLaTeX 项目以整编结果作预览并标明来源与延迟。

### 出口

- **Typst**：用户不打开源码即可完成一页含中英文和公式的笔记；点击预览定位、光标、IME、保存、强杀恢复全部通过；
  `.typ` 文件打开、同格式保存、语法错误保存、外部冲突与 PDF 输出通过。
- **LaTeX**：打开中英文混排的 LuaLaTeX 论文夹具，编辑正文与公式时预览按段落更新，停止输入后整编更新整页；
  XeLaTeX 夹具显示整编预览并标明来源；沙箱负对照在产品路径上复测通过，E6 在真实窗口内复测。
- **共同**：两种项目在同一应用内新建/打开、保存、强杀恢复、重开后内容与预览一致；PDF 导出与对应命令行构建一致；
  外部修改冲突两边都不静默覆盖。

## 阶段 2：定位、诊断、高效输入与文档结构

### 共享底座

1. 命令补全框架（`\` 命令面板）：输入、筛选、插入与撤销由底座提供，候选由内核提供。
2. 大纲与结构导航面板、文档内搜索与替换。
3. BibTeX 资源：`.bib` 解析、引用候选与缺失条目诊断，供两个内核使用。

### Typst 轨

1. 文本/数学/命令模式、焦点工具栏、Tab 槽位与符号循环、乐高符号、结构变体、结构导航/搜索。
2. 列表、定理、证明、矩阵、多行公式、表格、图片、题注、label/ref/cite、脚注和代码块。
3. 页面编辑面按最终分页显示（[主程序接入计划](TYPST_MAIN_APP_INTEGRATION.md) D 批：flow 分页、增量场景与显示性能）。
4. 文档设置：纸张、页边距、字体、字号、文档语言与样式模板。

### LaTeX 轨（L2 与补全）

1. LaTeX parser 裁决与 `latex` 模块的无损 CST、include graph、符号索引（[模块设计](../modules/latex.md)）。
2. 页面点击跳转源码、源码光标在页面上高亮，误差在 E5 声明范围内。
3. 构建错误与警告进入诊断面板并回链源码；实时路径退回整编的原因可见。
4. 命令与环境补全，label/ref/cite 补全，章节大纲与多文件项目（`\input`、`\include`、`\subfile`）导航。

### 出口

- **Typst**：完成 `EXPERIENCE.md` 中结构输入与导航的验收脚本；不用源码写出含定理、矩阵、图表、交叉引用和文献的短论文；
  每个工具栏操作都可撤销；编辑面分页与输出一致。
- **LaTeX**：在源码工作台写出同等内容的短论文（多文件、含文献），全程可用补全、大纲跳转、页面 ↔ 源码定位与诊断回链；
  E5 定位误差在真实窗口内复测。
- **共同**：补全、大纲、搜索与文献候选在两种项目中由同一套界面提供。

## 阶段 3：源码项目双向编辑与跨格式转换

### 共享底座

完善 format、latex、typst、markdown、bib adapters；实现 LaTeX ↔ Typst 一次性转换与跨格式另存为，
逐节点报告 Exact/Equivalent/RawPreserved/Degraded/Dropped。

### Typst 轨

完整 Source Studio：source lens、reconcile、持久草稿与 Raw；`.typ` 项目的可逆区域可在视觉视图修改。

### LaTeX 轨（L3）

先以新 ADR 裁决页面内编辑的范围（依据 E5 的定位粒度与 rtex 公式源码归属能力），再交付：
页面上直接编辑正文，公式整体或按槽位编辑；编辑结果以最小源码 patch 写回 `.tex`，未触及字节不变。
LuaLaTeX 以外的引擎只提供源码编辑。

### 出口

- 原生项目可在 Typst source lens 修改支持子集并回到视觉视图。
- 现有 `.typ` 项目可原地源码编辑；可逆区域能在视觉视图修改且未触及字节不变。
- 现有 `.tex` 项目可在页面上修改 L3 ADR 声明范围内的正文与公式，源码其余字节不变，预览与命令行构建一致。
- LaTeX ↔ Typst 转换逐节点报告，目标工具链构建通过；另存为创建独立目标项目，不改变当前项目及其语言；
  不可转换内容被准确降级或阻止导出。
- 未覆盖的宏/模板组合有兼容矩阵和错误，不以 Raw 原文保留宣称支持。

## 阶段 4：非线性历史

实现 actor scoped undo/redo、时间线、checkpoint、branch、任意 revert、cherry-pick、merge session、
结构 diff 与语义冲突检测。两个内核共用同一历史模型：Typst 的视觉动作与源码动作、LaTeX 的源码动作
（含 L3 页面内编辑产生的 patch）在同一时间线展示。

出口：远端动作穿插模拟下只撤本人操作；从历史分叉、分别修改、合并后可重启复现同一结果；
无法可靠 revert 的动作明确标记 non-revertible。以上在 Typst 原生项目、`.typ` 项目与 `.tex` 项目上各自通过。

## 阶段 5：团队协作产品化

实现 sync-server、鉴权、角色、邀请、共享树/文本 update、快照、presence、断线队列和撤权后的本地 fork。
同时覆盖两个内核：Typst 原生项目共享树与文本，源码项目（`.typ`/`.tex`）共享文本。项目语言固定，没有语言切换协议。

出口：三端在线/离线交替编辑最终收敛；视觉与源码协作产生 Raw 时双方收到诊断；服务器重启、重复/乱序
消息不丢不重；viewer 无法写。两位源码编辑者和视觉编辑者可协作；LaTeX 项目多人编辑时 CRDT 收敛，
各自的实时预览跟随本地 revision。离线修改重连后不丢输入。

## 阶段 6：扩展导出与产品化

在阶段 3 的源码项目/PDF 输出基础上完成 HTML、Markdown、纯文本导出与 Copy As；门控 DOCX/EPUB；补齐冲突 UI、恢复
中心、无障碍、i18n（完成[界面多语言计划](I18N.md)第 2 期）、更新、三平台打包、诊断包、性能和安全审计。
两个内核同时覆盖三平台：Typst 字体与资源打包；LaTeX 的 TeX 发行版检测与引导（TeX Live、MiKTeX）、
rtex 与构建沙箱在 Windows/macOS 上的等价隔离。

出口：默认策略不存在静默 Dropped；三平台发布候选通过完整门禁，两种项目在每个平台都完成阶段 1 的共同出口；
升级/回滚不损坏语义正文、源码、历史或凭据。

## 阶段 6a：Pi agent 与 MCP

依赖阶段 3 的源码合约、阶段 4 的历史及阶段 5 的权限；按 [组织计划](EXTENSION_ORGANIZATION.md)提取无 UI 依赖的 session。随后通过可选外部进程适配器直接接入 Pi agent，验证会话映射、流式事件、工具桥接、取消、预算和统一错误；不将 Pi 误作 provider 接口。AI 输出只能先形成带 snapshot revision 的 proposal，经预览确认或用户显式配置的项目授权策略，并通过 session 的 Action 校验后写入。两种项目语言都经同一 proposal 路径。

提供可选 MCP server，让其他 agent 通过只读资源和显式 mutation tools 调用 Scholium；stdio 与 Streamable HTTP 都经过同一认证、能力、超时、幂等、审计和权限门禁。MCP/AI 不能获得原始文件、shell、CRDT 或 WAL 权限。交付物、夹具和出口见 [AI 与 MCP 扩展设计](AI_MCP.md)。

## 阶段 6b：WIT/WASM 组件插件

依赖阶段 3–5 的源码、历史与权限合约。以 WebAssembly Component Model + WIT 建立跨平台、跨语言插件 ABI，先支持格式/诊断/转换等无副作用组件，再支持受限任务和 Action proposal。插件通过稳定 snapshot/proposal/task/resource world 与宿主交互，默认拒绝文件、网络和写入能力；桌面宿主优先验证 WASI 能力、资源隔离、fuel/内存/时间上限、取消、崩溃恢复和版本兼容。浏览器组件运行时及 Worker 路径单独探索，不作为桌面首版插件出口。

插件包须携带签名、WIT world hash、权限声明和迁移信息，返回值经过与 AI/MCP 相同的 revision、写集和语义校验。完整市场、远程下载、自动更新和任意网络不属于首个插件出口。设计与验证顺序见 [WASM 插件与 WIT 组件计划](WASM_PLUGINS.md)。

## 依赖关系

```text
否决性验证（阶段 0）
  └─ 阶段 1：两个内核的打开—编辑—预览—保存—输出闭环
       │     共享底座（项目/存储/源码编辑器/沙箱/导出）── Typst 轨 ── LaTeX 轨（L1）
       └─ 阶段 2：定位、诊断、高效输入与文档结构（LaTeX L2）
            ├─ 阶段 3：源码项目双向编辑（LaTeX L3）与跨格式转换 ── 阶段 6 扩展导出与产品化 ── 6a / 6b
            └─ 阶段 4：非线性历史 ── 阶段 5：团队协作
```

每个阶段内部，共享底座先于两条轨的对应条目；两条轨之间没有调用依赖，只有出口上的同步约束。
团队协作还依赖阶段 3 的源码草稿合约；最终产品化依赖两个内核全部验收。

旧的 28–37 周总估算不再适用。ADR 0041 移除了团队语言协调与混合构建两条关键路径，新增 LaTeX 内核；
各阶段按两条轨的实际数据分别估算，当前不承诺新日期。CAS、独立绘图工具仍在范围外。
AI 与插件执行必须在核心编辑、历史、协作和安全边界稳定后按独立 spike 引入；
已有 LaTeX/Typst 绘图代码在其所属语言的项目内受支持，与新增绘图工具不是同一要求。

## 每阶段完成定义

- 两个内核的用户验收脚本都通过，而非只有单元测试或 trait。
- 设计文档和 ADR 与实现一致。
- 公共 API、错误语义、数据版本和迁移测试完整。
- 单元、属性、集成和需要的端到端测试通过。
- 性能、安全、隐私和无障碍影响已记录。
- 用户可观察的失败有诊断与恢复路径。
- 没有用 TODO、假数据、空实现或固定示例冒充出口能力。

## 全栈与 WASM 验证补充

按[全栈候选](TECH_STACK.md)继续 CRDT Loro → Yrs → Automerge、本地存储 SQLite/rusqlite → redb、LaTeX TeX Live → Tectonic 比较
（LaTeX 实时预览见 [LaTeX 工作区计划](LATEX_WORKSPACE.md) 与 ADR 0043），并采用[Mogan 调研](../research/MOGAN_LATEX.md)中的独立转换/模板夹具。

尽早运行原生与 WASM 共享核心测试；浏览器 smoke、Typst Worker、持久化/预览适配按[WASM 计划](WASM.md)逐项报告。选型前必须披露 WASM 阻塞依赖；完整浏览器发行与 LaTeX 本地 WASM 后端仍为待验证范围，不能将网页演示作为原生编辑器验收。
