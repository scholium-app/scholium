# T40：LaTeX 图元预览与源码工作台

基线为 main `4bb9808ecf79aca8e17d7ba5247f4603688e8b8e`，工作槽为 scholium-wt3，分支为
`ation_ciger/feat/latex-view`。按 T40 例外推送分支，不创建 PR。

实现了内核 Drawing/RasterPage、300 dpi display-list 栅格、pdftex.map/encoding 的 Type 1 字形解析、
hayro 同引擎 PDF 回退、XeLaTeX/pdfLaTeX 沙箱整编、共享源码编辑器与开发工作台。
所有路径保持源码权威；Typst 的生成源码仍只读。决策与规范见
[ADR 0052](../adr/ADR-0052-latex-view-workbench.md)、[latex](../modules/latex.md)、[build](../modules/build.md)、
[render](../modules/render.md)、[app](../modules/app.md)。

## 复现与修复

- mailbox 的 Diagnostic/Location 满队列时静默淘汰：先运行失败测试，再加入饱和计数和排空后的汇总诊断。
  100+100 条输入保留 32 条，分别报告 84 条诊断和定位回复丢失；最多再占一条汇总槽。
  证据：[RED](evidence/latex-view/red-mailbox.log)、[GREEN](evidence/latex-view/green-mailbox.log)。
- 段落仅按基线移除旧行会误删同基线另一栏：像素测试先失败，再改为横坐标+基线匹配，失败事务回滚场景。
  页面增量在监督层累积为完整 Drawing 后才合并；栅格线程共享完整 RGBA 页集，使 UI 丢弃旧 revision 时仍保有依赖。
  证据：[RED](evidence/latex-view/red-scene.log)、[GREEN](evidence/latex-view/green-scene.log)。
- IME 原生撤销可能与前次粘贴合并：单独预编辑缓冲与提交撤销点，使预编辑不进入源码权威。
  补全经 TextEdit 输入路径插入，复用 math-input 候选；测试覆盖 frac/alpha、选区、IME、撤销/重做、UTF-8 查找和长文本滚动。
- 首次 XeTeX 默认 PDF 外部命令路径在无 shell 白名单内失败；改为 `-no-pdf` + 显式沙箱 xdvipdfmx。
  libpaper 的 paperspecs/papersize 为固定只读配置。引擎超时测试使用真正无限 TeX 循环，要求宿主 Timeout，3 秒内回收。
- 原生复测发现 inline-math 边界 `m` 是无墨迹元数据：显式忽略此已知类型，未知绘制记录仍触发 PDF 回退，增加回归测试。
- 首轮窗口脚本在窗口映射前聚焦触发 X11 BadMatch；改为等待 onlyvisible 窗口，不修改既有检查断言。

## 产品 E4 / E7

原生验收命令（未提供依赖时测试明确失败，常规 CI 为 ignored，不把未运行计通过）：

```sh
export SCHOLIUM_RTEX_BIN="$HOME/.cache/scholium-eval/rtex-531e2c6b32a09d33be58f6be1301fe8f793411cd/target/release/rtex"
export SCHOLIUM_RTEX_TEXDIR="$HOME/.cache/scholium-eval/rtex-531e2c6b32a09d33be58f6be1301fe8f793411cd/tex"
export SCHOLIUM_RTEX_FONT_CACHE="$PWD/target/t38-native-font-cache"
export SCHOLIUM_LATEX_EVIDENCE="$PWD/docs/log/evidence/latex-view/native"
cargo test --locked -p scholium-latex --test render_native -- --ignored --nocapture --test-threads=1
```

使用产品 launcher、产品 Renderer 与独立 `pdftoppm -r 300 -gray`。E4 墨迹阈值 96，双向 1 像素邻域，
未匹配墨迹比例 ≤1%；没有放宽阈值。参考 PNG 小夹具由 Pillow 独立绘制，未调用产品渲染器生成参考。

