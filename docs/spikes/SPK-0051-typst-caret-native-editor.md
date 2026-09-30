# Spike 0051：Typst 同源光标几何与原生编辑窗口

- 结论：**Pass（分数组合身份、正文 caret 与独立窗口限定场景）**；完整 K1/K2、K3/K4 和正式应用路径尚未验收。
- 对应验证项：[路线图阶段 0 第 1/2 项](../plan/ROADMAP.md)：结构编辑与 Typst 映射。
- 日期：2026-09-30；用户指定深改 Typst 并授权直接实现。
- 方向：[ADR 0032](../adr/ADR-0032-typst-edit-kernel.md)、[编辑内核计划](../plan/TYPST_EDIT_KERNEL.md)。
- 前置：[报告 0050](SPK-0050-typst-edit-origin-and-holes.md)。

## 问题与范围

验证 Typst 内部能否同时保留嵌套数学身份、包含槽位、分数线拥有者，并由实际 shaping/断行
结果给出叶内 UTF-8 插入点。进一步把结构操作接到原生窗口，检查页面像素、点击、光标与
IME 锚点使用同一次布局结果，不再先显示临时正文、随后换成 Typst 页面。

本次没有修改产品 `crates/`。窗口使用既有 `spikes/native-ui/core` 的语义文档图和 actor
文字撤销；这是固定可信结构夹具，不是生产模型迁移，也不处理任意项目代码。

## 版本与环境

Typst 0.15.1，上游 `v0.15.1` / `9dfd3a08500b7896045f907433cf7b4b02434fad`；
Rust 1.96.0 release，Linux 7.2.7-arch1-1，eframe/egui 0.36.2，winit 0.30.13。
字体为 typst-kit embedded + 同机 system 集合，宽度 420 pt、边距 10 pt、栅格 2 px/pt。
真实窗口自动操作在隔离 Xvfb X11 下完成，固定缩放 1。没有执行本窗口的实机 fcitx 验收。

上游保持 Apache-2.0，新探针代码 MIT OR Apache-2.0。fork 位于被忽略的 `.vendor/typst`；
仅保存小补丁系列及锁定依赖。bootstrap 从干净上游重建并比对完整补丁前缀，重复调用安全，
不覆盖不匹配的本地修改。独立 stock reference 使用未修改的 registry 版本。

## 实现

第二个补丁把节点 origin 与包含槽位分开，均参与 Content Hash/相等判断；聚合数学片段
保留两种身份。分数线使用真实线宽与位置输出不打印的装饰拥有者，嵌套分数各归自身。

第三个补丁在 inline collector 保留未改写文字的叶范围，shaping 使用最终字形 advance
生成 EditCluster 与 CaretStop，包括空格、RTL、字素、字体 fallback 和行宽调整。
空段落及连续换行的空行由行布局度量输出零宽插入点。元数据随 Frame inline、shift、
translate 和 Group transform 移动；渲染算法继续忽略这些元数据。

cluster 两端是实际 shaping 边界；**合字内部位置为比例回退，`exact=false`**，不是字体
精确 caret。跨叶合字、大小写改写后的文字拒绝猜测叶内可写位置；裁剪 Group 的交互几何
明确返回错误。数学数字范围独立于节点外框，避免每位数字小框替代整节点框。

窗口 worker 常驻字体/World；请求为容量 1 的完整文档快照，UI 保留最新 pending 请求。
像素和几何在同一 Scene 内交付，只采纳当前语义 revision；旧结果整体丢弃。
连续输入先进入唯一语义模型，后续快照包含已接受修改；pending 时旧图明确标旧 revision，
暂停旧图点击与光标。没有在 UI 线程布局，也没有已提交正文的 egui Text 绘制。
IME preedit 不修改文档，commit 防止同帧 Text 事件重复；候选锚点由同源 caret 提供。

这仍是整夹具 Content 投影与布局，不是 EditorLayoutSession 增量 Content、flow 检查点
或 SceneDelta。请求合并也不是 Typst 内部布局可中断/预算调度的证明。

## 结果与证据

[内核日志](evidence/SPK-0051/kernel.log)、[总验证日志](evidence/SPK-0051/verification.log)、
[原生操作汇总](evidence/SPK-0051/native/summary.json)、[四项 headless 回归](evidence/SPK-0051/native-tests.log)。

