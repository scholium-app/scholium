# 窗口检查脚本的增量日志与按需截图（T53）

## 范围与实现

本任务在 `ation_ciger/test/check-script-speed`、wt1 上修改了窗口检查脚本，基线为
`e39553f1432924af375ee21686e89506e2c36c10`。没有修改应用、CI 工作流或 runner 的执行代码；
`scripts/window-checks.py` 只补充了环境变量说明。T54 的并发调度与 CI 分片不属于这次测量的改动。

共享 `log_reader.py` 为每个日志订阅保留二进制偏移、未完成行及最新记录；只有需要计数、
切片或历史筛选的调用方保留历史。订阅只解析请求的前缀，并按原始行序合并多前缀观察。
日志换 inode 或变短时自动重置，应用启动前显式重置也覆盖同 inode 截断后快速长大的情况。
`latex_exit_window.py` 使用同一 reader 的新增记录流，保留原有 frame、engine timing 与 controls 收集。
失败诊断只读取末尾 4,000 字节。`git grep -n 'read_text()' spikes/typst-main-editor`
剩余项均为非日志文件。

默认跳过审阅截图及对应的 GPU 呈现等待，`SCHOLIUM_CHECK_EVIDENCE=1` 恢复原截图路径与完整截图矩阵。
`ui-polish-final` 的纯截图尺寸矩阵默认保留每个主题的一个尺寸；对话框 `*-fits` 和命令输入框矩阵
仍逐组合执行原有断言。`start-page-dialogs` 的两主题及窄窗矩形断言均保留。
`source-preview-layout` 的纯截图主题遍历默认保留 light；`problem-panel` 的显式纯截图尺寸矩阵也受证据开关控制。
interaction 的两张 tint 测量图、source-theme 的像素比较图、sharp-raster 的测量图以及 E6 的 XGetImage
路径始终执行。独立的 `chrome-shots.py` 本身就是显式截图工具，保持截图行为，只迁移日志读取。

## 验证与测量方法

新增 reader 与证据开关测试先得到缺少 reader 模块的失败，再实现并通过了 7 项测试。
测试覆盖跨写入的 UTF-8、未完成行、前缀筛选、跨前缀顺序、空闲轮询不重复读取、历史/仅最新状态、
截断/替换/显式重启和截图开关。AST 对照确认既有 `assert` 表达式、wait/state 参数及超时默认值不变。

[环境和二进制摘要](evidence/2026-11-09-check-script-speed/environment.json)记录了测量来源。
完整窗口检查使用原 runner 的串行双语言命令，始终遵守 `/tmp/scholium-window-checks.lock`，
未启用 T54 的并发实现。基线脚本复制到 `/tmp/t53-baseline-root` 作为只读测试夹具，
`crates`、`target` 等资源指向同一 wt1；这不是另建 Git worktree/clone。
所有应用只在 runner 创建的私有 Xvfb 上运行。

构建先执行任务通用规则的 app/storage bins 命令；基线中文 pdf-export 和 problem-panel 随后报告缺少
`scholium-sandbox-fs`。补跑 CI 实际使用的 `cargo build --locked --workspace --bins` 后，
对原脚本的 pdf-export、problem-panel 双语言单独补测。原始失败保留，不把失败用时作为成功基线。

本机没有 `/usr/bin/time`，因此使用[测量包装器](evidence/2026-11-09-check-script-speed/measure-run.py)的 Python `resource.getrusage(RUSAGE_CHILDREN)` 记录整个 runner
进程树的 user/system CPU 与最大 RSS，并同时记录 MONOTONIC/BOOTTIME。
runner 外部墙钟包含等待其它 slot 锁的时间；比较用各检查 seconds 的和，排除锁等待。
CPU 总量包括应用、Xvfb、ImageMagick，不能解释为纯日志轮询 CPU。Rust gates 与部分基线窗口检查
时间重叠；这里是同机单次工程测量，不是隔离环境的统计基准或 CI 加速承诺。

