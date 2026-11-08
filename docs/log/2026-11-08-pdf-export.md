# 2026-11-08：两内核共用 PDF 导出

本批在 `scholium-wt1` 的 `ation_ciger/feat/pdf-export` 实现了已打开原生 Typst、普通 `.typ` 和
LaTeX 的统一 `file.export`，默认 Ctrl+Shift+E 仍通过可配置键表路由。目标表单、后台任务、
状态栏与问题面板使用同一个导出报告，报告记录当前 host revision 和包含未保存编辑与否。

## 改动记录

- 原生输出编译当前提交快照的 A4 预览生成源码，使用相同字体/排版，拒绝 RawMath 和未填槽。
  普通 `.typ` 复用了原有 TypstPreview worker，只增加 PDF 产物；未改 build 挂载/资源限制。
- vendor 补入同一上游 pin 的未修改 typst-pdf；源重建校验清单增至 14 个库。
- LaTeX 增加独立 full::export_pdf，支持三引擎；预览仍拒绝 Lua 的一次性入口，live 路由没有改变。
  bibliography 没有运行，状态栏报告悬停与结构报告都明确保留这一边界。
- 完整 PDF 经 hayro 验证后，同目录临时文件写入、同步并 rename；编译/验证/发布前失败保留原目标。
- 最后成功目标按项目 ID 存在用户配置的本机项目设置，避免把绝对路径写入 ADR 0054 共享声明；
  该裁决见 [ADR 0056](../adr/ADR-0056-pdf-export.md)。没有升级项目存储版本。
- T50 所有的 `latex_workspace/mod.rs` 仅增加当前 revision 的只读 accessor，并把现有
  preview_source 暴露给同 crate 导出编排；未改其 live/session/supervisor，未触碰 T51 所有文件。

## 验证记录

[命令红灯](evidence/2026-11-08-pdf-export/command-red.log.gz)复现了 file.export 不在有效注册表；
随后同一测试与全部内核可见性、无文档禁用检查通过。
[非活动 LaTeX 成员红灯](evidence/2026-11-08-pdf-export/member-red.log.gz)复现了切回干净入口后，
报告遗漏其它成员未保存草稿的问题；修复后比较全部导出 overlay 与磁盘，回归测试通过。[目标记忆红灯](evidence/2026-11-08-pdf-export/settings-red.log.gz)还复现了大写 `.PDF`
成功导出却不被恢复的问题；修复并加入回归。

[Typst PDF 比较](evidence/2026-11-08-pdf-export/typst-pdf-parity.log.gz)的三个测试验证了
生成源码 API、显式资源 API 与直接 fork typst-pdf 的逐页像素一致，以及语法错误/未填槽拒绝。
app 常规测试补充了真实文件发布、当前未保存修订、字面文字/结构分式、旧目标保留、symlink 拒绝、
临时文件清理与诊断去重/过期。两条比较路径都用了 hayro 的固定 300 dpi，容差为零；
元数据时间不参与比较，没有放宽断言。

[完整测试](evidence/2026-11-08-pdf-export/test.log.gz)通过 752 项，失败 0 项，17 项显式 opt-in 测试
在常规门禁中跳过。任务新增的[三引擎 opt-in 验收](evidence/2026-11-08-pdf-export/latex-native.log.gz)另外在本机执行，
1 项测试覆盖 pdfLaTeX/XeLaTeX/LuaLaTeX，三份 fixture 均为 1 页、300 dpi 下 RGBA 完全一致，
源文件在未保存输入导出前后保持原字节。
[clippy](evidence/2026-11-08-pdf-export/clippy.log.gz)、
[rustdoc](evidence/2026-11-08-pdf-export/doc.log.gz)、
[二进制构建](evidence/2026-11-08-pdf-export/build.log.gz)及
[依赖策略](evidence/2026-11-08-pdf-export/deny.log.gz)通过；cargo deny 只留下原有未遇到许可证允许项提示。
[fmt、Rust 文件规模、文档链接、UI strings、layers 脚本](evidence/2026-11-08-pdf-export/static-gates.log.gz)也通过。

独立 stock 对照分别在[系统字体](evidence/2026-11-08-pdf-export/stock-comparison.json)与
[打包字体](evidence/2026-11-08-pdf-export/stock-bundled-comparison.json)下各 31/31 逐像素相同。
没有改变编辑面排版，独立参考源码无需更新；
[源清单摘要](evidence/2026-11-08-pdf-export/fork-source.json)确认新增 PDF crate 完全来自原 pin。

