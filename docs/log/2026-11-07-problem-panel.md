# 问题面板与诊断跳转

T47 将 LaTeX live/full、Typst 源码沙箱与原生 generated-source 诊断接入同一 revision-bound `DiagnosticBatch`。
面板提供文件分组、选中行、原始构建输出、Enter/F8 跳转与源码波浪线/gutter 标记。LaTeX 多文件跳转在已有项目锁下保留各文件私人草稿，声明的构建入口独立。

## 复审复现与修复

首次交付只跑了两个相关窗口检查，未满足完整窗口门禁；此前将原生检查超时归为环境问题也缺少证据，此处更正。
复审先保留了两轮私有 Xvfb 双语运行。第一轮两种语言均完成主题切换，AUDIT 含 `view.theme.light`、`view.theme.dark`，
`file_modal=false`。实际失败发生在修复源码后的重编译：当前资源同时出现在草稿列表与单独当前输入中，
worker 基于同一旧快照发送重复补丁，编辑器中 `Fixed` 在引擎中变成 `Fixed}`，编译报 Too many }'s，预览无法追上。
第二轮也复现了重复补丁；扩充窄窗口截图时，脚本另有错误：480 px 的源码工具栏仍可能放得下，不能把 toolbar overflow 当作窗口尺寸同步信号。
后续改为等待实际 source-pane 几何。

修复按资源去重输入，当前输入优先，每个文件每次只发送一个补丁。失败回归为输入数 3 而非 2，修复后通过，
并覆盖旧草稿不能覆盖当前输入。面板测试还验证：有选中问题、打开问题面板时，主题子菜单仍列出两个主题并能切换。
截图复查修正了 gutter 图标误传的 10 px 描边（现为 1 px），文件组箭头改用已有 Chevron 图标，避免字体缺字方框。
末尾 X connection 消息发生在 runner 清理 Xvfb 时；它不能证明 Xvfb 导致此前失败。
复现详情见 [压缩审计证据](evidence/problem-panel/reproductions.json)、
[失败回归](evidence/problem-panel/red-test.txt)、[通过回归](evidence/problem-panel/green-test.txt)。

## 截图来源

before 使用 `aa6ad1397df410f9968f5e55aed62d875071c9cc` 的完整 app 源码重建；
链接的 storage/latex 仍为 T47 最小接线修改，不影响 before 的 UI。after 使用本次修复版。
二者使用相同 LaTeX 文件，包含 undefined citation 与 `\foo`，覆盖 zh-Hans/en、light/dark、1280×900 与 480×900。
before 如实保留旧面板没有编译问题行、不可选中问题行的状态；after 显示选中行、两种严重度 gutter 标记、行列与窄状态计数。
具体文件、版本和哈希见 [截图清单](evidence/problem-panel/screenshots.json)。

## 验证

复审版 `python3 scripts/window-checks.py --locale zh-Hans en`：`62/62` 通过，31 项各在 zh-Hans 与 en 执行一次；没有失败或跳过。单独 LuaLaTeX native problem-panel 检查在两个 locale 的原生检查均通过，含修复清除、真实多文件跳转、主题切换、窄屏以及 F8。截图为 light/dark × 1280/480 × zh-Hans/en 的 before/after 共 16 张。完整窗口输出留在任务验证目录；摘要见 `evidence/problem-panel/window-totals.json`。

所有 00-conventions cargo、Rust 文档/规模、链接/UI 字符串/layer、cargo-deny/build 门禁通过；workspace tests 为 725 passed、0 failed、16 ignored。Typst stock 对照的 system 与 bundled 字体配置均通过。日志与截图均来自私有 Xvfb，不操作真实桌面。

## 范围例外

storage 的 `active_path` / `select_resource` / 派生草稿输入与测试是协调者批准的最小多文件跳转接线；
LaTeX supervisor 仅增加 edit 前加载目标资源，在线/离线分支一致。本次复审修复留在 app worker，不新增内核 API。
没有创建 PR；由协调者审查后创建。
