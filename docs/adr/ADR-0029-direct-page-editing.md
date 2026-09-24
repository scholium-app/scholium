# ADR 0029：编译字形驱动的页面内编辑

- 状态：Accepted（本地块文档限定范围）
- 日期：2026-09-24
- 影响模块：scholium-model、scholium-document、scholium-typst、scholium-app
- 接续：[ADR 0027](ADR-0027-typst-preview-integration.md)

## 背景与候选

所见即所得要求光标、选区与键入直接发生在排版后的中文正文和公式上。
页面下方的块输入框以及覆盖整段的 TextEdit 都不能满足该交互关系。
沿用[报告 0005](../spikes/SPK-0005-typst-mapping.md)验证的源位置与字形几何机制，
在产品适配器内实现直接编辑；不搬运废弃原型，不改变单一语义权威。

## 决策

Typst 生成投影同时记录生成源码区间到块 ID/规范标记 UTF-8 区间的映射。
编译从实际 Frame 中读取变换后的字形边界、源 span/offset、结构装饰及空槽，
按编译文档 ID/revision 交付 `PageGeometry`。页面坐标单位为 pt。
字形命中转为语义块位置；`alpha` 等命名符号映射完整 token，不能将显示的 α 当一个源字节。

app 在真实页面上绘制光标、选区与 IME 预编辑；不创建另一个正文编辑控件。
鼠标只使用画面与几何的文档、revision、页码完全匹配的结果。
重排中键盘输入仍作用于当前快照位置；源视图定位、缩放和跨页重排同步光标。
IME commit 才写入，同帧预编辑或提交事件禁止指针重定位。

model 新增 `BlockPosition` 与 `BlockEdit::ReplaceRange`。
document 校验 revision、目标、UTF-8 边界、范围顺序与容量后原子应用，可跨块。
首块 ID 保留，范围外节点不变，换行新建后续块，一帧编辑合成一个会话动作。
app 的标记文本仅为逐帧临时投影，保存与编译仍读取 `LocalSession` 快照。
被拒绝的输入保留供复制，并显示未应用的原因。

## 验证方法

`page_editor::tests` 用真实 Typst 编译的 α 字形验证鼠标拖选与直接替换，另覆盖
跨段原子替换、IME preedit/commit、过期画面、页码不符、同帧 IME、隐藏定界符与跨页光标。
document 验证跨块身份保留及非法边界不修改权威；session 验证被拒输入保留。

Linux Xvfb/X11 原生窗口脚本为 `spikes/typst-mapping/review-direct-page.py`，
直接选择已排版的括号子表达式，断言剪贴板精确内容，输入 `x` 后复核排版与光标，
再选择中文全文并断言剪贴板。原始截图与执行数据位于
[映射复验目录](../spikes/evidence/SPK-0005/direct-edit/README.md)，
过程记录见[页面直接编辑日志](../log/direct-page-edit-review.md)。

## 后果与替换条件

页面与输入不再分离。异步排版仍可能短暂显示旧 revision；不能据旧像素猜测新位置。
公式采用现有 `Inline::Math` 源串，并非完整数学树：槽位式导航、结构包裹和复杂公式选择
须后续替换为正式 TreeCursor/Selection。剪贴板复制规范标记文本，不承诺结构化跨应用格式。

本地范围字节位置不作为长期协作锚点；接入共享树时使用 CRDT 相对位置和正式事务 adapter。
生成源码仍只读，不替代 source reconcile。完整平台 IME、读屏字符操作、数学槽位播报与
跨平台验收仍有独立门禁；本决策不宣称 P1 完成。任意不可信源编译仍须 ADR 0012 的隔离。
