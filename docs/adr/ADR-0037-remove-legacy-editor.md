# ADR 0037：删除旧页面编辑器并合并开发构建

## 状态

接受。取代 [ADR 0036](ADR-0036-typst-default-editor.md) 第 1 条中的开发 manifest 与第 2 条的
`--legacy` 入口；ADR 0036 的依赖、默认文档与迁移决定继续有效。

## 背景

ADR 0036 让 Typst Content 编辑器成为默认启动，旧页面编辑器只经 `--legacy` 保留。旧编辑面
以全文 markup 编译页面并用 egui 文字回显掩盖延迟，与“已提交内容只由 Typst 绘制”冲突；
保留它意味着两套编辑语义、两套测试和两份构建（根 workspace 与 `dev/typst-editor`）并存，
且 `--legacy` 保存的修改不会再进入已迁移的默认文档。

## 决定

1. 删除旧编辑面：`page_editor`、`SessionBridge`、`workspace`、`preview`、`paper`、`sample`、
   `native_text`、`undo` 及其测试，和只驱动旧界面的脚本（体验审计、Ribbon 截图、冒烟）。
   应用只有一个编辑面 `typst_editor`；`--legacy` 与 `typst-editor` 特性一并移除。
2. 删除 `dev/typst-editor` manifest。fork 像素对照改为 `scholium-typst` 的
   `typst-main-fork-check` 示例，来源校验脚本移至 `scripts/verify-typst-fork.py`；
   独立 stock 参照 manifest 保留在 spike 目录，以免与 patch 后的版本组混用。
3. 迁移所需的旧格式读取（存储层 `SessionStore` v0、`migrate_legacy_file`、`legacy_has_head`）
   保留；默认路径首启迁移不变。无法迁移时只提示移走旧文件，不再提供旧编辑器入口。
4. `scholium-typst` 中面向旧 `DocumentSnapshot` 的源码生成与预览编译（`PreviewCompiler`、
   块锚点、字形几何、未完成公式反馈）不是编辑面或回显代码，暂不删除：应用不再调用，
   `spikes/render-latency` 仍以其构建。结构源码工作区/预览接入时移植或替换，随后删除。
5. 旧编辑面已验收而新编辑面尚未具备的能力（正文无障碍 Text 接口与数学播报、分页与纸张、
   生成源码工作区与快速预览、未完成公式原位诊断、阶段 0 远端动作注入）在新编辑面上重新实现
   并重验，不恢复旧代码。

## 代价

上述能力在重新实现前不可用；依赖旧界面的历史 spike 复现脚本（如 `spikes/typst-mapping/review-*.py`）
不再可运行，其报告只作追溯。验证见[报告 0060](../spikes/SPK-0060-remove-legacy-editor.md)。
