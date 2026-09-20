# 排版与双向转换设计

## 1. 四层表示

```text
Semantic Document Graph (SDG)
        │ projection/generation
        ├── LaTeX lossless CST/source ── TeX build ── final PDF
        ├── Typst lossless CST/source ── Typst build ─ fast/final preview
        └── Markdown/HTML/other targets
```

以上是纯目标路径。混合项目增加 ForeignSource 组件构建与宿主装配路径；项目的源文件集合可以包含两种语言，
但团队活动源码语言仍唯一。编辑语言、默认预览后端和最终输出目标相互独立，限制编辑不限制编译并发。
完整合约见 [混合源码与团队编辑](MIXED_SOURCE_EDITING.md)。

SDG 表达跨格式稳定语义；CST 保存某种方言的具体写法；SourceMap 连接 SDG NodeId 与生成源码范围、编译
诊断和预览位置。任何层都不能假装另一层没有表达能力差异。

## 2. 权威模式

### StructuredAuthority

SDG 是权威状态。LaTeX/Typst 源码由 generator 产生并保存 generation map。用户编辑生成源码时，系统将
新旧 CST 比较并生成 SemanticPatch；接受后更新 SDG，再重新生成源码。不能映射的合法片段变成 RawNode。

### SourceAuthority(format)

指定格式源码是权威状态。CST lossless，SDG 是投影。视觉编辑只对带 reversible capability 的节点启用，
并通过 adapter 写最小 source patch。整篇另一方言生成结果不能静默回写原项目；切换主语言需迁移或另存为。
项目可以包含独立的异语言组件，每份组件原文仍是单一权威资源；编辑它只切换团队语言，不改变主入口。

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

Raw 节点让未知 LaTeX 宏或 Typst 函数不丢失。相同方言生成时原样输出；其他方言尝试用户配置映射，
否则用 fallback 并产生 Degraded，或阻止导出。Raw preamble 不进入 Typst 快速预览执行。

Raw 保留与混合执行是不同能力。要使用原语言实际输出，需将 Raw 绑定为 ForeignSource，明确原文资源、
依赖、scope、排版与符号桥接。正文、公式、图表、宏、模板及跨组件引用全部在目标范围内。
宏/模板在所属引擎作用域执行，不把 LaTeX preamble 注入 Typst 解释器；未经桥接的内容标记 Unresolved。
fallback 只能作为带标识的降级或预览；未解决组件不能通过正式构建门禁。

## 4. LaTeX → SDG → Typst

1. lossless parse LaTeX，建立宏、环境、label、cite 和资源索引。
2. 只展开已声明为纯且安全的宏；其余保留 Raw。
3. 将已知节点投影到 SDG，并记录 provenance 和 coverage。
4. Typst generator 按语义模板生成源码与 SourceMap。
5. 需要原语言能力的部分由受控 LaTeX 组件编译并桥接到 Typst 宿主；Typst 可用于快速预览和最终输出。
6. 预览中的 Raw/未解决区域以占位、fallback 或诊断呈现；正式输出须验证所有组件、分页与引用。

快速预览不宣称等同 LaTeX 最终排版。Typst 最终输出也必须单独验证，不把预览占位直接发布。

## 5. Typst → SDG → LaTeX

1. parse Typst 的 markup/math/常用函数调用；任意代码与动态计算默认 Raw。
2. 将静态可理解结构投影到 SDG。
3. LaTeX generator 根据 document class/profile 选择命令、环境和包。
4. 生成 preamble requirements、正文、资源与 ConversionReport。
5. 需要保留 Typst 编程能力的内容在 Typst 组件作用域构建，经声明桥接进入 LaTeX 宿主。
6. 在隔离 TeX 环境验证最终构建，诊断映射回 SDG 节点或组件源位置。

Typst 的动态编程能力无法普遍翻译为 LaTeX。原文可作为混合组件执行，但不能把运行结果反推为可逆语义树。

## 6. 受支持等级

每个语义节点、每个方向独立声明：

