# T54：窗口检查分片与跨 worktree 并发

本次在 `scholium-wt2`、`ation_ciger/ci/parallel-window-checks` 上从
`e39553f1432924af375ee21686e89506e2c36c10` 实现了调度与 CI 修改。最终差异没有修改 T53 拥有的检查脚本、`chrome_ui.py`、`latex_exit_*.py`、`a11y/` 或 tasks。
按 coordinator 指示签署合并了 PR #115 后的 `origin/main`（`fe3cff0`），合并提交为 `58bce6c`；
仅 runner docstring 冲突，保留 T53 的 evidence 说明与 T54 的调度说明。

## CI 与传输取舍

查阅已完成的 [main Rust 运行 42599680431](https://github.com/scholium-app/scholium/actions/runs/42599680431)
得到基线：提交 `398f2b5b21a63d67941d940baaa4e146be859f89`，test job 1531 s，
setup 40 s、build 115 s、cargo test 67 s、窗口检查 1281 s。此历史提交与本任务起点不同，
不能把其时长直接当作本分支的同机对照。

本机比较了缓存构建与二进制打包路径：完全暖构建 0.254 s；仅 app 包改用等价的
`profile.dev.package.scholium-app.debug=1`、保持依赖缓存后重编 3.731 s；恢复默认构建后
没有删除已有 target。五个二进制的 tar 为 158,474,240 bytes，打包 0.089 s、解包 0.044 s。
本机配置了 kache，因此缓存重编数字不能外推到 GitHub。
这是本地传输开销和缓存构建的下界比较，没有包含 GitHub artifact 网络传输或远端缓存恢复时间；
实际 CI 时长由 coordinator 查看后补充。

初始 durations 来自上述已完成 CI 日志的两语言均值，而非虚构所有检查时长。
35 个发现项（34 个常规执行项，latex-exit 保留跳过）采用 2 个分片时为 320.00 / 319.95 秒；
3 片约 213 秒，4 片约 160 秒，但会把 runner 数从 4 增到 6/8。先采用 2×2：
窗口预算由约 1281 秒降至每片约 320 秒，额外加 artifact/调度与包安装；这是预测而非 CI 实测。

选择一次构建后传输 tar：四个窗口 job 无须恢复完整 Cargo target 或重编工作区，且运行的是同一批
二进制。tar 内有 `scholium-app`、`scholium-sandbox-fs`、`scholium-pdf-worker`、`hammer`、
`project_hammer`，与当前 workspace 的五个 bin target 一致。tar 保留执行权限，artifact 关闭重复压缩。
LaTeX workflow 保留原有工具链包，setup 的共享 key 改为 `test`，与 build-window 使用同一路径。

job 名称为 `lint`、`build-window`、`window (zh-Hans, 1)`、`window (zh-Hans, 2)`、
`window (en, 1)`、`window (en, 2)`、`test`。必需检查仍为 `test`；该 job 的 `always()`
与成功状态判断使构建/任意分片失败、跳过或取消均无法放行。矩阵关闭 fail-fast，所有分片独立上传日志与证据。
每片 `--jobs 1`，没有增加逐项 workflow step，也没有改变现有 LaTeX 独立验收范围。
PR/push 运行不加 `--evidence`，只保留像素断言需要的截图（T53）；手动 `workflow_dispatch` 且 `evidence=true` 时
各分片生成完整审阅截图。LaTeX 独立验收仍带 `--evidence`。（coordinator 审阅时修改）

## 本机信号量和独占项

普通检查持有一个机器级槽位；`--jobs` 覆盖 `SCHOLIUM_WINDOW_JOBS`，默认
`max(1, min(3, cores // 6))`。同机 runner 使用相同 K 以共享容量。读写 gate 使独占项排斥所有 runner，
即便对方配置不同 K；admission 锁防止普通检查持续进入而饿死独占项。gate 兼容旧
`/tmp/scholium-window-checks.lock`，本次确实等待了 T53 的旧 runner，没有绕过它。

独占清单：

| 检查 | 原因 |
| --- | --- |
| latex-exit | 真实引擎 E6 输入、重绘及 p50/p95 |
| latex-view | E6 key-to-present p50/p95 |
| caret-follow | 持续输入与长文档重排 |
| sharp-raster | 大栅格、多倍率 CPU 负载 |
| parity | 输入回放与独立栅格比较 |
| source-preview-layout | 昂贵求值的截止时间与 CPU 预算 |

Xvfb 使用高编号显示、原子服务器锁、`-displayfd` 就绪通知、每项认证 cookie 和禁用 TCP；
不会继承桌面 DISPLAY、WAYLAND_DISPLAY、XAUTHORITY 或可访问性 bus。检查退出/超时会清理自己的进程组。

开发中复现了 Xvfb 握手竞态：管道的一次 read 可只返回数字，尚未返回换行；父进程关闭读端后，
服务器以 `Cannot write display number to fd` 退出。修复为读完一整行就绪通知。随后 100 对同时存在的
私有显示均成功连接；没有增加 UI sleep 或放宽断言。显示分配冲突可以重选编号，检查本身绝不自动重试。

第二轮并行运行复现了 `ui-polish-final [zh-Hans]` 的日志竞态：`raw()` 先对整个文件 UTF-8 解码，
而文件尾部可能只有一个多字节字符的首字节，触发 `UnicodeDecodeError`。一行改动先在 bytes 上按换行
截取完整记录，再逐行严格 UTF-8 解码；未完整记录留待下次真实状态轮询。没有忽略解码错误、放宽断言
或增加 sleep。使用“完整 AUDIT + 不完整 PROJECT UTF-8 尾”夹具复现旧失败，再验证新代码保留旧完整状态、
写完新行后准确返回 Unicode 新状态。这行临时修复没有提交；合入 T53 后 `raw()` 使用 `log_latest`，reader 已按 bytes 缓存不完整尾部，
因此删除临时修复，完全保留 T53 reader。失败轮次与合并前成功轮次均保留，只作诊断记录。
最终零失败连续证明在合并树上重新执行三轮并行加一轮串行。

## 验证记录

runner 契约先在旧实现上失败，随后通过：分片覆盖无遗漏/重复、最长优先均衡及确定性、新检查默认时长、
非法分片、独占清单、跨进程容量与独占、超时清理子进程、证据环境、认证显示隔离。
9 项测试通过；actionlint 1.7.11 使用临时配置登记其尚未内置的 `ubuntu-26.04`，两个 workflow 均通过。
Rust workspace 测试为 795 passed / 21 ignored；fmt、clippy、rustdoc、Rust size、文档链接、UI strings、
layers、cargo deny licenses/bans/sources、workspace bins build、rtex bundle 脚本测试均通过。

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --locked --workspace` | PASS，795 passed / 21 ignored |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | PASS |
| `python3 scripts/check-rust-size.py` | PASS |
| `python3 scripts/check-doc-links.py` | PASS |
| `python3 scripts/check-ui-strings.py` | PASS |
| `python3 scripts/check-layers.py` | PASS |
| `cargo deny --locked check licenses bans sources` | PASS；既有未用 license allowance 提示 |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | PASS；workspace bins 更广构建也通过 |
| `python3 scripts/test-window-checks.py` | PASS，9 项，显式 spawn context |
| `python3 scripts/test-bundle-rtex.py` | PASS，3 项 |
| `actionlint` 两个改动 workflow | PASS，临时登记固定镜像 label |

合并树 `58bce6c` 在相同二进制与检查脚本下连续执行了三轮 `--jobs 3 --locale zh-Hans en`：
751.3、747.9、751.7 秒，均为 68 个实际 pass + 2 个预定 skip（latex-exit 双语独立 workflow），
共 204 个实际检查全部通过。未使用证据开关，T53 的像素/行为断言仍全部运行。
单线程同树运行 `--jobs 1 --locale zh-Hans en --refresh-durations` 通过，1105.8 秒（18 分 26 秒）。
四轮合计 272 个实际 pass、8 个预定 skip、0 个失败。并行中位数 751.3 秒（12 分 31 秒），
相对同树串行缩短 32.1%，约 1.47 倍吞吐。合并前结果不计入这张验收表。

| 合并树运行 | jobs | 秒 | 实际 pass | 预定 skip | fail |
| --- | --- | --- | --- | --- | --- |
| merged-parallel-1 | 3 | 751.3 | 68 | 2 | 0 |
| merged-parallel-2 | 3 | 747.9 | 68 | 2 | 0 |
| merged-parallel-3 | 3 | 751.7 | 68 | 2 | 0 |
| merged-serial | 1 | 1105.8 | 68 | 2 | 0 |

`--refresh-durations` 已从串行成功运行更新全部常规项，latex-exit 为 0（常规套件跳过）。
最终两片每语言预算为 276.27 / 276.60 秒；完整归属见
[shard-balance.json](evidence/2026-11-09-parallel-window-checks/shard-balance.json)。
结果、历史 CI、缓存/传输测量与门禁摘要在
[证据目录](evidence/2026-11-09-parallel-window-checks/merged-serial.json)；这些均不包含用户文档。
额外以 `--only source-view --locale zh-Hans en --jobs 1 --evidence` 实测开关：2/2 通过，
6.7 秒，生成 6 张 PNG；[图像 manifest](evidence/2026-11-09-parallel-window-checks/evidence-images.json)
记录文件大小与 SHA-256，证明 CLI 确实将 evidence 开关传至 T53 helper。
原始记录保留于本 slot 的
`target/t54-validation/`（不纳入 Git）。

按任务要求只推分支、不创建 PR、不轮询 CI；此 workflow 的 push 只触发 main，
本分支的远端 CI 结果需要 coordinator 创建 PR 后记录，不能把本地测量写成已完成的 CI 成绩。

## CI 实测（PR #116，coordinator 补记）

[Rust workspace 运行 42643624632](https://github.com/scholium-app/scholium/actions/runs/42643624632)，提交 `876533f`：

| 作业 | 用时 |
|---|---:|
| lint | 1m33s |
| build-window（构建 + cargo test + 打包） | 3m38s |
| window (zh-Hans, 1) / (zh-Hans, 2) | 5m11s / 5m15s |
| window (en, 1) / (en, 2) | 5m02s / 4m38s |
| test（gate） | 3s |
| **整个工作流墙钟** | **9m05s**（此前 `test` 作业 23–25 分钟；#115 后 21m43s） |

[LaTeX 真实引擎运行 42643624966](https://github.com/scholium-app/scholium/actions/runs/42643624966)：命中共享 `test` 缓存，
`cargo build --workspace --bins` 401 秒 → 112 秒；作业 15m41s，其余主要是 setup 174 秒、rtex 打包 85 秒和
真实引擎验收本身 554 秒（该工作流仅在相关路径变化时触发）。
四个分片用时相差不到 40 秒，暂不需要增加分片数。
