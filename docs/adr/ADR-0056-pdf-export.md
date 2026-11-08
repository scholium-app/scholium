# ADR 0056：同引擎 PDF 导出与本机项目目标设置

- 状态：Accepted
- 日期：2026-11-08
- 影响模块：app、typst、latex
- 关联：[ADR 0054](ADR-0054-project-settings.md)、[ADR 0055](ADR-0055-source-preview-worker.md)

## 决策

文件导出的唯一入口捕获当前权威快照，在后台执行并返回共同报告。原生 Typst 使用当前提交文档的
A4 生成预览源码、相同字体与 Typst 排版；RawMath 和未填槽阻止最终输出。普通 `.typ` 继续使用
TypstPreview worker 的显式资源 World，PDF 仅替代 RGBA 产物，挂载和限制不变。vendor 添加同一
锁定上游提交的未修改 typst-pdf，并由现有逐文件重建校验覆盖。

LaTeX 的独立 export_pdf 入口复用 FullCompilation 的快照、两轮引用编译及 XeLaTeX XDV 转换，
接受固定的三个引擎；实时预览的 LuaLaTeX 排除规则不变。不执行 bibliography 工具，报告明确说明。
导出报告属于 app 编排层，不向无 IO 的 model 引入本机路径或具体 PDF 引擎类型。

所有内核在共用发布路径中经 hayro-syntax 打开完整 PDF 一次并读取页数，不执行逐页栅格化。
PDF 专用上限为 2 GiB / 100,000 页，仅阻止病态输出；沙箱 /work 配额是实际构建限制。
保留 Typst 编译器页数校对；预览页数/像素预算不约束导出。
解析成功后，在目标同目录创建独占临时文件、写入并同步、rename 替换，再同步目录。
失败前保留旧目标；rename 后目录同步失败可能已留下完整新文件，但仍报告失败；绝不发布部分字节。

ADR 0054 的共享声明只保存主文件和构建偏好。最后 PDF 目标是本机绝对路径，依照数据模型的本机路径边界，
保存到用户配置目录 `scholium/project-settings/<project-id>.json`，不增加共享声明字段或版本。
新身份的另存为项目不继承旧项目的本机目标。`SCHOLIUM_EXPORT_SETTINGS_DIR` 可隔离测试配置。
目标记忆失败不撤销完整 PDF，报告警告。跨设备、并发本机实例和系统原生选择器不是该设置的保证。

## 验证合约

相同 fixture 的产品导出与直接 fork typst-pdf、或宿主相同 LaTeX 引擎输出，均通过 hayro 在固定
300 dpi 比较每页尺寸、页数与 RGBA。容差为零：相同字体、同引擎和同一栅格器没有合理像素差异来源；
元数据时间差不参与比较。不得为未来升级放宽容差，差异必须定位到真实字体、排版或引擎变化。
