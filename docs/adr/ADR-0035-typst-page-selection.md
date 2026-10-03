# ADR 0035：Typst 候选页面选区的语义端点与同源几何

## 状态

接受，仅扩展 ADR 0034 的显式开发候选，不批准默认后端替换。

## 背景

正文原子范围内核已有稳定 Text 端点；主程序需要在 Typst Frame 中呈现和命中这些端点。
跨段、多行插入可能创建新叶，UI 不能从前后快照差异猜光标 ID；旧 Scene 或比例合字
坐标也不能作为新位置的命中证明。见[报告 0056](../spikes/SPK-0056-structured-body-range.md)。

## 决定

1. app 保存 anchor/focus 的稳定正文 Text 叶 ID 和 UTF-8 字素 byte。核心只接受按文档
   顺序归一化的 BodyTextPosition；完整覆盖的公式可摘除，数学内部跨槽位选区不在此接口。
2. 原子范围规划器返回 StructuralOutcome 的存活光标，包含新建叶 ID，并在接受内容中
   重锚到合法字素边界。bool API 委托同一流程；no-op 可返回光标，拒绝不能修改 UI
   两端。结果属于瞬时本地编辑返回值，不增加持久化 schema 或协作协议。
3. Typst adapter 从同一 padded Frame 的 shaped cluster stops 与数学根结构 bounds
   生成 SelectionQuad；不从 markup、inline 包装或旧 raster 推测矩形。缺范围映射失败，
   合字比例边界保留 exact=false。空段落的 1 pt 标识只表示选中的结构边界。
4. 鼠标事件与文字/键盘事件依输入顺序处理。命中只接受当前全 SceneStamp、已知叶的
   精确 caret；拖选额外绑定 placement。接受新编辑、恢复换 epoch、请求改变或页面
   缩放/位置变化均使旧拖选失效。pending 可按合法语义端点继续键盘操作，不命中旧图。
5. app 对包含 RawMath 的范围明确拒绝；当前场景缺选区几何也拒绝范围替换。失败保留
   权威、两端和被拒绝输入草稿。复制剪切、选区样式/结构命令和数学内部选区另行接入。
6. undo/redo 保存本地两端和完整权威快照/日志，恢复后换 epoch。所有选框是非打印
   overlay；已提交正文仍只由 Typst 纹理呈现，保留 ADR 0034 的依赖与入口隔离。

## 代价与替换条件

逻辑叶导航不代表视觉 bidi/上下行导航；范围规划和历史仍复制全快照。多边形几何可表达
Frame 变换，但本批窗口夹具不证明任意文档、物理延迟、真实 IME 或无障碍通过。
未知能力必须先扩展语义及同源几何，并提交独立验收；不得增加临时正文回显补洞。
实现与限定窗口证据见[报告 0057](../spikes/SPK-0057-typst-page-selection.md)。
