# SPK-0062：结构源码工作区与 A4 快速预览

- 日期：2026-10-11；结论：**Pass**。
- 对应：[ADR 0037](../adr/ADR-0037-remove-legacy-editor.md) 第 4 条（旧模型预览栈随结构源码工作区删除）、
  [P1 排版工作区规划](../plan/P1_UI.md)的源码与预览分屏、阶段 0 条件 3/4 的源码与后台预览能力在新编辑面重建。
- 基线：main `3142b07`；分支 `ation_ciger/feat/structured-source-view`。

## 改动

- `scholium-typst::generate_source(&StructuredDocument, SourceMode)`：正文字面转义，粗体/强调用
  `#strong[..]`/`#emph[..]`（Typst 对两侧都是字母的 `*`/`_` 不作标记），标题 `=`/`==`，数学文字为字面量
  （单字母/纯数字裸写，其余加引号），分数 `(a)/(b)`，必填空槽 `□` 计数。Unicode 换行类字符写成
  `\u{..}`，块首 `1.` 转义，避免改变结构。`Preview` 模式加 preamble 与块锚点，原串公式换成可见占位，
  从不在进程内编译。
- `PreviewCompiler::submit` 只接收源码与调用方单调 generation；删除旧 `DocumentSnapshot` 投影、
  `edit_compile`（输入期公式反馈）、`geometry`（旧页面编辑器字形几何）和 `submit_snapshot`。
  `spikes/render-latency` 改为“迁移为结构文档后生成”，锁文件只因 `thiserror` 精确版本变化。
- 应用源码工作区（Ctrl+2）：左侧只读生成源码，光标块高亮并滚到视野；右侧 A4 预览，首次进入才启动
  worker，按快照身份提交新 generation、只采纳当前 generation，编译后翻到光标块所在页；重编译期间保留
  上一页但不响应点击；点击预览把光标移到该块首个正文位置。源码与高亮的排版结果按（快照、高亮）缓存。
- 实机脚本 `spikes/typst-main-editor/source-view-check.py`，并入 CI 窗口作业。

## 代码评审后修正

- 首轮删除 `docs/modules/typst.md` 旧段落时切片越界，误删了直接 Content 接口、不变量、失败处理、
  测试门禁、团队合约与 WASM 章节；已从 HEAD 恢复，只移除真正过时的旧预览与输入期反馈说明。
- 词内粗体/强调改为函数形式；补 `(`/`)`、块首 `1.`、Unicode 换行的字面化，并以真实编译验证锚点数不变。
- 评审怀疑块末锚点在末行顶部、点击末行下半会落到下一块。实测非末块的结束锚点等于下一块起点
  （块底边），现有规则正确；新增多行段落末行点击的回归测试固定该结论。
- 每帧不再克隆整份源码与重建 LayoutJob；点击定位优先正文叶；CI 步骤限时。

## 复现

```sh
cargo +1.96.0 test --locked --workspace
cargo +1.96.0 build --locked -p scholium-app -p scholium-storage --bins
xvfb-run -a -s '-screen 0 1280x900x24' \
  python3 spikes/typst-main-editor/source-view-check.py target/debug/scholium-app /tmp/scholium-source-view
cargo +1.96.0 check --locked --manifest-path spikes/render-latency/Cargo.toml
```

## 结果

| 检查 | 结果与证据 |
|---|---|
| 回归 | **160 passed / 0 failed**；生成器 7 项（映射与转义、数学字面、原串不执行、词内样式/括号/列表/换行、真实编译、锚点与多行末行点击）与源码工作区 5 项；[tests.log](evidence/SPK-0062/tests.log) |
| gates | fmt、clippy、rustdoc、文件规模、licenses/bans/sources；`render-latency` 以 `--locked` 编译 |
| 源码工作区实机 | **5/5**：编辑后 Ctrl+2 显示生成源码与 A4 预览（1 页、无错误）、点击预览第二段定位光标、Ctrl+1 后键入落在该段、预览跟随新快照；[summary.json](evidence/SPK-0062/source-view/summary.json)、[截图](evidence/SPK-0062/source-view/02-source-and-preview.png) |
| 既有窗口流程 | 33 步、迁移检查与 AT-SPI 9/9 仍通过 |
| advisories | **Fail**，仍是既有 7 项；[advisories.log](evidence/SPK-0062/advisories.log) |

## 未覆盖

源码只读、不 reconcile；预览不是最终构建（无 LaTeX、无 PDF 导出）。数学多字符文字按字面直立显示，
与编辑面个别排版细节（斜体、运算符间距）可能不同；原串公式需转换后才能编辑与预览。
编辑面本身仍是受控不分页流。阶段判定不升级。
