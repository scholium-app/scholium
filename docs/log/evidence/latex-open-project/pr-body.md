## 问题

项目文件流程打开 LaTeX 时仍显示占位区；真实工作台只接受独立启动变量，固定 main.tex，未保存输入没有进入项目草稿。

## 改动

将 New/Open 的 LaTeX 项目装配为真实工作台，使用声明入口与 SourceProject 作为唯一可写正文。编辑保存 draft，Save 复用 materialize 与外部冲突 UI；重载/另存为重建派生预览。移除 SCHOLIUM_LATEX_PROJECT，统一使用 SCHOLIUM_PROJECT_PATH。补齐模态输入保护和源码原生撤销路由，更新双语窗口检查、模块文档与开发日志。

## 验证

所有常规 Rust/文档/分层/许可证/构建门禁通过；workspace 627 passed、13 既有 ignored。stock Typst 系统/bundled 字体各 31 个 fixture 像素一致。双语窗口首轮 52/54，两项补跑通过，合计 54/54：一项被外部终止 Xvfb，另一项使用修复前启动的旧撤销路由进程。双语项目窗口验证 Save→编辑→SIGKILL→重开草稿、声明入口、冲突重载、New/Open 文件与目录。本机额外验证 LuaLaTeX 实时与 XeLaTeX 整编预览。证据见 docs/log/2026-11-03-latex-open-project.md。

## 待办

本任务无未完成项。工具链仍为可选配置；CI 保持验证明确不可用状态。未修改依赖、存储格式、Typst 参考实现或 CI 配置；未创建 PR。
