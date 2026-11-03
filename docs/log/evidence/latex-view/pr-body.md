## 问题

LaTeX L1 尚缺可绘制预览和源码工作台；XeLaTeX/pdfLaTeX 只有路由标识。事件队列满时诊断与定位回复会被静默挤掉。

## 改动

- 内核自有 Drawing/RasterPage、300 dpi 图元绘制、pdftex.map 的 Type 1 字体解析；TikZ/无法表达的页面使用同引擎 PDF 经 hayro 回退。
- build 增加 FullCompilation profile；不可变资源快照、XeLaTeX XDV→xdvipdfmx/pdfLaTeX 整编、宿主 deadline、来源/耗时/revision 标识与过期结果拒绝。
- 抽取共享源码布局/编辑组件，保留 Typst 生成源码只读行为；LaTeX 左源码右预览，IME 预编辑隔离、查找/诊断/高亮接口，复用 math-input 的源码补全。
- 队列溢出计数汇总；段落更新事务回滚和完整页面累积，避免合并/丢弃旧 revision 丢失像素。
- ADR 0052、模块规范、原生依赖登记及开发日志；启动只追加 SCHOLIUM_LATEX_PROJECT 小分支，未改文件命令/持久化。

共享文件最小追加 Cargo/lock、i18n、ADR 索引和原生依赖登记。额外在 i18n/tests.rs 注册 9 个新增 key，满足现有无未使用目录项断言；没有修改断言。

## 验证

- conventions 全部门禁通过：fmt、clippy -D warnings、workspace tests（614 passed / 13 ignored）、rustdoc -D warnings、Rust size/doc links/UI strings/layers、cargo deny、bins build。
- 真实窗口中英文 48/48；原 Typst source-view/source-theme 无回退。新检查实际断言缺工具链状态，另验证本机 LuaLaTeX 与 XeLaTeX 预览。
- 两项新 ignored 原生测试实际执行通过：中英文段落编辑、TikZ fallback、XeLaTeX/pdfLaTeX 整编及宿主超时取消。E4 最大未匹配墨迹 0.0059%（阈值 1%，1px 容差）；小 display list + 独立参考 PNG 及假 rtex 事件→像素测试通过。
- E6 实际窗口 20 次/locale：zh-Hans p50/p95=56.5/73.5 ms；en=46.9/63.3 ms。测量起点为击键发出、终点为 XGetImage 预览像素变化；非 UI 提交/协议耗时。
- 系统字体与 bundled-only 两套独立 stock Typst 像素对照通过。
- mailbox 与同基线另一栏误删均保留先失败、后通过的测试证据。

证据见 docs/log/2026-11-03-latex-view.md、docs/log/evidence/latex-view/。

## 待办

- coordinator 接入正式文件/工作区入口与保存；开发入口当前固定 main.tex，未保存编辑仅保留内存。
- FullCompilation 的可信字体缓存预热仍依赖基础 LuaLaTeX；当前要求完整 TeX 安装，不自动下载宏包。
- 仅推送分支，按任务要求未创建 PR。
