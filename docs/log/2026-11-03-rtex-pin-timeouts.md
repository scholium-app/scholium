# 2026-11-03：rtex pin、超时参数与监督器恢复

## 结果与范围

T38 从 `caad0638d3b70f77be606088d0a66d26bc6ad6f8` 的指定 wt3 分支完成 pin 升级及两处监督器修复。
`rtex-pin.py` 完成仓库外 locked release 构建和 E1/E2/完整 E8 后，把 pin 从 `a27561f` 更新到
`531e2c6b32a09d33be58f6be1301fe8f793411cd`。版本、二进制 SHA-256、Cargo.lock SHA-256 与 CLI help 见
[provenance.json](evidence/rtex-pin-timeouts/provenance.json)。上游 checkout 干净；没有改用户的 realtime-tex clone。

行为合约同步到 [latex 模块](../modules/latex.md)，决策修订见
[ADR 0043](../adr/ADR-0043-rtex-live-preview.md)。本批没有改沙箱、build crate、依赖锁、编辑器、绘制或 CI。
ROADMAP 仅改 LaTeX 轨第 2 条的过时超时说明；LATEX_WORKSPACE 同步 CLI 参数与旧二进制诊断路径。

## 两处监督器回归

先加入行为回归并实际观察失败：

- [重启预算 red](evidence/rtex-pin-timeouts/red-restart-budget.log)：旧实现累计整个会话的重启次数，
  多次间隔健康期的故障耗尽预算，预期恢复事件超时。
- [慢消费者 red](evidence/rtex-pin-timeouts/red-consumer.log)：旧事件队列满后终止，出现 `live supervisor stopped`。

随后实现并取得 [green](evidence/rtex-pin-timeouts/green-supervisor.log)：

- `LiveConfig.restart_stability_window` 默认 30 秒，拒绝零值。有效 status 回复证明健康，连续健康达到窗口后重启计数清零；
  重启重新计健康期。测试以 max_restarts=1 注入三次相隔健康窗口的 SIGKILL，四次 launch 均成功；
  快速两次故障仍耗尽预算并保留终止原因。
- 私有 mailbox 保持最多 32 条事件及一个合并布局重取标记。同 generation 的 Pages 合并为最新；
  同 generation、同段落的 Paragraph 合并。溢出淘汰非 Engine；淘汰预览产生 `LayoutRequired`，调用者据此重取布局。
  Engine 状态与原因按序保留；全为 Engine 时异步背压，等待不计入心跳截止或健康期，不误报引擎挂起。
- 暂停消费期间 100 次编辑后仍能得到 revision 100 布局，只有一次进程启动；不同段落溢出后标记补布局，源码和 Stopped 事件保留。
  mailbox 单测用 1000 次更新证明合并/溢出上界，用 40 个 Engine 事件证明背压后所有原因仍按序到达。

边界：Diagnostic/Location 在溢出时可以淘汰；全 Engine 队列背压期间 actor 命令也等待消费恢复。
本批不引入无限事件存储；Drop 继续中止 actor 并清理进程树。

## Pin 审查

原始命令、退出码和判定见 [pin/run.json](evidence/rtex-pin-timeouts/pin/run.json)，完整输出见
[pin-driver.log](evidence/rtex-pin-timeouts/pin-driver.log)。命令：

```bash
python3 scripts/rtex-pin.py 531e2c6b32a09d33be58f6be1301fe8f793411cd --evidence docs/log/evidence/rtex-pin-timeouts/pin --run-id t38-531e2c6
```

- E1 Pass：licenses/sources exit 0；未过滤 bans exit 2（三个 crate 的内部 wildcard path 依赖诊断（共五条依赖边）），advisories exit 1
  （一条 `ttf-parser` 不维护通知），按 ADR 0043 既有标准照录、不阻断，不修改政策。
- E2 Pass：全部必需负对照通过；默认网络的沙箱外攻击也无效，标为 Inconclusive，解除 TeX 限制后的网络正/负对照通过。
  死循环、修复恢复、进程清理也通过。逐项观察见
  [negative-summary.json](evidence/rtex-pin-timeouts/pin/e2/negative-summary.json)。
- 完整 E8 Pass：后台 kill、upstream timeout、缺宏包失败、越界/反向/UTF-8 与路径拒绝，以及 serve kill 等故障。
  超时 helper 仅在外部评估副本中配置上游库，评估锁文件与候选相同；产品仍运行真实 CLI 二进制。

## 产品沙箱原生复验

全部使用同一新 pin 的 release 二进制与 tex 目录，独立字体缓存位于 `target/t38-native-font-cache`。
环境不可用会以“未运行”失败，不将 ignored 当执行通过。复现：