[首次 PDF 窗口检查](evidence/2026-11-08-pdf-export/window-initial.log.gz)中英文通过，
中文在应用启动前出现 XOpenDisplayFailed，未计产品通过。
[完整窗口门禁](evidence/2026-11-08-pdf-export/window-all.log.gz)在新目录最终通过 66/66，中文/英文各 33 项。
PDF 每语言 10 个断言分别见[中文](evidence/2026-11-08-pdf-export/window-pdf-zh-Hans.json)与
[英文](evidence/2026-11-08-pdf-export/window-pdf-en.json)，覆盖未保存正文和公式、报告版本、PDF 提取文字、
重启后目标记忆、沙箱源码成功及错误拒绝。截图为
[原生成功](evidence/2026-11-08-pdf-export/native-export.png)和
[源码失败](evidence/2026-11-08-pdf-export/source-export-error.png)。全程
遵守机器锁、私有 Xvfb、键表和 function-key keycode 约束，没有访问用户桌面。

## 边界

未实现 bibliography、打印、PDF/A 或选区导出。已有导出页数、文件/资源预算仍可能拒绝超大文档；
栅格预算只属于预览和像素比较测试。
本机路径设置不进入共享项目，也不随新身份的另存为继承。目录同步在 rename 后失败可能留下完整新 PDF，
但报告仍为失败，不承诺“不产生任何完整产物”。其它 OS 的沙箱与系统原生选择器未在本批验收。
未开 PR，未轮询 CI；由协调者起草 PR。


## Review follow-up：导出只解析一次 PDF

协调者指出 LaTeX 导出先为计数、再为验证，两次在产品密度下栅格化完整 PDF；Typst 也走了后一次。
本次移除全部导出 worker 栅格调用，三类产物统一在发布前经 hayro-syntax 打开一次、读取页数，
并直接从解析器持有的原字节发布，不复制 PDF buffer。LaTeX 不再提前计数，Typst 保留编译器页数校对。
64 MiB 与 1–256 页检查、诊断、同目录原子发布等其它行为保留；没有执行页面解码或性能基准。

[失败先行记录](evidence/2026-11-08-pdf-export/review-red.log.gz)中两项回归均失败：worker 调用了完整
预览栅格器，100 页 PDF 被预览像素预算拒绝。100 页夹具刻意使用大页面，让误调用在分配像素前失败；
修复后同一发布路径成功导出/计数 100 页，并覆盖无引擎页数提示、错误页数提示、损坏 PDF 保留原目标。
另有源码边界检查禁止 worker 重新调用完整栅格器。既有像素一致性测试继续保留 300 dpi、零容差。

[格式](evidence/2026-11-08-pdf-export/review-fmt.log.gz)、
[workspace clippy](evidence/2026-11-08-pdf-export/review-clippy.log.gz)、
[app/LaTeX 测试](evidence/2026-11-08-pdf-export/review-test.log.gz)通过（446 passed、0 failed、9 ignored）。
[重建 app](evidence/2026-11-08-pdf-export/review-build.log.gz)通过，
[三引擎显式原生验收](evidence/2026-11-08-pdf-export/review-latex-native.log.gz)另外通过 1 项，
三引擎均为 1 页、300 dpi、零 RGBA 差异。

[PDF 窗口复验](evidence/2026-11-08-pdf-export/review-window.log.gz)遵守机器锁等待后通过 2/2，
[中文](evidence/2026-11-08-pdf-export/review-window-zh-Hans.json)与
[英文](evidence/2026-11-08-pdf-export/review-window-en.json)各 10 个断言全部通过。
本次按协调者限定的受影响门禁复验，没有重跑全套窗口/stock 比较；未开 PR、未轮询 CI。


## Review follow-up：长论文与图片 PDF 上限

协调者指出 256 页 / 64 MiB 会拒绝正常论文。此批把发布路径改为命名的 100,000 页 / 2 GiB
病态输出保护上限，注释说明沙箱 /work 配额才是实际构建限制。向前追踪还发现 Typst 导出复用了
100 页预览限制、worker PDF 读取与 LaTeX 输出读取仍为 64 MiB；一并改为 PDF 专用宽松上限，
预览页数/像素限制、输入资源预算和沙箱挂载/配额不变。

[300 页回归红灯](evidence/2026-11-08-pdf-export/bounds-red.log.gz)先复现了 Typst 的旧页数限制；
修复后[同一生成源码和发布测试](evidence/2026-11-08-pdf-export/bounds-focused.log.gz)通过，
验证完整 PDF 计数 300 页且目标文件存在，没有为测试栅格化全部页面。
[fmt](evidence/2026-11-08-pdf-export/bounds-fmt.log.gz)、
[workspace clippy](evidence/2026-11-08-pdf-export/bounds-clippy.log.gz)及
[app 测试](evidence/2026-11-08-pdf-export/bounds-test.log.gz)通过（394 passed、0 failed、1 ignored）。
没有生成 2 GiB 夹具或运行性能基准；按协调者要求未重跑窗口检查。未开 PR、未轮询 CI。

因修改了 PDF 生产者和输出读取，[Typst/LaTeX 测试](evidence/2026-11-08-pdf-export/bounds-kernels.log.gz)
也通过（90 passed、0 failed、10 ignored）。