独立轮询 CPU 探针使用基线中已结束的三个真实 app.log，各执行 100 次相同的 latest-AUDIT 查询，
包含第一次读取/解析；验证两实现返回完全相同的最后状态。
[探针代码](evidence/2026-11-09-check-script-speed/poll-cpu.py)和
[原始指标](evidence/2026-11-09-check-script-speed/poll-cpu.json)记录了输入大小、SHA-256、读取字节数及 process CPU。
旧实现每次扫描整个日志，新实现总共读取一遍：

| 日志 | 字节 | 原轮询 CPU（秒） | 增量轮询 CPU（秒） | 降低倍数 |
|---|---:|---:|---:|---:|
| chrome | 1,987,156 | 0.2751 | 0.0233 | 11.8× |
| start-page-dialogs/light | 15,508,389 | 4.7010 | 0.0218 | 216.0× |
| diagnostics | 27,405,564 | 3.6253 | 0.3260 | 11.1× |

这是固定完成日志的轮询成本，不能替代整套实时检查的结果。

普通模式首轮中文 display-formula 暴露了脚本迁移中的 helper import 放置错误（首次使用后才 import），
已将导入移至顶部，并对全部新 helper import 的位置做静态核对；该失败保留，另外补跑双语言该检查。

## 完整窗口结果

普通模式完成了 68 个实际检查组合：67 pass，中文 display-formula 的导入错误为唯一失败；
另有 2 个 latex-exit 按 runner 规则跳过。原始命令输出和进程树指标分别保存在
[普通模式日志](evidence/2026-11-09-check-script-speed/after.log.gz)、
[逐检查记录](evidence/2026-11-09-check-script-speed/after.json)、
[进程指标](evidence/2026-11-09-check-script-speed/after-process.json)。
基线的对应证据为[原始日志](evidence/2026-11-09-check-script-speed/before.log.gz)、
[逐检查记录](evidence/2026-11-09-check-script-speed/before.json)、
[进程指标](evidence/2026-11-09-check-script-speed/before-process.json)。

排除基线缺 helper 的两个中文组合及普通模式中文 display-formula 后，65 个两边均通过的组合
合计 1,406.2 → 1,259.5 秒，减少 10.4%。此配对结果不混入失败用时。
普通模式双语言保留了 16 张参与像素断言的 PNG；完整基线初轮为 526 张（含不完整的失败检查产物）。
[基线图片清单](evidence/2026-11-09-check-script-speed/before-images.json)与
[普通模式图片清单](evidence/2026-11-09-check-script-speed/after-images.json)保留相对路径。

证据模式完整双语言运行 68/68 个实际检查通过；另 2 个 latex-exit 条目按既有规则跳过，
runner 因此显示 70/70 ok。普通模式最终也有全部 68 个组合的通过记录：初轮 67 个，
修正导入后的[display-formula 双语言补测](evidence/2026-11-09-check-script-speed/after-display.json) 2/2。
基线的[pdf-export 补测](evidence/2026-11-09-check-script-speed/before-pdf.json)与
[problem-panel 补测](evidence/2026-11-09-check-script-speed/before-problem.json)各 2/2 通过。
这些是定向修复后的显式补测，不是 runner 自动重试。

下表单位为秒。仅将失败的中文三个单元格替换成各自补测成功值；所有原始失败/用时仍在初轮记录中。
基线中文 pdf-export=13.0、problem-panel=10.8，普通模式中文 display-formula=6.7 来自补测；
其余单元格来自对应完整运行。[可机读配对记录](evidence/2026-11-09-check-script-speed/complete-comparison.json)
保留了全部 68 个组合；证据模式[原始输出](evidence/2026-11-09-check-script-speed/evidence.log.gz)、
[逐检查记录](evidence/2026-11-09-check-script-speed/evidence.json)及
[进程指标](evidence/2026-11-09-check-script-speed/evidence-process.json)另存。

