# scholium-render

## 职责

把 SDG revision 调度为 Typst 快速预览，管理原生预览页与导出 artifact、NodeId ↔ 预览位置映射、视觉选区与
caret geometry、LaTeX 验证预览状态以及双后端结构差异诊断。

## 非职责

不修改文档，不实现 Typst/TeX 编译器，不持久化权威内容，不把快速预览冒充 LaTeX 最终结果。

## 内部结构

```text
src/
├── scheduler.rs    debounce、取消、revision 淘汰
├── projection.rs   SDG → Typst generation 请求
├── scene.rs        preview artifact/page model
├── mapping.rs      TreeAnchor/NodeId ↔ preview geometry
├── overlay.rs      caret、selection、remote presence、focus
├── final_preview.rs LaTeX/Typst 最终 artifact 状态
├── compare.rs      双后端结构清单比较
└── cache.rs        revision/profile/resource keyed cache
```

## 原生展示

页面与选区通过所选原生 UI 的绘制接口呈现，不嵌入浏览器。HTML 是导出/剪贴板数据；应用内预览由语义模型
走原生渲染，不能执行 HTML 脚本或以 WebView 绕过技术栈约束。字体、字形和 PDF/SVG 解码可用受控原生库。

## 预览状态

`FastCurrent`、`FastStale`、`FinalBuilding`、`FinalCurrent`、`FinalStale`、`Failed` 明确显示。发布目标为 LaTeX
时，快速 Typst 预览始终带后端标识；最终构建完成后用户可切换或分屏比较。

## 交互映射

点击预览通过 layout span → generated SourceMap → NodeId → document focus。视觉 caret 的权威位置仍是
TreeCursor；geometry 只是当前 revision 派生物。derived 内容如自动编号可以选择其拥有者节点，但不可进入
不存在的文本槽。

## 公共接口

- `schedule(sdg_revision, profile) -> PreviewRequest`，负责 debounce、取消和 revision 淘汰。
- `preview_state() -> PreviewState`（FastCurrent / FastStale / FinalBuilding / FinalCurrent / FinalStale / Failed）。
- `hit_test(page, point) -> Option<PreviewTarget>`；`bounds(TreeAnchor) -> Option<Geometry>`。
- `overlay() -> OverlayInputs`，只输出 caret、selection、presence 的绘制输入，不写回 graph。
- `compare(fast, final) -> Vec<BackendDifference>`，只报告结构、引用和编号级差异。

## 不变量

- 过期预览不替换 current，也不提供当前点击定位。
- 快速/最终后端标签不可隐藏。
- overlay 不写回 graph；所有输入转为 SemanticEdit。
- 双后端比较只报告结构/引用/编号等可验证差异，不声称像素一致。
- 页面虚拟化不改变 NodeId 映射。

## 失败处理

- Typst 编译失败时进入 `Failed` 并展示诊断，保留上一次 current，不隐藏后端标签。
- 旧 revision 结果直接丢弃，既不替换 current 也不提供点击定位。
- source span 找不到 NodeId 时标记 derived，不猜测最近节点。
- 缺少可靠 geometry 的混合组件禁用精确光标操作并给出解释。
- 占位或过期预览不能进入正式输出；快速与最终后端差异必须可定位。

## 测试门禁

调度取消、旧结果竞态、多页虚拟化、嵌套映射、derived 内容、缩放滚动坐标、远端光标、后端状态机、
LaTeX 构建慢于连续编辑和双后端差异诊断。

## 团队源码语言与混合项目合约

显示混合组件状态 Native/ForeignRendered/Bridged/Unresolved 和两种最终宿主结果；Raw 与可执行组件不同。
预览点击映射到组件原文位置，异语言原文在团队切换前只读；原文跳转无需先切换语言。
带占位/旧组件的快速预览不能冒充正式输出。宿主、组件 revision、草稿状态与转换差异均可定位。
支持正文分节/分页桥接的显示，不以整页图像冒充可重排正文；无可靠 geometry 时禁用精确光标操作并解释。
新增混合映射、多引擎旧结果淘汰、最终页码/链接和只读交互测试。

共同要求见 [混合源码与团队编辑](../MIXED_SOURCE_EDITING.md) 与 [ADR 0001](../adr/ADR-0001-mixed-source-team-editing.md)。
