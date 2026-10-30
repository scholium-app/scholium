# ADR 0041：LaTeX 与 Typst 分为两个编辑内核，项目语言固定

- 状态：Accepted（产品方向）；LaTeX 实时预览引擎的选型另立 ADR，见下文“验证方法”
- 日期：2026-10-30
- 决策者：产品方向由用户确认；rtex 接入待评估
- 取代：[ADR 0001](ADR-0001-mixed-source-team-editing.md)（团队单一源码语言与混合项目）
- 影响模块：model、document、format、latex、typst、build、render、storage、collab、history、app、sync-server

## 背景

ADR 0001 让一个项目同时承载 LaTeX 与 Typst：语义图可生成两种源码，LaTeX 发布目标借 Typst 做快速预览，
异语言内容以 ForeignSource 组件混用，团队用 `ActiveSourceDialect` + `edit_epoch` 的切换协议保证同一时刻
只有一种语言可写。阶段 0 在声明子集内验证了这些机制（报告 0009、0010、0035，判定见报告 0012 第 7–10 条），
但它们带来的兼容负担始终在：

1. **预览与最终输出不同源。** 规则 6 只能要求 UI 标明“Typst 快速预览不是 LaTeX 结果”，并为两后端差异
   另设诊断（RENDERING_CONVERSION §9）。用户看到的始终不是将要发布的排版。
2. **混合构建是最长的关键路径。** 跨引擎宏/模板桥接、多轮引用收敛、矢量载体、混合重建包都只在夹具子集通过，
   正式化需要阶段 3 全部工期。
3. **团队语言切换只为混合而存在。** Draining 状态机、写许可、写集校验、旧 epoch 拒绝、离线草稿合入，
   都是因为一个分支内可能有两种语言的写入才需要。
4. **LaTeX 源码项目只能拿到降级预览。** `.tex → SDG → Typst` 的结构预览注定有损。

