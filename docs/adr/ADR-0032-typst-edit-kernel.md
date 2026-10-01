# ADR 0032：扩展 Typst 为结构编辑布局内核

- 状态：Proposed（用户已指定研究方向；内核补丁与集成仍待验证）
- 日期：2026-09-30
- 影响模块：model、document、typst adapter、render、app
- 实施判据：[Typst 编辑内核计划](../plan/TYPST_EDIT_KERNEL.md)
- 主程序接入顺序：[接入计划](../plan/TYPST_MAIN_APP_INTEGRATION.md)
- 入口证据：[报告 0049](../spikes/SPK-0049-typst-content-layout.md)
- 身份/空槽限定证据：[报告 0050](../spikes/SPK-0050-typst-edit-origin-and-holes.md)
- 组合身份/caret/原生窗口限定证据：[报告 0051](../spikes/SPK-0051-typst-caret-native-editor.md)
- 持久 Content 与增量段落限定证据：[报告 0052](../spikes/SPK-0052-typst-incremental-content.md)

## 背景

页面先使用 egui 绘制临时文字/公式源码，再被完整 Typst 页面替换。
当前画面与交互几何无法一起局部重排，输入前后可能换字体、换结构或错位。
用户选择深改 Typst：保留其排版算法，使结构输入本身获得可编辑的布局结果。

## 验证方法

先验证锁定 0.15.1 的公开 Content → Frame 入口；报告 0049 仅证明不用读取源码也能
布局段落和空/完整分数，不证明编辑场景和增量分页。其后按计划 K1–K4 验证稳定身份、
Hole、caret、memoize 身份隔离、分页检查点、动态依赖和真实窗口端到端时延。
修改后的合法支持子集须与同字体 stock Typst 完整排版比较；输入未完成状态单列。

## 候选方案

1. 保留源码编译与临时 echo，缩短去抖、复用缓存、换矢量绘制：无法建立同源新几何。
2. 自研文字/数学排版内核：能定义编辑生命周期，但需要另行承担排版算法和后端差异。
3. 扩展 Typst Content、math/inline/flow 为结构编辑内核：复用排版算法，同时承担稳定身份、
   空槽、依赖和持久会话改造。按用户指令选择该研究方向。

## 决策提案

在独立 spike 中管理固定上游版本的 fork 与小补丁系列，不直接修改 registry 或默认产品依赖。
原生文档从结构增量构造 Content；Typst 自身输出 Frame 与 EditGeometry。
model 保持唯一权威，Typst 的借用 IR、Content、Frame 均不进入长期存储或协作协议。

绘制、命中、光标、选区和 IME 使用同一场景产物，复用节点局部版本与依赖经过验证的布局。
空数学槽位由编辑模式的 Hole 表达，严格输出拒绝必填 Hole。
完整源码编译、标准导出和全局 introspection 收敛继续保留；未知动态依赖保守全量重算。
未完成布局的区域显式 pending，不能拿旧几何定位新内容，也不能用普通文字回显代替公式。

本提案通过验证并正式接入后，需同步修订 EXPERIENCE、ARCHITECTURE 和相关模块合约，
细化/替代 ADR 0029/0030 的页面映射与输入期源码降级路径。
当前不将这些已接受的产品边界标成 Superseded，也不改变阶段出口判定。

主程序内的开发候选在显式验证入口、隔离测试会话中复用真实 session/storage；不嵌入另一份
spike Editor 充当正文权威。结构身份/迁移与必要同源几何先验证，再交替推进应用适配和
flow 分页；候选可运行不表示普通启动已经替换。稳定身份及本地候选持久格式的迁移边界见 [ADR 0033](ADR-0033-structured-local-session-migration.md)，
默认启用和旧路径移除仍受完整 K1–K4、兼容与安全门禁约束。

## 后果

预计可减少逐键源生成/eval 与全场景扫描，并使输入期结构与完成后的排版使用相同算法。
代价是维护 Typst fork、原生数学树迁移、正确的缓存身份和分页依赖；这些成本不是换一个
绘制 API 就能消除。任意 Typst 模板/程序的局部输入时延不能与静态原生子集等同承诺。
直接 Content 布局也可能执行 show/context，既有不可信执行与资源沙箱规则继续适用。

## 替换方案

上游合入对应接口后，以同夹具替换 fork；保持 adapter 合约不泄漏 Typst 内部类型。
任一步无法通过门禁时停留在独立 spike，保留当前产品与严格输出路径，并报告具体失败范围。
切换上游版本、替换内核或降低支持子集均须重新测量并记录兼容边界。
