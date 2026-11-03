# scholium-math-input

## 职责

提供与 UI、Typst、LaTeX 无关的数学输入规则与共享验收词汇，依据 [ADR 0048](../adr/ADR-0048-shared-math-input-keymap.md)。

## 非职责

不持有文档、光标身份、撤销栈、排版场景、平台键位或源码；不执行用户文本。

## 内部结构

`data` 从嵌入的 `core.toml` 读取符号、组合、循环、结构槽与命令名。表由本项目依据 Unicode 与公开排版文档独立编写，
不提取 GPL Scheme 表。`engine` 只保留临时命令 query 和刚键入的候选 token。`accept` 定义按键序列与归一化公式树。

## 公共接口

`Engine::feed(Event, Context) -> Intent` 接收抽象事件及只读数学上下文，返回字面插入、符号、向前替换 UTF-8 字节、结构或槽位导航意图。
`tab_action` 与实际 Tab 使用同一判定：命令补全优先，其次未中断的候选循环，否则前进槽位；Shift 反向。
`candidates` 供 UI 生成浮层；`interrupt/reset` 在导航、其它动作或权威文档切换时清理临时状态。
`accept::Adapter` 由内核以真实文档实现，运行 `accept::cases()` 比较 `Tree`，后续 LaTeX 适配器复用同一组夹具。

## 不变量

引擎没有文档。`Context.before` 和 Replace.bytes 都以叶内 UTF-8 字节计，不是字符序号。
符号有两种语言名称或明确 unsupported 标记；组合无前缀冲突；循环组不重叠且前后向闭合；命令引用已登记符号或模板。
候选框是 UI 浮层，接受的文字只经对应项目语言的排版器绘制。

## 失败处理

数据一致性由测试拒绝；嵌入表损坏是构建错误。未知命令前缀不产生编辑，Esc 关闭浮层并保持打开前文档原文。
文档适配器负责验证与原子回滚，IME 提交及粘贴作为字面文字绕过组合规则。

## 测试门禁

数据引用与前缀冲突测试、Tab 三分支与双向循环测试、命令取消测试，以及 Typst 适配器运行共享公式树夹具。
工作区层级检查禁止 UI 或任一语言内核依赖进入此 crate（层 1）。