- Exact：目标能保持语义与关键属性。
- Equivalent：语义一致，具体命令或排版算法不同。
- RawPreserved：只在同方言无损保留。
- Degraded：使用可见 fallback，丢失部分属性。
- Dropped：无输出；默认禁止完成转换。

版本化 compatibility matrix 进入测试数据，UI 可在转换前按文档统计。

保真度之外分别记录执行路径 Native/ForeignRendered/Bridged/Unresolved、编辑性和工具链可移植性。
ForeignRendered 不是 Exact 的同义词；保留原文也不代表目标源码原生可编辑。报告覆盖原始输入及所有依赖。

## 7. 生成稳定性

generator 输出必须确定且尽量局部稳定。每个节点有独立渲染函数与 style context；修改一段不应导致整篇
源码无意义重排。用户选择 formatter profile 后才规范化生成格式。

SourceMap 同时记录 generated range → NodeId 和 NodeId → ranges；生成源码手改后映射进入 dirty 状态，
重新 parse/diff 成功后建立新 generation。

## 8. 预览调度

- **预览时延按实测设定，不按帧预算设定。** 阶段 0 实测（[报告 0005](spikes/SPK-0005-typst-mapping.md)，
  20 页 / 257 段，release，复用同一个编译 World）：
  单字符编辑的"重新生成 + 重新编译"合计中位 **19.2 ms**、p95 **21.3 ms**；
  改动首段导致源码整体平移时 p95 达 **207.8 ms**。因此：
  - 编译**必须异步**：帧内只入队与取用最新结果，绝不同步等待；debounce 取百毫秒量级
    （建议 150–250 ms，可按可见页优先进一步优化）；
  - **旧 revision 的结果一律丢弃**（只采纳比已落地更新的 revision），并用负对照测试守住——
    朴素的"谁最后到用谁"必须被同一用例抓出画面倒退；
  - 若要把预览压到帧级，只能做视口局部编译或源码级差分，当前未实现。
- LaTeX 验证构建按空闲、保存、显式命令或发布策略触发，永不阻塞输入。
- 活动源码语言不限制构建调度；混合依赖中的两种引擎均可运行，无依赖任务可并行。
- 源码项目按原生后端预览；LaTeX 项目可选择实验性 Typst 结构预览，但必须带明显标签。
- 预览缓存 key 包含 SDG/source revision、backend、template、资源 hash 和工具链版本。

## 9. 差异诊断

双后端比较关注结构而非像素：缺失节点、编号、引用目标、目录项、脚注、图片/表格、数学语义和构建错误。
分页、断行、字距等差异默认信息级；用户可以选择 LaTeX 最终预览确认。

### 混合排版与引用

冻结完整输入后构建外语组件、宿主排版和符号桥接。宿主控制最终页面与计数器；外语模板只影响声明的组件/
章节，冲突的全局样式须选择宿主或分节策略。正文和跨页表格需结构/分页合约，不能用整页图片替代流式支持。
跨语言宏调用通过显式参数/结果适配；编号、文献、目录、页码与链接通过稳定符号 ID 连接。
引用反馈采用有上限的多轮构建，以最终装配的规范化引用/布局清单判断收敛；达到上限返回 Unresolved。
缓存包含全部组件、工具链、字体、模板、桥接版本和轮次输入。正式输出验证最终链接目的地与页码。
具体嵌入载体和分页桥接由阶段 0 验证，不预先假定任意模板组合可实现。

## 10. 测试门禁

- SDG → LaTeX/Typst → SDG 的支持子集语义 round-trip。
- 源码项目局部视觉编辑的未触及字节稳定。
- Raw 同方言无损、跨方言明确降级。
- 双后端结构清单一致性。
- SourceMap 双向定位、旧 revision 淘汰和生成源码 dirty/reconcile。
- 随机嵌套节点生成后两后端均不崩溃，错误可定位到 NodeId。
- 混合正文/宏/模板/跨页表格、双向编号与页码引用分别通过两种最终宿主输出。
- 缺依赖、模板冲突、引用不收敛会阻止正式输出，原文和上次成功产物保留。
- 两种源码目标均测试标准工具链包与混合重建包，编辑性和依赖声明符合实际。
