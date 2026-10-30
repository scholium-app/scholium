# scholium-render

## 职责

按项目语言调度对应内核的预览，管理原生预览页与导出 artifact、源码/节点 ↔ 预览位置映射、
视觉选区与 caret geometry、实时/整编状态及诊断。预览与最终输出使用同一引擎。

## 非职责

不修改文档，不实现 Typst/TeX 编译器，不持久化权威内容，不决定项目语言或执行跨格式转换。

## 内部结构

```text
src/
├── scheduler.rs    debounce、取消、revision 淘汰
├── projection.rs   SDG → Typst generation 请求
├── scene.rs        preview artifact/page model
├── mapping.rs      TreeAnchor/NodeId ↔ preview geometry
├── overlay.rs      caret、selection、remote presence、focus
├── final_preview.rs LaTeX/Typst 最终 artifact 状态
└── cache.rs        revision/profile/resource keyed cache
```

## 原生展示

页面与选区通过所选原生 UI 的绘制接口呈现，不嵌入浏览器。HTML 是导出/剪贴板数据；应用内预览展示对应
内核的引擎结果，不能执行 HTML 脚本或以 WebView 绕过技术栈约束。字体、字形和 PDF/SVG 解码可用受控原生库。

## 预览状态

`PreviewCurrent`、`PreviewStale`、`FinalBuilding`、`FinalCurrent`、`FinalStale`、`Failed` 明确显示。
显示项目引擎与实时/整编来源；pdfLaTeX/XeLaTeX 显示“整编预览”及延迟，旧结果不能冒充最新结果。

## 交互映射

显式 Content 候选的映射直接来自同一 Frame 的 opaque-origin 注册表，使用 typed 叶位置与
完整 SceneStamp，不经过 generated markup shift。其受控单 flow、全 Scene 交付边界见
[Typst 模块](typst.md#直接-content-开发接口)、[ADR 0034](../adr/ADR-0034-typst-main-app-candidate.md)。

Typst 点击预览通过 layout span → generated SourceMap → NodeId → document focus。视觉 caret 的权威位置仍是
TreeCursor；geometry 只是当前 revision 派生物。derived 内容如自动编号可以选择其拥有者节点，但不可进入
不存在的文本槽。
LaTeX 通过本引擎的 span/SyncTeX 或等价映射回权威源码范围，不借用 SDG 节点或 Typst 几何。

## 公共接口

- `schedule(input_snapshot, profile) -> PreviewRequest`，负责 debounce、取消和 revision 淘汰。
- `preview_state() -> PreviewState`（PreviewCurrent / PreviewStale / FinalBuilding / FinalCurrent / FinalStale / Failed）。
- `hit_test(page, point) -> Option<PreviewTarget>`；`bounds(TreeAnchor) -> Option<Geometry>`。
- `overlay() -> OverlayInputs`，只输出 caret、selection、presence 的绘制输入，不写回 graph。

## 不变量

- 过期预览不替换 current，也不提供当前点击定位。
- 编辑画面的图像、源码映射与字形几何必须同 revision 原子采纳；不能将编译图像与旧的自绘布局混合用于交互。
- 编辑投影的空槽位标记只是派生显示，不得进入文档文本或发布产物；显示投影与可写源码投影分别标识用途。
- 预览与最终输出使用同一引擎，引擎与实时/整编来源标签不可隐藏。
- overlay 不写回权威内容；编辑请求交还对应内核校验。
- 页面虚拟化不改变 NodeId 映射。
- 持久编辑会话返回完整场景资源清单；丢弃中间 revision 不得丢失其后续仍依赖的像素变化。
  页面资源可复用，但图像块与交互几何必须原子采纳，会话重建不得误用旧资源标识。
- 结构布局的文字尺寸与文字绘制使用同一字体度量；节点外框包含自身符号、线条及后代，
  随布局平移，不由纯文本后代范围代替。空可编辑槽位仍提供光标几何。

## 失败处理

- 项目引擎编译失败时进入 `Failed` 并展示诊断，保留上一次 current，不隐藏后端标签。
- 旧 revision 结果直接丢弃，既不替换 current 也不提供点击定位。
- Typst source span 找不到 NodeId 时标记 derived；LaTeX 源范围无法定位时返回无映射，不猜测位置。
- 缺少可靠 geometry 的区域禁用精确光标操作并给出解释。
- 占位或过期预览不能进入正式输出；诊断必须可定位。

## 测试门禁

调度取消、旧结果竞态、多页虚拟化、嵌套映射、derived 内容、缩放滚动坐标、远端光标、后端状态机、
LaTeX 构建慢于连续编辑、同引擎实时/整编状态与定位诊断。

## 项目语言合约

预览调度与展示按固定项目语言接入对应内核，预览与最终输出同一引擎，结果与定位绑定同一 revision。
Typst 展示 SDG/源码的 Typst 排版；LaTeX 展示适配层的段落/页面更新与定位/诊断，整编结果明确标识。
跨语言只经转换或导出及报告；混合组件、双后端差异诊断与团队语言切换已推迟。
共同要求见 [项目语言、跨格式转换与文件操作](../MIXED_SOURCE_EDITING.md) 与 [ADR 0041](../adr/ADR-0041-separate-latex-typst-kernels.md)。
