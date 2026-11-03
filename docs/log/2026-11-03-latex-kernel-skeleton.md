# 2026-11-03：LaTeX 内核骨架与产品沙箱

## 结果与边界

T37 在指定 wt3 上新增了层级 1 `scholium-build` 和层级 2 `scholium-latex`，完成无 UI 的源码会话、
沙箱启动、事件适配、心跳与重同步、输入校验以及故障注入。公共接口没有 rtex 类型，也没有宿主执行文档的降级路线。
规范见 [build](../modules/build.md)、[latex](../modules/latex.md)，资源决策见
[ADR 0050](../adr/ADR-0050-linux-live-session-cgroups.md)。

本批没有接入窗口、绘制 display list、hayro、整编/导出、CST 或 Typst 沙箱。Paragraph/Pages 事件只包含布局元数据，
不冒充完整预览绘制。`/work` 累计磁盘配额及其他平台等价隔离仍待共享底座后续实现；单文件 64 MiB 限制不等同累计配额。

## 实现与故障注入

- build 使用 Rust argv 调用 systemd 用户 scope 与 bubblewrap，固定动态库/TeX/字体白名单，项目只读、构建目录可写、
  无网络、无宿主 HOME；2 GiB memory、64 tasks、swap 0。常驻进程没有 CPU 秒数或墙钟上限。
- 固定 fontspec 可信文档预热私有用户缓存，成功后原子发布；每个文档会话使用独立可写副本，拒绝缓存链接和特殊文件。
- 源码会话保存项目根、入口、引擎和单调 revision，LuaLaTeX 走 live，XeLaTeX/pdfLaTeX 只标识为整编预览。
  所有 UTF-8 offset 和相对路径在转发前校验，编辑不修改磁盘源文件。
- 独立 Tokio actor 监督 status 心跳；EOF/挂起/写失败后整树取消、有限重启、重发所有最新内存源码并请求布局。
  引擎 Restarting/Failed 后及恢复 Ready 时主动补布局；响应心跳的缺包失败不触发 host restart loop。
- stdout/stderr 共用累计限额，逐行检查后解析；未知字段兼容、畸形消息发诊断、超限终止、队列有界。
  未匹配确认的预览 revision 为 None，不能声称是最新结果。宿主使用单调时钟记录编辑到段落更新及布局操作耗时。
- 启动时在同一沙箱探测 CLI timeout flags；不支持的参数不传，能力缺失发事件，保留上游默认。
  后台 pass 使用上游超时，不安装 host per-compile watchdog。

CI 常规路径新增 **17 tests**（build 1、latex 16），无需 rtex、TeX 或 namespace。五条宿主兜底均有假工具故障覆盖：
杀/挂 serve 与未保存源码恢复；Restarting/Failed 丢更新补布局；UTF-8/路径/链接校验；timeout flags 缺失和上游 Failed；
导言区缺包只失败一次且无 host 重启。另覆盖畸形/超长/累计输出、启动失败、定位、未知字段和 revision 确认。

审查中先加入失败回归再修复了 stderr EOF 误判和畸形 acknowledgement 错配后续 revision：
[stderr red](evidence/latex-kernel-skeleton/red-stderr.log)、
[revision red](evidence/latex-kernel-skeleton/red-revision.log)。最终通过证据见 workspace 测试日志。
初次未提供原生环境变量的调用明确以“未运行：缺少 SCHOLIUM_RTEX_BIN”失败，未算通过；随后配置环境实际执行。
E2 测试驱动也修正了 openout 自动 `.tex` 后缀及宿主多次连接未清空队列的误判；最终逐项重新执行，没有把无效正对照算成通过。

## 原生来源与复现

来源和二进制/锁文件 SHA-256 见 [provenance.json](evidence/latex-kernel-skeleton/provenance.json)。
本机 Linux 7.2.11-arch1-1、Rust 1.99.0、bubblewrap 0.13.1、TeX Live 2026/Arch LuaHBTeX 1.24.0。

- 主验收：外部缓存精确 pin `a27561fdd6ec913f2a80d5b264f8fa7aaa228478`，使用已有 locked release 构建，
  source checkout 干净；没有把 rtex 链接或复制进 workspace。
- timeout 额外验收：用户只读 clone `~/Projects/realtime-tex`，`serve-timeouts`，commit
  `5c7ee8e604418453136a474b495b246c98bb4aaf`，使用其已有 release 二进制；未修改、提交或切换该 clone。

在仓库根设环境后运行（cache 路径位于本 slot 的 `target/`，不提交）：

```bash
export SCHOLIUM_RTEX_BIN="$HOME/.cache/scholium-eval/rtex-a27561fdd6ec913f2a80d5b264f8fa7aaa228478/target/release/rtex"
export SCHOLIUM_RTEX_TEXDIR="$HOME/.cache/scholium-eval/rtex-a27561fdd6ec913f2a80d5b264f8fa7aaa228478/tex"
export SCHOLIUM_RTEX_FONT_CACHE="$PWD/target/t37-native-font-cache"
cargo test --locked -p scholium-build native_rtex_e2 -- --ignored --nocapture --test-threads=1
cargo test --locked -p scholium-latex --test real -- --ignored --nocapture --test-threads=1
```

真实启动和编译始终使用产品 Sandbox/LiveSession。沙箱探测失败时 panic 消息明确标“未运行”及原因，命令失败，
不能静默 skip 后计通过。timeout 分支额外使用该只读 clone 的 binary/tex 和独立 cache 后运行：