| 夹具 | 来源 | 未匹配墨迹 |
| --- | --- | --- |
| English initial | display list（含 Type 1 数学） | 0% |
| English edited live | 段落 display list | 0% |
| 中文 initial | display list | 0.0030% |
| 中文 edited live | 段落 display list | 0.0059% |
| TikZ | hayro PDF fallback | 0% |

两项 ignored 原生测试均通过，详见 [native.log](evidence/latex-view/native.log) 与
[逐页数据](evidence/latex-view/native/chinese-edited-live.json)。XeLaTeX 和 pdfLaTeX 均编译未保存 revision 1、
返回 FullCompilation 来源与耗时、保持磁盘原文，并验证宿主截止取消；旧 revision UI 注入测试拒绝结果。

## 真实窗口与 E6

使用 `scripts/window-checks.py` 的机器级锁与私有 Xvfb，未访问用户桌面。
新 `latex-view-check.py` 总是移除 rtex 配置并实际断言不可用状态、0 页、源码补全与撤销。
设置 `SCHOLIUM_LATEX_NATIVE_WINDOW=1` 时额外验证 LuaLaTeX display list 与 XeLaTeX FullCompilation。

```sh
SCHOLIUM_LATEX_NATIVE_WINDOW=1 python3 scripts/window-checks.py --locale zh-Hans en --out target/t40-windows-all
```

E6 从 xdotool 击键发出开始，到 XGetImage 首次观察到右侧预览实际像素变化结束；裁剪区排除源编辑器、状态栏和滚动条。
每个 locale 20 次单字符编辑、2 ms 轮询，等待 revision 采纳且为 Paragraph 更新后再下一次输入。
使用 CLOCK_BOOTTIME 与 monotonic 差值检查休眠（超过 1 秒必须丢弃），不把 UI submitted_ms 或协议 elapsed 当呈现。
首轮单独 en 检查的 p50=45.7 ms、p95=53.7 ms；完整双语窗口检查 **48/48 通过**（每种 locale 24 项，含 source-view/source-theme 与 a11y）。
[窗口汇总](evidence/latex-view/windows/all.log)记录全部结果；下表来自各 locale 的原始 20 次测量：

| locale | 击键数 | p50 | p95 |
| --- | --- | --- | --- |
| zh-Hans | 20 | 56.5 ms | 73.5 ms |
| en | 20 | 46.9 ms | 63.3 ms |

截图：[中文实时](evidence/latex-view/windows/zh-Hans/latex-live.png)、[英文整编](evidence/latex-view/windows/en/latex-xelatex.png)、
[不可用状态](evidence/latex-view/windows/en/latex-unavailable.png)。

## 门禁与交付边界

常规 workspace 测试 614 passed、13 ignored（ignored 不计入通过；本任务新加的两个原生验收另行实际执行）。
全部常规门禁逐命令、耗时和原始输出保存在 [gates.json](evidence/latex-view/gates.json)。
系统字体及仅 bundled 字体的 stock Typst 两套独立像素对照均通过，见 [stock.json](evidence/latex-view/stock.json)。
未修改独立参考的夹具或阈值，Typst 结构输入/渲染行为不变。

共享文件只追加 Cargo 依赖/lock、i18n keys、ADR 索引与原生依赖登记；`main.rs` 只增加 env 入口与模块声明。
`i18n/tests.rs` 最小追加 9 个 key 注册，配合原有“无未使用目录项”断言，未调整断言。
未改文件命令、工作区持久化、Loro、路线图或阶段状态；未创建 PR。

开发入口固定读取 main.tex；源码编辑保留内存，文件菜单/保存由 coordinator 后续装配。
预览预算为每页 2400 万、总计 9600 万像素、PDF 64 MiB/256 页；超限明确失败。
FullCompilation 仍要求基础 LuaLaTeX 参与固定可信字体缓存预热，完整 TeX 安装为当前宿主依赖。
