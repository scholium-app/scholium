# ADR 0034：主程序的隔离 Typst Content 编辑候选

## 状态

接受，仅用于显式开发候选；不批准默认发行后端替换。

## 背景

[ADR 0032](ADR-0032-typst-edit-kernel.md) 的同源布局实验必须进入真正的 `scholium-app`，
才能验证已提交内容的绘制和保存恢复。[ADR 0033](ADR-0033-structured-local-session-migration.md)
提供了有稳定身份的 `LocalSession<StructuredDocument>` 与隔离 v1 文件。
锁定 Typst 版本的依赖通告仍未处理，不能用开发实验批准默认发行依赖。

## 决定

1. 根 workspace 保留 registry Typst。`dev/typst-editor` 是独立开发 manifest，二进制仍为
   `scholium-app`，直接使用 `crates/scholium-app/src/main.rs`。它统一 patch 13 个 Typst
   库到跟踪的 `vendor/typst-edit`，不依赖被忽略的实验 checkout。普通 root 构建拒绝开发选项。
2. vendored 库由上游 v0.15.1 提交 `9dfd3a08500b7896045f907433cf7b4b02434fad` 和四份
   已跟踪的编辑补丁重建，保留 Apache-2.0 LICENSE。上游 workspace 仅裁剪成员/默认成员到
   这 13 个库。验证脚本从 Git 对象重建并逐文件比较，不接受 checkout 中未记账的修改。
3. 开发候选必须显式传 `--typst-editor`，并指定 `SCHOLIUM_SESSION_FILE`。已有文件只按
   v1 打开；缺失文件原子创建；旧 schema 或坏库启动失败，不自动迁移、另建权威或覆盖。
4. `SessionBridge` 在 legacy 恢复前互斥选择候选。候选唯一可写内容仍是
   `scholium-document::LocalSession`，UI 元数据、历史保存点与 Typst Content 都是派生物。
   复用现有 Ribbon、视图命令、保存文件环境变量和 SessionStore。
5. 每帧依次接受结构输入，再提交布局、绘制。布局和字体扫描在常驻 CPU 线程执行，完成后
   唤醒 egui；UI 不 join。两个单槽 mailbox 只合并派生布局请求/结果，不合并或丢掉权威动作。
   传输暂用完整不可变快照，worker 按稳定节点/语义子树保留 Content；这不是节点级增量协议。
6. 图像与光标几何来自同一 padded Frame，整体按完整 SceneStamp 采纳。撤销/重做、新建、
   恢复清理场景并换 epoch。等待/失败保留标明 revision 的旧图，停用旧命中、caret 与 IME
   锚点。页面正文和数学只画 Typst 纹理；唯一可画文字的页面临时层是 IME preedit。
7. 单机历史保存完整快照、对应 request journal 与光标，最多 512 步；redo 不从较短日志
   猜请求前缀。保存状态比较完整快照，避免撤销后相同 revision 的不同内容被误报已保存。

## 能力和代价

限定正文、整叶样式、标题、inline Math Text/Row/Symbol/Fraction/Hole，文字输入不求值。
UI 先接入正文/数学叶的键盘输入、分数、Tab、局部字素删除、正文拆段和单机历史。
空公式/分母是带身份的非打印结构，保存不插入提示字符。RawMath 原串仍保存在 v1，直接
adapter 报节点诊断并拒绝该次布局，不执行或伪造成可编辑公式。

完整单页流宽 420 pt，外围 padding 10 pt，高度预算 2000 pt；超限报错，不截掉尾部。
图像比例为 2 px/pt，显示缩放只改变共同 placement。这里没有 flow 分页/checkpoint、物理
刷新延迟证明或任意 Typst 程序逐键增量保证。快照复制、子树比较和完整栅格仍有文档规模成本。

候选的源码按钮显示只读结构 JSON；生成 Typst 源码工作区、跨叶选区/替换、多行粘贴、
复制剪切、完整输入法/无障碍与旧公式转换须后续接入。普通 workspace 保留原有能力。
多行粘贴在候选中明确拒绝并保留可复制草稿，不能静默截断。

依赖通告不加 ignore；安全门禁不通过前不将 fork 合入默认发行依赖。候选可操作的证据不
等于候选兼容通过或默认替换完成。验证见[报告 0054](../spikes/SPK-0054-typst-main-app-candidate.md)。