```bash
cargo test --locked -p scholium-latex --test real native_capability_probe -- --ignored --nocapture --test-threads=1
```

## E2：产品路径逐项复测

完整布尔观察见 [e2-summary.json](evidence/latex-kernel-skeleton/e2-summary.json)，原始输出见
[native-e2.log](evidence/latex-kernel-skeleton/native-e2.log)。每项顺序 host → sandbox，全部满足 `[true, false]`。

| 探针 | 宿主攻击有效 | 产品沙箱攻击有效 | 判定 |
|---|---|---|---|
| Lua `io.open` 读白名单外随机秘密 | 是（输出包含随机秘密） | 否 | Pass |
| `input` 绝对路径 | 是（输出包含随机秘密） | 否 | Pass |
| shell escape / os.execute | 是（构建目录外文件产生） | 否 | Pass |
| Lua 写出构建目录 | 是 | 否 | Pass |
| TeX openout 写出构建目录 | 是 | 否 | Pass |
| Lua 写项目只读挂载 | 是 | 否 | Pass |
| 网络连接宿主 loopback listener | 是 | 否（0 connections） | Pass |

shell/network 使用仅测试内部的 Rust LuaLaTeX launcher 加 `--shell-escape`，解除 TeX 本身的限制，
让探针真正打到 OS 边界；产品没有对应 bypass 参数或环境变量。除了只替换该可信工具的挂载以放大攻击，
所有 argv、隔离、资源和取消都使用产品实现。秘密同时检查 JSON/stderr 和全部普通 build 文件；不读取 FIFO 或其他特殊文件。
会话尝试写入字体缓存的 poison 没有进入持久 `ready` 缓存。每项完成后取消 cgroup。

原生测试还读取了实际 cgroup v2 文件：`memory.max=2147483648`、`pids.max=64`、`memory.swap.max=0`，
`Max cpu time unlimited/unlimited`。不是仅断言拼接出的 argv。

## E8 子集与 timeout 能力

[固定 pin 的原生日志](evidence/latex-kernel-skeleton/native.log) 包含 **3/3 native tests Pass**：

- 启动 → 首布局 → 正文编辑 → 匹配 host revision 的 Paragraph，越界、UTF-8 中间、`..`、绝对路径及 symlink 逃逸均被拒绝。
- 分别 SIGKILL/SIGSTOP 真正的 `rtex serve`，检测 EOF/心跳截止，generation 增加，取消整个旧 scope，
  最新未保存文本重新同步后取得匹配 revision 的布局。最终没有遗留 rtex/LuaLaTeX 进程。
- 固定 pin 缺 timeout flags，能力诊断实际到达。其 timeout 注入部分明确标为“未运行：没有 PR #36 flags”，
  这里通过的是能力探测和缺失报告，不声称验证了 pin 的显式 2 秒配置。

[PR #36 分支日志](evidence/latex-kernel-skeleton/native-timeouts.log) 额外 **1/1 Pass**：
两个 timeout flags 均被识别并传入，后台死循环由 upstream pass timeout 在 **2 秒**失败，Pages outcome 为 Failed；
删除循环后匹配 host revision 2 的布局恢复 Complete，没有宿主单次编译 watchdog。
该轮上游 live engine 的 watchdog 原因显示 30 秒（此前一轮显示 1 秒）；照录此差异，不把 CLI 参数识别等同
所有 live-engine 阶段都实际遵守 1 秒。后台 2 秒 Failed 的判据和修复后的恢复均在原始日志中。

以上耗时是功能监督观察，未作为性能 benchmark；原生窗口 E6、纹理上传/呈现及 CJK 完整论文仍属于后续 view/UI 验收。

## 仓库门禁与交付

最终命令/退出码见 [gates.json](evidence/latex-kernel-skeleton/gates.json)。

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Pass |
| `cargo test --locked --workspace` | Pass：552 passed / 8 ignored；其中本批 4 ignored 原生测试已实际另跑 |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` | Pass |
| `python3 scripts/check-rust-size.py` | Pass |
| `python3 scripts/check-doc-links.py` | Pass |
| `python3 scripts/check-ui-strings.py` | Pass |
| `python3 scripts/check-layers.py` | Pass |
| `cargo deny --locked check licenses bans sources` | Pass |
| `cargo build --locked -p scholium-app -p scholium-storage --bins` | Pass |
| `python3 scripts/window-checks.py --locale zh-Hans en --out target/t37-windows` | Pass：44/44（zh-Hans/en 各 22），包含 a11y、parity、math、source checks |

窗口检查使用机器级锁和私有 Xvfb，没有接触用户桌面；完整截图/raster 留在本机 `target/t37-windows/`。
未改 editor surface、Typst fork 或 adapter，Cargo.lock 只有新增 crate 的依赖，因此没有另跑 stock-Typst 像素比较。

文件归属：仅新增两个 owned crate，更新 build/latex 模块与原生依赖登记，新增 ADR 0050 和本日志/evidence。
共享文件只追加 workspace members、LAYERS、模块分层行、ADR 索引；Cargo.lock 由 Cargo 更新。
没有修改 storage/app/session、ADR 0049/0051 或 CI workflow。基于 `origin/main` 的 `891be743767a4ddd0b68d703cefac7153a3e7cf6`。
按任务例外只推送分支、不创建 PR、不合并、不轮询 CI。