2026 年 10 月出现的 [realtime-tex（rtex）](https://github.com/HenryXiaoYang/realtime-tex) 改变了第 1 条的前提：
它让一个已加载导言区的、未经修改的 LuaLaTeX 进程只重排被编辑的段落（约 1 ms，与文档长度无关），
全局分页、浮动体、目录与引用交给后台整编；输出与普通 LuaLaTeX 构建逐字形一致（精确到 1/65536 pt）。
依据是 Clemens Lode 在 TUG 2026 的论文《Real-Time LuaTeX》。LaTeX 项目因此可以拥有**用自身引擎产生的**
实时预览，不必借道 Typst。

当前仓库没有任何 LaTeX 生产代码：所有 crate 都服务 Typst/SDG 路径，LaTeX 与混合只存在于 spike 和文档。
现在拆分的代码代价最低。

## 候选方案

1. **维持 ADR 0001，只把 LaTeX 快速预览换成 rtex。** 解决第 1 条，但第 2–4 条照旧，关键路径不变。
2. **拆成两个独立应用。** 边界最清楚，但窗口壳、存储、历史、协作、构建沙箱要维护两套发行与测试。
3. **同一应用、两个编辑内核，项目语言在创建/导入时固定。** 共享底座，内核互不调用；跨语言只走一次性转换。

## 决策

采用方案 3。

1. **项目语言（ProjectLanguage）固定。** 每个项目在创建或导入时确定为 Typst 或 LaTeX；Markdown/BibTeX 作为
   资源不改变项目语言。项目内只有一种排版语言，不存在运行时语言切换、团队语言 epoch 或 Draining 阶段。
   改换语言是规则 2 已有的“带兼容报告的迁移”：「转换为…」生成新项目，原项目保持不变。
2. **Typst 内核**：原生结构项目（SDG 为权威）与 `.typ` 源码项目。Typst 同时负责交互预览与最终输出，
   现有 `scholium-model`、`scholium-document`、`scholium-typst` 与 Typst Content 编辑器全部归属此内核。
3. **LaTeX 内核**：`.tex` 源码为权威（SourceAuthority）。预览与最终输出使用同一 LaTeX 引擎：
   - LuaLaTeX 项目：实时段落重排 + 后台整编（候选 rtex，待评估）；
   - pdfLaTeX / XeLaTeX 项目：后台整编作为预览，UI 标明“整编预览”与其延迟；
   - 不再经 `SDG → Typst` 为 LaTeX 项目产生预览。
   首发能力是源码编辑、精确实时预览、源码 ↔ 页面双向定位与诊断；在页面上直接编辑 LaTeX 正文须另行评估。
4. **共享底座**：窗口外壳、命令注册表与界面文字目录、storage、history（Action/checkpoint）、collab（文本与树
   CRDT）、build 沙箱、诊断面板。工作区按项目语言装配内核；两个内核之间不互相调用，也不共享文档模型。
5. **推迟、不再是产品要求**：ForeignSource 混合组件、跨引擎宏/模板/引用桥接、混合重建包、团队语言切换协议
   （控制记录、写许可、写集校验、epoch）、“Typst 快速预览代替 LaTeX 目标”及双后端差异诊断。
   阶段 0 对应证据（报告 0009、0010、0032、0035、0038 等）保留为可追溯记录，不再约束后续阶段出口；
   [ADR 0016](ADR-0016-team-window-gate-spike.md) 的团队门禁 spike 结论同样不再是产品路径。重新引入任何一项须新立 ADR。
6. **跨格式只走转换与导出。** LaTeX ↔ Typst 保留 `源 CST → 转换 IR → 目标生成器`，逐节点报告
   Exact/Equivalent/RawPreserved/Degraded/Dropped；Raw 只在同方言无损保留，跨方言降级或阻止导出。
   原生结构项目可导出 LaTeX 源码（generator 子集 + 报告），这是导出能力，不是在 Typst 项目里编辑 LaTeX 目标。

据此修订 [AGENT.md](../../AGENT.md) 规则 6、8、9。

## 验证方法

产品方向不依赖 rtex；LaTeX 内核的实时预览引擎是第三方核心选型，必须先评估再另立 ADR。评估计划见
[LaTeX 工作区计划](../plan/LATEX_WORKSPACE.md)，至少包括：

- **中文**：ctex / luatexja 文档能否走实时路径、单段时延、与整编逐字形一致性；
- **沙箱**：rtex 自行拉起 `lualatex`（`PATH` / `RTEX_TEXLIVE_BIN`）且常驻管道通信，必须套入现有 OS 沙箱与运行时白名单
  （阶段 0 出口第 6 条、[ADR 0023](ADR-0023-stage0-feasibility-boundary.md)）。LuaTeX 的 Lua `io` 默认可读任意文件，未隔离不得接入；
- **绘制**：display list（字体文件 + 字形坐标）进入 egui 纹理管线；含 TikZ 等矢量路径的页面以 PDF 回退，
  需另选 PDF 栅格化依赖（MuPDF 为 AGPL，不可用）；
- **定位**：span 字节范围 ↔ 行/字形的粒度能否支撑点击跳转、光标同步与诊断回链；
- **非 LuaLaTeX 回退**：XeLaTeX/pdfLaTeX 整编预览的时延与旧 revision 门；
- **稳定性**：rtex 为 0.0.5，API、C ABI 与 display list 格式未冻结；以适配层隔离并锁定版本。

评估证据写入 `docs/log/`，选型裁决另立 ADR；评估不通过时 LaTeX 内核退回“源码编辑 + 后台整编预览”，本 ADR 的拆分决定不变。

## 后果

正面：

- 每个项目的预览与最终输出同源，规则 6 不再需要“快速预览 ≠ 最终排版”的长期例外。
- 去掉混合构建与团队语言切换两条最长的关键路径；阶段 3、5 的范围显著缩小。
- LaTeX 用户获得自身引擎的精确实时预览，不再承受 `.tex → SDG → Typst` 的有损投影。
- 两个内核各自演进，Typst fork 的改动不会以兼容性代价波及 LaTeX 项目，反之亦然。

负面与风险：

- 失去项目内混用（例如 Typst 项目内嵌 TikZ 图）；用户须先在对应语言中生成图像/PDF 资源再引用。
- LaTeX 内核需要独立的解析、定位、诊断与（后续）可视编辑；Typst 侧的结构编辑成果不会自动惠及 LaTeX。
- LaTeX 实时路径只覆盖 LuaLaTeX；中文社区常见的 XeLaTeX/ctex 项目首发只有整编预览，除非用户迁移到 LuaLaTeX。
- 依赖一个发布不足一个月的上游项目；若长期不可用，需自研同类机制或保持整编预览。

需要新增的门禁：LaTeX 工作区计划中的评估项；LaTeX 内核的沙箱接入与 Typst 内核同等对待；
跨格式转换报告覆盖所有被降级或丢弃的节点。

## 替换方案

LaTeX 实时引擎经适配层（`latex` 模块内）接入，对上只暴露“段落更新 / 页面更新 / 定位 / 诊断”事件；更换 rtex
或改为自研时公共接口保持不变。若将来重新需要项目内混用或多语言团队协作，须新立 ADR 重新定义 ForeignSource
与语言协调，并以阶段 0 报告为起点重新验证，不能直接恢复 ADR 0001。
