# T51：总输出配额、宿主边界与共享源码查找

## 问题与变更

构建输出此前只有单文件限制。实现增加了内核执行的 tmpfs byte/inode 双配额，
把常驻状态和字体缓存副本放入同一会话卷，宿主只读取或复制有界普通输出。
裁决见 [ADR 0058](../adr/ADR-0058-output-quota-decoder-boundary.md)。

## 验证记录

Part A 先加入零预算和拷贝预算/链接拒绝回归，确认失败后实现。
真实 tmpfs 超字节与超 inode 对照通过；原 Typst profile 的文件/网络隔离、rlimit/cgroup、
输出超限、整树取消和 wall timeout 原生回归通过。

## 所有权交叉

`full.rs` 只新增 quota 错误到统一诊断的映射；`supervisor.rs` 只新增终止 quota 事件分支。
`typst_source/worker.rs` 只新增基础设施 quota 诊断。整编/导出和 live 调度其它行为归各任务所有。

## 宿主边界盘点

| 边界 | 限制 | hostile-input 回归 |
|---|---|---|
| rtex JSON-lines stdout/stderr | line 16 MiB、generation 64 MiB、host revision 32 MiB/4096 消息，channel 8 | malformed/truncated/unknown/oversized protocol；原 supervision 输出预算对照 |
| display-list / fragment / raster | 256 页、20000 rows、100000 items、128 fonts、坐标绝对值 ≤ 1e12；fragment 范围和长度；单边 8192、单页 24M/总 96M pixels | huge dimensions/offsets/page counts；现有 malformed scene 测试 |
| rtex 字体 | 只读系统可信字体路径、文件大小限制、128 faces/32768 outlines；TFM 名白名单 | 现有字体路径拒绝与 raster 测试；项目字体不进入宿主解析 |
| LaTeX PDF → hayro | 私有子进程、1 GiB/16 tasks/10 CPU + wall 秒；输入 64 MiB、256 页、page/scene pixels | decoder hostile frames（巨尺寸、零尺寸、缺失/截断）；无效 PDF；native render 回归 |
| decoder → host RGBA | 元数据 16 KiB；精确帧长度、排序/唯一页码、page/pixel 总预算 | hostile_decoder_frames_fail_before_allocating_pixels |
| Typst worker → host | 请求 16 MiB、元数据 1 MiB、100 页、8192 单边、64 MiB RGBA、diagnostics revision/count | worker 的现有预算校验加宿主维度与诊断身份检查 |
| LaTeX log | 4 MiB input、64 KiB line/message、1024 diagnostics、128 stack | pathological_log_is_rejected_without_building_unbounded_diagnostics |

Part B 的 plain hostile-input tests 通过；没有为此增加 fuzz 工具链依赖。

## 共享源码查找与替换

Typst Source Studio 与 LaTeX workbench 复用同一个查找栏和 literal-search model。
查询、前后跳转、计数、区分大小写、整词、替换与全部替换均通过共享源码状态工作；
快捷键来自 keymap，Esc 关闭并恢复编辑器焦点。可见匹配按主题 token 高亮。
扫描每帧最多处理 32768 个 Unicode scalar，查询上限 4096 bytes、匹配上限 100 万；
达到上限明确提示，扫描未完成或超限时禁止替换。多 MB 输入回归验证了逐帧上界。
全部替换通过原生 TextEdit undo boundary 作为单步撤销，Unicode 回归验证恢复完整原文。
Typst 原生页面编辑仍不提供 `edit.find`，本次没有实现页面搜索。

两套 stock-Typst 像素对照（普通字体发现与仅 bundled fonts）均通过，未修改独立 reference
或放宽阈值。实现按 A/B/C 三部分提交，随后补充分离的 hostile-input、窗口与原生配额测试提交；
这是相对于“三部分、三个提交”的提交数量偏差，保留了可独立审查的验证与修复历史。

首轮窗口验证发现 Replace All 的按钮释放帧会清除源码焦点，导致随后 Ctrl+Z 未送入源码。
补充真实 pointer-click 单元回归后，把焦点恢复移到 TextEdit 处理该帧之后；单步 undo 回归通过。
协议坐标检查曾误拒绝大整数 metadata；补充回归后只按字段语义检查标量与数组坐标。
源码预览布局的 missing-toolchain negative control 改用真实 detector 的 `SCHOLIUM_RTEX_BIN` override。
新缓存 native render 首次暴露预热缓存读取位置错误；修复为从受信固定文档的私有卷发布。
真实 rtex cancel 还暴露会话 socket/FIFO 导出问题；回归验证省略 IPC 对象但保留双预算和链接拒绝。
最终 native render 的两个测试通过：英文/中文 live 编辑、TikZ 隔离 PDF fallback、XeLaTeX/pdfLaTeX 整编，
所有 Poppler 对照均通过既有 1% 阈值，未放宽阈值。
追加 hostile-output nesting 回归后，宿主复制深度上限为 128；超过时有界错误退出。
Unicode 编辑和同帧高亮的回归先复现了 stale UTF-8 byte range panic，再以 snapshot 一致性守卫修复。