| 检查 | 前 zh-Hans | 后 zh-Hans | 前 en | 后 en | 证据 zh-Hans | 证据 en |
|---|---:|---:|---:|---:|---:|---:|
| a11y | 6.6 | 6.7 | 6.5 | 6.8 | 6.9 | 7.1 |
| a4-surface | 7.1 | 5.7 | 6.2 | 5.8 | 6.4 | 6.6 |
| backspace-block-merge | 7.0 | 7.0 | 7.0 | 7.0 | 7.1 | 7.1 |
| caret-follow | 33.7 | 57.6 | 31.8 | 32.5 | 32.1 | 32.6 |
| chrome | 26.9 | 29.9 | 27.0 | 24.6 | 27.7 | 28.0 |
| default-launch | 7.4 | 7.7 | 7.5 | 7.3 | 7.4 | 7.5 |
| diagnostics | 15.2 | 14.0 | 15.4 | 13.2 | 14.5 | 14.5 |
| display-formula | 6.5 | 6.7 | 6.5 | 6.5 | 6.9 | 7.0 |
| fraction-at-caret | 35.1 | 33.8 | 35.0 | 34.1 | 35.2 | 35.2 |
| ime-inline | 4.3 | 4.1 | 4.3 | 4.3 | 4.6 | 4.6 |
| inline-style | 5.8 | 5.4 | 5.7 | 5.6 | 6.0 | 6.1 |
| interaction | 8.0 | 25.5 | 8.1 | 8.3 | 8.6 | 8.8 |
| latex-view | 3.8 | 4.4 | 3.8 | 3.7 | 4.4 | 3.9 |
| math-attach | 5.0 | 7.1 | 5.0 | 4.9 | 5.4 | 5.4 |
| math-edge-caret | 11.4 | 13.6 | 11.4 | 10.3 | 11.5 | 11.5 |
| math-enter | 11.7 | 13.9 | 11.7 | 11.2 | 12.2 | 12.3 |
| math-input | 10.7 | 10.3 | 10.7 | 9.8 | 11.6 | 11.7 |
| math-root-delimiter | 9.9 | 10.9 | 10.3 | 9.7 | 10.1 | 10.1 |
| native | 17.8 | 15.4 | 17.7 | 11.8 | 20.3 | 20.5 |
| parity | 72.5 | 70.5 | 70.9 | 70.8 | 71.7 | 72.1 |
| pdf-export | 13.0 | 10.2 | 10.6 | 10.9 | 12.7 | 12.7 |
| problem-panel | 10.8 | 8.6 | 10.6 | 8.7 | 9.7 | 9.6 |
| project-latex | 13.6 | 11.5 | 12.7 | 11.7 | 14.2 | 14.1 |
| project-native | 8.4 | 6.9 | 7.5 | 7.4 | 8.5 | 8.5 |
| project-source | 5.2 | 4.8 | 5.1 | 5.2 | 5.4 | 5.4 |
| sharp-raster | 57.9 | 57.7 | 54.3 | 62.8 | 65.2 | 65.5 |
| source-find | 5.0 | 4.7 | 5.0 | 4.7 | 5.1 | 5.1 |
| source-preview-layout | 34.3 | 27.4 | 34.1 | 27.5 | 34.8 | 34.9 |
| source-theme | 8.7 | 8.7 | 8.7 | 8.8 | 9.6 | 9.6 |
| source-view | 5.7 | 5.1 | 5.7 | 5.1 | 6.3 | 6.3 |
| start-page-dialogs | 80.5 | 55.1 | 78.1 | 54.4 | 79.2 | 78.2 |
| ui-foundation | 20.0 | 15.7 | 20.0 | 15.6 | 19.6 | 19.7 |
| ui-polish-final | 156.2 | 99.1 | 146.4 | 99.1 | 142.9 | 143.0 |
| undo-group | 4.8 | 4.6 | 4.7 | 4.6 | 5.0 | 5.1 |
| 双语言合计 | 1436.5 | 1285.0 | — | — | 1459.1 | — |

