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


## PR #110 Ubuntu restricted-userns 修订

协调者报告 Ubuntu 26.04 AppArmor 开启 restricted userns 时所有构建失败。
本机是 AppArmor 未启用的 Arch，不能把本机结果称为 Ubuntu 原生验收。
先保留旧 allocator，在 bwrap 丢弃全部能力后执行，得到 `Error: EPERM`，
复现其对子进程 mount 能力的错误依赖；日志为 `t51-ubuntu-fix/old-mount-denied.log`。

移除 unshare/nsenter allocator 路径，改由 bwrap `--size --tmpfs` 建立输出卷。
无挂载 helper 在能力已丢弃后设置 NO_NEW_PRIVS 与继承的 seccomp，拒绝所有 mount
和 namespace 进入/创建 syscall，再交付目录 fd、等待有界初始化并 exec。
四 profile 的 argv 回归验证只有 bwrap 挂载、无可写 host bind 和外部 unshare/nsenter。
原生 fixture 验证 CapEff=0、NoNewPrivs=1，unshare(0) 和 mount 均返回 EPERM。
因此后续原生/窗口验收一直在非 bwrap 子进程不能挂载的条件下执行；
这满足协调者允许的最低验证路径，不替代真实 Ubuntu AppArmor 集成验收。

采用 fd capability 保留 live 输出和退出后的复制能力，无需跨 AppArmor 读取子进程 root。
字节仍由内核 tmpfs size 限制；inode 数改为 20 ms 监督与复制前检查，
内核 memcg 约束 tmpfs 页与 inode/dentry slab 的分配内存；复制仍有独立 byte/inode 预算。
ADR 0058 保留旧方案作为拒绝的替代项，并引用 Linux 官方 cgroup v2 memory 记账规范。

新增原生记账测试中，写入 32 MiB 并创建 12000 个空文件，实际 scope 的
`memory.stat (shmem, slab)` 从 `(0, 0)` 增至 `(33554432, 11488376)` bytes；
取消后 fd 仍能复制出完整 32 MiB 产物。host 初始化页面不宣称计入子 memcg。
原生 quota 测试验证 Rtex、FullCompilation、TypstPreview 的 byte/inode 六种攻击均为 OutputQuota。
证据根目录为 `/home/ation_ciger/.cache/scholium-eval/t51-ubuntu-fix/`。


修复提交 `a0572fe` 后依次签署合并 main `8b7f6fc`（`2ee368e`）和
PDF 导出集成 `9093c82`（`f5e1af3`）。后一次合并保留查找/PDF 导出双快捷键，
把 Typst 的 1024 条诊断上限保留在新共用 execute 入口；其余双方功能和文档均保留。
所有权交叉是这些集成冲突，未新增跨任务产品功能。

`f5e1af3` 上 fmt、clippy、workspace tests（772 passed / 19 ignored）、
rustdoc -D warnings、size、links、UI strings、layers、cargo-deny、app/storage bins、
build/latex helper bins 和 rtex bundle 脚本测试均通过。
原生 build 4/4（配额攻击、memcg、Typst 隔离/取消、rtex E2）和 latex render 2/2 再次通过。
合并后的 stock Typst 对照在系统字体与 bundled-only 两组各 31/31 equal_pixels，
固定上游对象与 vendor patch 重建校验通过。

第一次 main 合并后全量窗口运行 66/66、exit 0。随后 main 合入 PDF 导出，
重新运行全量 34 checks × 2 locales，并启用 SCHOLIUM_LATEX_NATIVE_WINDOW=1 与
SCHOLIUM_T47_NATIVE=1。此轮中文 34 项全部完成、英文前两项完成后 runner 消失，
日志停在空的英文 caret-follow，未计该项通过；后续通过同一 machine-wide lock
在 fresh remaining-en 目录补跑剩余 32 项，不绕锁、不操作真实桌面。
中文原生 latex-view 已实际验证 LuaLaTeX DisplayList、20 次输入呈现、XeLaTeX FullCompilation；
problem-panel 已实际验证 LaTeX 整编错误、引用、成员文件跳转和成功编译清理；
PDF 导出窗口验证原生未保存文字/公式和受限 .typ 导出。
本机始终未启用 AppArmor，采用了协调者允许的无 allocator/unshare 路径与挂载禁止验证，
不声称做过真正 Ubuntu AppArmor 原生验收。

已提交的压缩证据在 [ubuntu-userns](evidence/2026-11-08-work-quota-review-find/ubuntu-userns/)，
完整窗口 PNG/audit 留在上述 home cache；不重复提交大量截图。


最终 remaining-en 32/32、exit 0；与中断前 34 个中文和 2 个英文成功项合计，
`f5e1af3` 的 34 checks × 2 locales 全覆盖通过。不把中断轮次记成单次 68/68 exit 0。
指定 problem-panel、source-preview-layout、project-source、latex-view 全部双语通过，
其中 latex-view 和 problem-panel 开启真实 TeX 原生验收开关。
额外运行 app 三引擎 PDF 导出原生测试，pdfLaTeX、XeLaTeX、LuaLaTeX 均在
300 dpi 与宿主相同引擎输出零 RGBA 差异，LuaLaTeX 包含未保存中英混排。
最后的证据提交只增加日志和压缩证据，不修改已验证代码。

## 协调者整合：导出不在宿主解析 PDF

合并 #109 后，导出发布路径仍在宿主用 `hayro-syntax` 解析 LaTeX 生成的 PDF 计页，与本 ADR 的“宿主不解码不可信 PDF”边界冲突。
改为页数由编译器报告：LaTeX 取引擎日志最后一条 `Output written on … (N pages`（先去掉 TeX 的定宽换行再匹配），
Typst 取文档页数；宿主只检查 `%PDF-` 头、尾部 `%%EOF`、字节与页数上限，再原子写出。
`scholium-app` 去掉 `hayro-syntax` 依赖；ADR 0056 与 app 规范同步。
同时把 #109/#110 新增的 `PDF_EXPORT`、`SOURCE_FIND` 输出改为 `audit_line::emit` 单次写出。

验证：`reported_pages` 单元测试（含换行折断、多次输出取最后、缺失/非数字）；`scholium-app`、`scholium-latex` 测试通过；
三引擎原生导出像素对照（`--ignored`）通过；窗口检查 pdf-export、source-find、problem-panel、source-preview-layout、
project-latex、latex-view 中英 12/12 通过。

## 协调者整合：真实引擎验收暴露的问题（2026-11-09）

合并 #108 后，专用 `latex-exit` workflow 第一次在本分支的新沙箱上运行真实 rtex：

- workflow 只构建 `scholium-app`，缺少同目录的 `scholium-sandbox-fs` 与 `scholium-pdf-worker`，应用报告
  “install scholium-sandbox-fs beside the application”。改为 `cargo build --workspace --bins`。发行打包同样必须携带这两个 helper。
- 产品缺陷：100 页 LuaLaTeX 夹具的整页布局超过 `validate::coordinates` 的固定 120 万 JSON 节点上限，引擎被标为 Failed、无预览。
  该遍历发生在 serde 已按 16 MiB 行上限解析之后，固定节点上限只会误伤长文档；改为由行上限推导
  （每个节点至少 2 字节，`DEFAULT_LINE_BYTES / 2`），仍以字节约束遍历。新增 150 万节点消息通过的回归。
- 检查脚本：增量读日志时以文本模式读取，读到多字节字符中间时抛 `UnicodeDecodeError`；改为按字节读取、只解码完整行。

本机真实 rtex `latex-exit` 中英 2/2 通过（含 100 页 E6）。
