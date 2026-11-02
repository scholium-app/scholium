# ADR 0036：Typst Content 编辑器成为默认启动路径

## 状态

接受。取代 [ADR 0034](ADR-0034-typst-main-app-candidate.md) 第 1、3 条及其“安全门禁不通过前
不将 fork 合入默认发行依赖”的限定；ADR 0034 其余决定（单一权威、同源 Scene、历史）继续有效。
第 1 条的开发 manifest 与第 2 条的 `--legacy` 入口已由 [ADR 0037](ADR-0037-remove-legacy-editor.md) 取代。

## 背景

普通启动走旧页面编辑器：击键先由 egui 文字回显画出，全文 Typst 编译完成后再替换为页面像素。
已提交正文因此存在两套绘制，违背[主程序接入计划](../plan/TYPST_MAIN_APP_INTEGRATION.md)
“已提交正文和结构公式只由 Typst 绘制”的目标。显式候选入口已在主程序内闭环（报告 0054–0058），
release 构建中单段到 80 段文档的 worker 布局为 0–11 ms，输入直接经 Typst Content 呈现。

ADR 0034 以依赖通告未处理为由不批准默认替换。核对发现 main 的**普通 registry 构建**同样
失败于完全相同的 7 项通告（同为 Typst 0.15.1 依赖树；证据
[main-stock-advisories.log](../log/evidence/typst-default-editor/main-stock-advisories.log)）。其中
bincode、paste 为“停止维护且无安全升级”，短期不可清零。该门禁并不区分两条路径，不能继续
作为保留回显路径为默认的理由。

## 决定

1. 根 workspace 以 `[patch.crates-io]` 统一使用已跟踪的 `vendor/typst-edit` 13 个库，与
   `dev/typst-editor` 同一版本组；来源仍由 `verify-fork.py` 从上游对象与四份补丁逐文件重建。
   `scholium-app` 默认启用 `typst-editor`，`scholium-typst` 的 `editor` 特性同时打开
   `typst-layout/editor`。
2. 普通启动进入 Typst Content 编辑器。旧编辑器只在显式 `--legacy` 时启动；默认路径失败
   时窗口内报错、不运行任何编辑器，不自动回退。`--typst-editor` 保留为无副作用参数。
3. 未设 `SCHOLIUM_SESSION_FILE` 时文档为 XDG 数据目录下 `scholium/document.v1.sqlite`。
   该文件不存在且同目录旧版 `session.scholium.sqlite` 有已保存 head 时，用
   `migrate_legacy_file` 一次性迁移到新文件；旧文件只读打开、保持原样作为可恢复副本，
   界面显示迁移提示和转为原串的公式数。已有 v1 文件不会被迁移覆盖；显式
   `SCHOLIUM_SESSION_FILE` 仍须是 v1。探测与迁移都以只读方式打开旧文件；迁移失败时窗口
   显示原因与恢复方式（`--legacy` 或移走旧文件），不启动任何编辑器。并发首启时由先发布
   目标文件者为准。迁移只发生一次：此后用 `--legacy` 保存的修改只写旧文件，不会同步进默认文档。
4. 依赖通告继续如实失败，不加 ignore，不把许可/来源通过当作安全通过；通告处理（升级或
   替换 Typst 依赖）作为独立工作跟踪。

## 代价与后续

旧路径独有的能力暂不在默认路径：分页与纸张设置、生成 Typst 源码工作区与最终预览、正文
无障碍文本、示例文档；旧公式迁移为 RawMath 后在新编辑器中报布局诊断，转换前不可编辑。
下一步删除旧编辑器与回显代码、合并开发 manifest，并按需移植上述能力；在此之前 `--legacy`
仍可打开旧数据。验证见[开发日志《普通启动切换为 Typst Content 编辑器》](../log/2026-10-10-typst-default-editor.md)。