```bash
export SCHOLIUM_RTEX_BIN="$HOME/.cache/scholium-eval/rtex-531e2c6b32a09d33be58f6be1301fe8f793411cd/target/release/rtex"
export SCHOLIUM_RTEX_TEXDIR="$HOME/.cache/scholium-eval/rtex-531e2c6b32a09d33be58f6be1301fe8f793411cd/tex"
export SCHOLIUM_RTEX_FONT_CACHE="$PWD/target/t38-native-font-cache"
cargo test --locked -p scholium-build native_rtex_e2 -- --ignored --nocapture --test-threads=1
cargo test --locked -p scholium-latex --test real -- --ignored --nocapture --test-threads=1
cargo test --locked -p scholium-latex --test native_links -- --ignored --nocapture --test-threads=1
```

- [native-e2.log](evidence/rtex-pin-timeouts/native-e2.log)：1/1 Pass，含七项攻击的 host→sandbox 对照，全部 `[true,false]`。
  Lua 外部读、绝对 input、shell、Lua 写构建外、openout 写构建外、写只读项目及网络均被隔离。
  shell/network 继续沿用 T37 的测试专用 LuaLaTeX launcher 放大攻击，产品无 bypass。实际 cgroup 值仍是 2 GiB、64 tasks、swap 0、CPU unlimited。
- [native-real.log](evidence/rtex-pin-timeouts/native-real.log)：3/3 Pass。启动→编辑→Paragraph revision 1；输入/链接逃逸拒绝；
  SIGKILL/SIGSTOP serve 后恢复 generation 1、未保存 revision 1；CLI 两个参数均存在并传入。
  后台 pass 在 2 秒报 Failed（revision 1），去除死循环后 revision 2 恢复 Complete。
  live engine 原因仍显示 30 秒 watchdog，照录，不把探测到 compile 参数等同于所有阶段都遵守 1 秒。
- [native-links.log](evidence/rtex-pin-timeouts/native-links.log)：1/1 Pass。检查上游实际生成的
  `/work/build/src/plain.txt -> /project/plain.txt` 与 `/work/build/src/assets -> /project/assets`。
  沙箱外同形链接攻击先证明可读写；产品 Sandbox 中实际链接可读，两种写回都被拒绝，原始文件内容不变。
  无 EXDEV 或悬空链接，无需调整挂载或 profile。

## 回归与仓库门禁

E3/E5/E6 复现驱动见 [regressions.py](evidence/rtex-pin-timeouts/regressions.py)。
[E3](evidence/rtex-pin-timeouts/e3/e3-summary.json)：ctex 4/4 eligible、174/174 identical glyphs；
luatexja 4/4 eligible、188/188 identical glyphs；各 1/1 页 exact，capture PDF 与 clean 相同。
[E5](evidence/rtex-pin-timeouts/e5/e5-summary.json)：288/288 marker click/caret exact，正文 alignment coverage
97.79% / 99.08%，TFM 仍为 0%，没有把该定位范围扩大到任意公式字符。
[E6](evidence/rtex-pin-timeouts/e6/e6-summary.json)：10 页与 100 页夹具在 steady / bursts 各
120/120 fast + paragraph-ok（合计 480/480），sandbox exit 0；恢复后的两次 CLOCK_MONOTONIC 与
CLOCK_BOOTTIME 差为 0.00000034 秒，小于 1 秒门槛。此原型测量包含 host 事件和 raster，不是窗口呈现时延。

仓库命令/退出码见 [gates.json](evidence/rtex-pin-timeouts/gates.json)。窗口检查采用脚本的机器级锁和私有 Xvfb；
截图/raster 保留在本 slot 的 `target/t38-windows/`，没有接触用户桌面。

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Pass |
| `cargo test --locked --workspace` | Pass：600 passed / 11 ignored；本任务 5 条原生测试已实际另跑 |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | Pass |
| `python3 scripts/check-rust-size.py` | Pass |
| `python3 scripts/check-doc-links.py` | Pass |
| `python3 scripts/check-ui-strings.py` | Pass |
| `python3 scripts/check-layers.py` | Pass |
| `cargo deny --locked check licenses bans sources` | Pass |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | Pass |
| `python3 scripts/window-checks.py --locale zh-Hans en --out target/t38-windows` | Pass：46/46，zh-Hans/en 各 23 |

本批没有改 editor surface、Typst fork/adapter 或 Cargo.lock，因此无需 stock-Typst 像素比较。
按任务例外只推送分支，不创建 PR、不合并、不轮询 CI。后续 UI/绘制、其他平台隔离与累计磁盘配额仍是既有后续工作。