补测替换后的合计为 1436.5 → 1285.0 秒（23.94 → 21.42 分钟），
减少 10.5%。这是补测拼合的成功覆盖合计，并非另一次从头无失败运行的墙钟。
初轮原始合计为 1,485.1 → 1,281.4 秒，因含不同失败，不能直接作为成功检查加速比例。

| 原完整运行 | 检查用时之和（秒） | 含锁等待墙钟（秒） | 子进程 user CPU（秒） | 子进程 system CPU（秒） | 最大 RSS（KiB） |
|---|---:|---:|---:|---:|---:|
| 基线（2 失败） | 1485.1 | 1485.4 | 9727.8 | 182.7 | 6538780 |
| 普通模式（1 失败） | 1281.4 | 2161.3 | 7392.2 | 129.1 | 5790496 |
| 证据模式（全通过） | 1459.1 | 3573.6 | 8405.7 | 144.4 | 5988340 |

三次完整运行的 BOOTTIME 与 MONOTONIC 差均小于 0.001 秒，没有检测到挂起；进程 CPU 总量包含
应用/Xvfb/抓图且初轮成功覆盖不同，仅作原始观测，不能归因成纯日志 CPU 改善。

证据模式恢复了全部 530 张原有图片：将原始完整基线中失败检查的图片目录替换为对应成功补测，
与新证据模式的相对路径集合逐项比较完全相等。
[图片集合核对](evidence/2026-11-09-check-script-speed/image-comparison.json)、
[成功原始检查图片清单](evidence/2026-11-09-check-script-speed/before-complete-images.json)和
[证据模式清单](evidence/2026-11-09-check-script-speed/evidence-images.json)保留了可检查的证明。
其中 ui-polish-final 每语言 112 张、chrome 每语言 7 张、start-page-dialogs 每语言 36 张均恢复。
图片本体及完整 app.log 留在本地测试输出目录，Git 只保留测量、路径清单和压缩 runner 输出。

普通模式补测的[原始输出](evidence/2026-11-09-check-script-speed/after-display.log.gz)及
[进程指标](evidence/2026-11-09-check-script-speed/after-display-process.json)、
基线 pdf-export 的[原始输出](evidence/2026-11-09-check-script-speed/before-pdf.log.gz)及
[进程指标](evidence/2026-11-09-check-script-speed/before-pdf-process.json)、
基线 problem-panel 的[原始输出](evidence/2026-11-09-check-script-speed/before-problem.log.gz)及
[进程指标](evidence/2026-11-09-check-script-speed/before-problem-process.json)保留补测来源。

专用 real-rtex / TeX Live latex-exit 验收不在默认窗口套件中，因此没有声称本次重跑了它；
其共享 harness 的增量 reader 迁移由 CPU 回归测试覆盖完整行/部分字节语义，
E6、XGetImage 和原有像素断言路径未改动。


## 非窗口门禁

[命令、退出码与耗时](evidence/2026-11-09-check-script-speed/gates.json)及
[完整压缩日志](evidence/2026-11-09-check-script-speed/gates.log.gz)保留了验证证据。

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --locked --workspace --all-targets -- -D warnings`：通过。
- `cargo test --locked --workspace`：795 passed / 21 ignored，通过。
- `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps`：通过。
- `python3 scripts/check-rust-size.py`：通过。
- `python3 scripts/check-doc-links.py`：通过。
- `python3 scripts/check-ui-strings.py`：通过。
- `python3 scripts/check-layers.py`：通过。
- `cargo deny --locked check licenses bans sources`：通过；仅保留原有未遇到许可 allowance 的提示。
- `cargo build --locked -p scholium-app -p scholium-storage --bins`：通过。
- `cargo build --locked --workspace --bins`：通过，补齐 sandbox/PDF helper。
- `python3 -m unittest discover -s spikes/typst-main-editor -p 'test_*.py'`：7/7 通过。
- `python3 -m compileall -q spikes/typst-main-editor`：通过。

未修改编辑器表面、Typst fork 或锁文件，因此未触发 stock-Typst 对照。