最终常规门禁通过：fmt、clippy `-D warnings`、workspace tests（756 passed / 18 ignored）、
rustdoc `-D warnings`、Rust size、doc links、UI strings、layers、cargo-deny 与 app/storage bins 构建。
allocator/PDF helper 也单独构建通过。byte/inode 各 profile 原生攻击与 Typst 原生隔离/取消回归再次通过。

## 最终窗口证据与接手边界

首轮全量双语运行 63/66：source-find 两语言 undo 焦点失败，zh-Hans source-preview-layout
的 missing-toolchain fixture 失败；修复及先红后绿单元回归见上文。定向双语复验 4/4 通过。
最终代码全量复验先完成 58 个成功组合，随后主机 `/tmp` user quota 耗尽（shell 也报
`disk quota exceeded`，runner 以 120 退出且日志截断）。只迁移本任务 evidence 到 `/home`，
释放约 3.5 GiB 临时配额；通过同一 machine-wide lock runner 重跑剩余英文 8 项，8/8、exit 0。
最终同一代码的 33 checks × 2 locales 全覆盖通过；没有把被中断的单次运行称为 66/66 exit 0。
未绕过锁、未并行窗口、未重试不明失败、未接触真实桌面、未放宽阈值。

原生 rtex E2 的 7 个 host-positive / sandbox-negative 攻击对照再次通过，
包括文件读取、绝对 input、shell、Lua/openout/项目写入与网络；实际 cgroup 预算不变。
本分支提供了阶段 1 接手门禁 item 4 的本地工程证据：总输出 quota 与宿主 decoder/IPC 边界审查完成。
没有编辑 SPK-0012；阶段状态的最终集成裁决仍由协调者记录。

Evidence 根目录：`/home/ation_ciger/.cache/scholium-eval/t51-window-evidence/`：

- `scholium-window-t51/`：首轮失败 evidence。
- `scholium-window-t51-fixed/`：定向 4/4。
- `scholium-window-t51-final/`：全量 58 个完成项及 host quota 中断。
- `remaining-en/`、`remaining-en-run.log`：剩余 8/8。
- `/tmp/t51-final-windows.log`：保留的中断 runner 日志。
- `/tmp/t51-final-workspace-tests.log`：756 passed / 18 ignored。
- `/tmp/t51-rtex-native-final.log`、`/tmp/t51-render-final/`：原生隔离与 rendering。
- `/tmp/t51-stock-comparison.json`、`/tmp/t51-stock-bundled-comparison.json`：每套 31 cases equal pixels。

所有权交叉还包括 `source-preview-layout-check.py` 的单行 detector override 名称修正，
其余 T50 工作区逻辑未改。未开 PR、未轮询 CI；分支由协调者起草 PR。

## 变更文件清单

```text
Cargo.lock
crates/scholium-app/i18n/en.ftl
crates/scholium-app/i18n/zh-Hans.ftl
crates/scholium-app/src/command/avail.rs
crates/scholium-app/src/command/context.rs
crates/scholium-app/src/command/default-keymap.toml
crates/scholium-app/src/command/edit.rs
crates/scholium-app/src/command/exec.rs
crates/scholium-app/src/command/spec.rs
crates/scholium-app/src/command/tests.rs
crates/scholium-app/src/command/ui.rs
crates/scholium-app/src/diagnostics/panel.rs
crates/scholium-app/src/project_files/mod.rs
crates/scholium-app/src/source_editor/find.rs
crates/scholium-app/src/source_editor/mod.rs
crates/scholium-app/src/source_editor/search.rs
crates/scholium-app/src/source_editor/tests.rs
crates/scholium-app/src/typst_source/worker.rs
crates/scholium-build/Cargo.toml
crates/scholium-build/src/bin/scholium-sandbox-fs.rs
crates/scholium-build/src/cache.rs
crates/scholium-build/src/launch.rs
crates/scholium-build/src/lib.rs
crates/scholium-build/src/process.rs
crates/scholium-build/src/quota.rs
crates/scholium-build/src/runtime.rs
crates/scholium-build/src/sandbox.rs
crates/scholium-build/src/tests/mod.rs
crates/scholium-build/src/tests/quota.rs
crates/scholium-build/src/tests/quota_profiles.rs
crates/scholium-build/src/tests/typst_native.rs
crates/scholium-build/src/typst_preview.rs
crates/scholium-build/tests/support/typst_worker.rs
crates/scholium-latex/src/bin/scholium-pdf-worker.rs
crates/scholium-latex/src/full.rs
crates/scholium-latex/src/log.rs
crates/scholium-latex/src/protocol.rs
crates/scholium-latex/src/supervisor.rs
crates/scholium-latex/src/tests/boundaries.rs
crates/scholium-latex/src/tests/mod.rs
crates/scholium-latex/src/transport.rs
crates/scholium-latex/src/view/mod.rs
crates/scholium-latex/src/view/pdf.rs
crates/scholium-latex/src/view/pdf_worker.rs
crates/scholium-latex/src/view/validate.rs
crates/scholium-model/src/diagnostic.rs
docs/adr/ADR-0058-output-quota-decoder-boundary.md
docs/log/2026-11-08-work-quota-review-find.md
docs/modules/app.md
docs/modules/build.md
docs/modules/latex.md
docs/modules/model.md
spikes/typst-main-editor/source-find-check.py
spikes/typst-main-editor/source-preview-layout-check.py
```
