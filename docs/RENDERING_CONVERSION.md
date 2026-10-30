# 排版与转换设计

## 1. 两条排版路径

```text
Typst 内核
  Semantic Document Graph (SDG) ── generation ──┐
  .typ lossless CST/source ─────────────────────┴─ Typst 编译 ─ 交互预览 = 最终输出

LaTeX 内核
  .tex lossless CST/source ─┬─ 实时引擎（LuaLaTeX，段落级）── 段落更新
                            └─ 后台整编（同一引擎）─────────── 页面更新 = 最终输出

跨格式（一次性）
  源 CST/SDG ── 转换 IR ── 目标生成器 ── 目标工具链验证 ── 新项目 / 导出
```

项目语言决定走哪条路径（[ADR 0041](adr/ADR-0041-separate-latex-typst-kernels.md)）。两条路径不互相调用：
不把 LaTeX 项目经 SDG 转成 Typst 预览，也不在 Typst 项目里执行 LaTeX。文件与报告合约见
[项目语言、跨格式转换与文件操作](MIXED_SOURCE_EDITING.md)。

SDG 表达 Typst 原生结构项目的稳定语义；CST 保存某种方言的具体写法；SourceMap 连接 SDG NodeId 或源码
范围与编译诊断、预览位置。任何层都不能假装另一层没有表达能力差异。

## 2. 权威模式

### StructuredAuthority（仅 Typst 项目）

SDG 是权威状态。Typst 源码由 generator 产生并保存 generation map。用户编辑生成源码时，系统将新旧 CST
比较并生成 SemanticPatch；接受后更新 SDG，再重新生成源码。不能映射的合法片段变成 RawNode。

### SourceAuthority(format)

指定格式源码是权威状态，CST lossless。Typst 源码项目有 SDG 投影，视觉编辑只对带 reversible capability
的节点启用，并通过 adapter 写最小 source patch。LaTeX 项目首发以源码编辑为主，页面用于定位与诊断；
页面内直接编辑须按 [LaTeX 工作区计划](plan/LATEX_WORKSPACE.md) 另行评估。另一方言的生成结果不能回写
原项目；切换项目语言需迁移或另存为。

## 3. Raw 节点

```text
RawNode {
  dialect,
  source,
  declared_effect: inline | block | preamble | unknown,
  fallback,
  dependencies,
}
```

Raw 节点让未知 LaTeX 宏或 Typst 函数不丢失。相同方言生成时原样输出；跨方言转换时尝试用户配置映射，
否则用 fallback 并产生 Degraded，或阻止导出。Raw 保留不等于支持执行：目标项目不执行源语言。

## 4. LaTeX → Typst 转换

1. lossless parse LaTeX，建立宏、环境、label、cite 和资源索引。
2. 只展开已声明为纯且安全的宏；其余保留 Raw。
3. 将已知节点投影到转换 IR（或直接转为 Typst 原生结构项目的 SDG），记录 provenance 和 coverage。
4. Typst generator 按语义模板生成源码与报告。
5. 在隔离环境用 Typst 构建目标，验证后发布为新项目或导出副本。

## 5. Typst → LaTeX 转换

1. parse Typst 的 markup/math/常用函数调用（或直接读取 SDG）；任意代码与动态计算默认 Raw。
2. 将静态可理解结构投影到转换 IR。
3. LaTeX generator 根据 document class/profile 选择命令、环境和包。
4. 生成 preamble requirements、正文、资源与 ConversionReport。
5. 在隔离 TeX 环境验证最终构建，诊断映射回源节点。

Typst 的动态编程能力无法普遍翻译为 LaTeX，反向同理；不能把运行结果反推为可逆语义树。

## 6. 受支持等级

每个语义节点、每个方向独立声明：

- Exact：目标能保持语义与关键属性。
- Equivalent：语义一致，具体命令或排版算法不同。
- RawPreserved：只在同方言无损保留。
- Degraded：使用可见 fallback，丢失部分属性。
- Dropped：无输出；默认禁止完成转换。

版本化 compatibility matrix 进入测试数据，UI 可在转换前按文档统计。保真度之外记录编辑性；
保留原文不代表目标源码原生可编辑。报告覆盖原始输入及所有依赖。

## 7. 生成稳定性

generator 输出必须确定且尽量局部稳定。每个节点有独立渲染函数与 style context；修改一段不应导致整篇
源码无意义重排。用户选择 formatter profile 后才规范化生成格式。

SourceMap 同时记录 generated range → NodeId 和 NodeId → ranges；生成源码手改后映射进入 dirty 状态，
重新 parse/diff 成功后建立新 generation。

## 8. 预览调度

### Typst 项目

- **预览时延按实测设定，不按帧预算设定。** 阶段 0 实测（[报告 0005](spikes/SPK-0005-typst-mapping.md)，
  20 页 / 257 段，release，复用同一个编译 World）：
  单字符编辑的"重新生成 + 重新编译"合计中位 **19.2 ms**、p95 **21.3 ms**；
  改动首段导致源码整体平移时 p95 达 **207.8 ms**。因此：
  - 编译**必须异步**：帧内只入队与取用最新结果，绝不同步等待；debounce 取百毫秒量级
    （建议 150–250 ms，可按可见页优先进一步优化）；
  - **旧 revision 的结果一律丢弃**（只采纳比已落地更新的 revision），并用负对照测试守住——
    朴素的"谁最后到用谁"必须被同一用例抓出画面倒退；
  - 若要把预览压到帧级，只能做视口局部编译或源码级差分，当前未实现。

### LaTeX 项目

- **LuaLaTeX**：常驻引擎在已加载导言区的状态下重排被编辑的段落/环境，返回该单元的行与字形位置；
  后台整编经去抖后运行至辅助文件收敛，返回整页。宿主绘制最近一次整页结果并叠加段落更新，新整页到达后整体替换。
  无法证明实时结果精确的单元（导言区、目录、依赖页面状态的内容等）只随整编更新，并向 UI 说明原因。
  候选实现为 rtex，选型待评估。
- **pdfLaTeX / XeLaTeX**：没有实时段落路径，后台整编结果即预览，UI 标明“整编预览”及其时延。
- 实时引擎与整编都运行在 OS 沙箱内，与 Typst 编译同等对待不可信输入；超时、崩溃的单元退回整编路径。
- 两种来源都按 revision 采纳，旧结果丢弃；实时结果与整编不一致时以整编为准。

### 共同规则

- 预览缓存 key 包含源码/SDG revision、引擎、模板、资源 hash 和工具链版本。
- 所有编译永不阻塞输入；当前后端、实时/整编来源与是否最新在 UI 中始终可见。

## 9. 诊断

诊断回链到 SDG 节点或源码位置，包括缺失节点、编号、引用目标、目录项、脚注、图片/表格与构建错误。
跨格式转换的差异进入转换报告，不作为同一项目内的双后端比较。

## 10. 测试门禁

- SDG → Typst → SDG 的支持子集语义 round-trip。
- 源码项目局部视觉编辑的未触及字节稳定。
- Raw 同方言无损、跨方言明确降级。
- SourceMap 双向定位、旧 revision 淘汰和生成源码 dirty/reconcile。
- 随机嵌套节点生成后 Typst 不崩溃，错误可定位到 NodeId。
- LaTeX 实时结果与整编逐字形一致；不一致或不可证明的单元退回整编并给出原因。
- LaTeX 实时引擎与整编在沙箱内运行，超时/崩溃不影响编辑。
- Typst ↔ LaTeX 常用论文子集转换后目标构建通过，报告与实际差异一致；缺依赖时阻止导出并保留原项目。