| 检查 | 结果 |
|---|---|
| 嵌套组合身份 | 内层 node 300 与外层 slot 400/11 同框；两者同时保留 |
| 分数线归属 | 两条实际分数线分别由 node 300/400 拥有 |
| 缓存隔离 | 同 Content 不同 containing slot 600/700 不串身份；复合 row 槽位非零 |
| 正文与空行 | 中英/组合字符/ZWJ、合字、RTL、bidi、长段落、空段、连续换行全部覆盖字素边界 |
| 缓存与变换 | 重复 Frame hash 相同；同文本不同 leaf 不串身份；平移与 1.5 倍缩放 caret 正确 |
| 保守映射 | 大小写改写及跨叶 ffi cluster 不生成猜测的可写端点 |
| stock 对照 | 完整/嵌套分数及七组正文共九张图，同机同字体差异像素全部为 0 |
| 真实窗口操作 | 六组操作全部通过，记录了实际图像与 revision/光标/文字状态 |
| headless 行为 | 正文无 Text Shape 且有 IME 锚点；preedit/commit；过期 Scene 原子丢弃；正文分数命令拒绝，4/4 |

正文 fixture caret 数量分别为 42/38/22/40/4320/1/29；合字测试有 8 个、长段落有 160 个
非精确回退端点。邻接字体可能共享视觉边界但有不同高度，命中要求点击位于选中光标段上，
不要求相同字素边界的每个字体度量完全相同。

真实窗口场景依次为：点击空分母填 `12`、包装为嵌套分数并填 `3`、撤销恢复空槽、
pending 时点击旧正文仍继续在当前分母输入 `qw`、正文 UTF-8 插入/移动/删除/撤销。
包括初始空槽共六份截图，已人工查看：
[嵌套分数](evidence/SPK-0051/native/nested-fraction.png)、
[空槽恢复](evidence/SPK-0051/native/undo-restores-hole.png)、
[正文编辑](evidence/SPK-0051/native/utf8-body-edit-and-undo.png)。

测试人为增加 **200 ms** 布局等待以稳定重现 pending 状态。日志中的 `layout_ms` 和
`raster_ms` 只是 worker 阶段记录，不代表事件到呈现延迟；本次不宣称达到 8/32 ms 性能门禁。
初次自动化继承 Wayland、随后又混用逻辑/物理像素而点击错误；固定隔离 X11 与缩放 1 后
重新执行。本报告只使用最终成功运行的证据，不把环境错误计为产品行为结论。

格式、release Clippy（含 native 和 stock reference）、editor 关闭编译、文件/函数规模通过；
许可证/来源/bans 通过，**完整 advisories 安全门禁 Fail**，见[依赖检查日志](evidence/SPK-0051/deny.log)。
锁定 Typst 依赖包含 bincode、paste、rustybuzz、ttf-parser、yaml-rust 停维护通告及 quick-xml
的 RUSTSEC-2026-0194/0195 漏洞通告。没有增加忽略项或修改许可证/安全政策；不能将功能
限定 Pass 当作安全通过或生产接入许可。既有生产测试不覆盖该独立 workspace。

## 局限与后续入口

- 只覆盖正文 Text、段落、Math 与分数；根式、上下标、矩阵和更复杂装饰未验收。
- 没有全范围行/选区模型、视觉 bidi 导航、跨叶合字编辑、裁剪命中或 show-derived 映射。
- 无保存重开、分数结构撤销、协作 TreeAnchor 与生产数学模型迁移；探针 opaque ID 仅本夹具有效。
- IME 为事件适配与候选矩形 headless 检查，不能替代平台输入法/无障碍验收。
- 仍为 EmptyIntrospector；无分页、脚注、全局 query/state/counter、增量 Content 或性能验收。
- 锁定依赖安全门禁未通过；替换/升级涉及字体与资源解析兼容，需要单列并重新做排版对照。
- 正式应用仍有旧输入路径。本次只证明支持子集可以在独立原生窗口取消正文回显交接；
  K3/K4 通过前不能默认替换产品依赖。ADR 0032 保持 Proposed，阶段出口判定不变。

下一批按原计划验证持久 Content 与 flow 分页，再做版本化模型迁移和应用 SceneDelta 接入；
不可把本窗口的完整快照请求改名成“增量会话”后跳过对应门禁。

## 复现

见[探针 README](../../spikes/typst-edit-session/README.md) 的内核和原生窗口命令。
历史报告 0050 的两图/初始补丁结论对应提交 `9c9ae8a`；当前 check 默认写入本报告证据目录，
不会覆盖 0050 的历史文件。需要保留多轮结果时用 `check.py --out <fresh-directory>`。
