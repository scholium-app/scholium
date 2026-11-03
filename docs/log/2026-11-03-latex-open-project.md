# LaTeX 工作台接入项目文件流程

问题：项目文件流程仍把 LaTeX 装配成占位区；真实工作台只接受独立开发变量、固定 main.tex，编辑也没有接入持久化。

本次将 LaTeX ProjectHandle 装配为真实 Workspace，入口取项目声明。SourceProject 持有唯一可写正文，
编辑写私人 draft，Save 复用 materialize 与磁盘 hash 冲突检查；重新载入/另存为重建预览。
恢复草稿覆盖派生 SourceSession 的入口，初始内核 revision 映射回 UI revision 0。
独立 SCHOLIUM_LATEX_PROJECT 入口被删除，窗口检查统一使用 SCHOLIUM_PROJECT_PATH。
文件模态框禁用 TextEdit 交互及补全；源码撤销/重做快捷键保留给原生 TextEdit。

验证先用失败测试确认 LaTeX 仍装配占位区，再修复；接入后窗口检查也发现全局 Typst 撤销路由吞掉源码撤销快捷键，
增加失败测试后修复。回归覆盖 paper.tex 声明入口、私人 draft、保存、重开恢复、外部冲突、另存为与重载，
以及禁用源码框时对话框输入不被消耗。

验证结果与命令见 [门禁证据](evidence/latex-open-project/gates.json)；
[原始失败](evidence/latex-open-project/red-workbench.log)、
[撤销路由失败](evidence/latex-open-project/red-source-history.log) 记录了修复前行为。
系统字体与 bundled 字体各 31 个 fixture 的 stock Typst 像素对照全部相等，未调整阈值或参考实现。

源码改动位于 scholium-app：`project_workspace` 装配与测试、`project_files` 保存/冲突路由、
`latex_workspace` 正文/预览生命周期与测试、`source_editor` 模态输入保护与测试、
`command/shortcuts` 原生撤销路由与测试、main 的启动入口、状态栏及中英 catalog。
窗口脚本修改为 `latex-view-check.py` 与 `project-latex-check.py`；规范更新为 app.md / latex.md。
没有修改存储层、内核、依赖、参考实现或 CI 配置。

`cargo test --locked --workspace` 共 627 passed、13 ignored（既有可选原生/基准测试），没有新增 ignore。
`cargo deny` 通过，仅报告既有的未命中许可 allowance 警告。
全套双语窗口首轮 52/54：中文 caret-follow 被外部终止 Xvfb 打断，中文 latex-view 在修复前的旧进程中无法撤销；
修复后的英文 latex-view 与双语 project-latex 均通过。中文两项补跑 2/2 通过，合计 54/54。
额外启用本机工具链验证项目声明的 paper.tex：LuaLaTeX DisplayList 实时预览、20 次真实呈现检查和 XeLaTeX FullCompilation 预览均通过。
[完整首轮](evidence/latex-open-project/windows-all.log)、[补跑](evidence/latex-open-project/windows-retry.log)、
[窗口命令与环境](evidence/latex-open-project/windows.json)、[原生预览](evidence/latex-open-project/latex-view-native.json)
保留了失败与后续结果，不把首轮改写成全绿。
窗口均使用机器级锁及私有 Xvfb；未为外部终止 Xvfb 修改产品代码。
