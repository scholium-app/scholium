# T54：窗口检查分片与跨 worktree 并发

本次在 `scholium-wt2`、`ation_ciger/ci/parallel-window-checks` 上从
`e39553f1432924af375ee21686e89506e2c36c10` 实现了调度与 CI 修改。仅修改了 T53 拥有的 `ui-polish-final-check.py` 一行，作为任务明确允许的完整状态等待修复；
其余检查脚本、`chrome_ui.py`、`latex_exit_*.py`、`a11y/` 和 coordinator 的 tasks 文件未改。

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
每片 `--jobs 1 --evidence`，没有增加逐项 workflow step，也没有改变现有 LaTeX 独立验收范围。

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
写完新行后准确返回 Unicode 新状态。此行与 T53 日志读取工作重叠，coordinator 合并时需保留其完整行语义。
失败轮次保留，不计入零失败连续证明；修复后重新执行三轮并行加一轮串行。

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
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | workspace bins 更广构建已通过 |
| `python3 scripts/test-window-checks.py` | PASS，9 项，显式 spawn context |
| `python3 scripts/test-bundle-rtex.py` | PASS，3 项 |
| `actionlint` 两个改动 workflow | PASS，临时登记固定镜像 label |

完整稳定性序列、最终时长表与分片平衡将在结束后填入。原始记录保留于本 slot 的
`target/t54-validation/`（不纳入 Git）。

按任务要求只推分支、不创建 PR、不轮询 CI；此 workflow 的 push 只触发 main，
本分支的远端 CI 结果需要 coordinator 创建 PR 后记录，不能把本地测量写成已完成的 CI 成绩。
