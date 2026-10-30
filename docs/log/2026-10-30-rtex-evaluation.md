# 2026-10-30：rtex 评估第一轮（E2 沙箱、E1 许可、E3 准备）

本记录只追溯这一轮评估做了什么、怎么验证、观察到什么；评估项与判据见 [LaTeX 工作区规划](../plan/LATEX_WORKSPACE.md)
的 L0（E1–E8），背景见 [ADR 0041](../adr/ADR-0041-separate-latex-typst-kernels.md)。选型结论不在这里，由协调者据此另立 ADR。
驱动、夹具与复现命令见 [spikes/rtex-eval](../../spikes/rtex-eval/README.md)，原始证据在
[evidence/rtex-evaluation](evidence/rtex-evaluation/)。

## 结论摘要

| 项 | 结果 | 一句话 |
|---|---|---|
| 冒烟（沙箱外） | Pass | doctor、10 页书、一次编辑、verify 全部通过 |
| 正对照（沙箱内外） | Pass | 书与文章在沙箱内外页数相同、导出文本 sha256 相同，实时段落更新到达 |
| E2 沙箱（否决项） | **Pass** | 9 组负对照里 8 组在沙箱外确实生效、在沙箱内全部被挡住且四个通道都没有泄漏；1 组（默认配置下的网络）在沙箱外就被 LuaTeX 挡住，已改用“解除 TeX 限制”配置重做并通过 |
| E1 许可与依赖 | licenses、sources 通过；bans 未通过（3 条，均为 rtex 自身路径依赖没有版本号）；advisories 未通过（2 条） | Linux 上没有任何原生代码被编译或链接 |
| E3 中文 | 准备完成，未判定（不是 Blocked） | luatexja 已在本机安装；两份夹具编译通过，一次沙箱外 `rtex verify` 字形全部一致 |

E2 通过的前提是 rtex 只在沙箱内运行：沙箱外，同一份文档可以读 `/etc/passwd`、`$HOME` 下的文件和主机环境变量，可以写构建目录以外的任意可写路径，失控的后台编译会无限占用 CPU（见“发现与局限”）。

## 环境

| 项 | 值 |
|---|---|
| rtex | v0.0.5，commit `e0ad11765ec016846b6e07619081f0528621636b`，浅克隆到 `~/.cache/scholium-eval/rtex-v0.0.5`，`cargo build --release` 构建（rustc 1.99.1，[构建日志](evidence/rtex-evaluation/build.log)，[克隆日志](evidence/rtex-evaluation/clone.log)）；克隆目录 `git status` 干净，上游代码未进入仓库 |
| TeX Live | TeX Live 2026（Arch：texlive-basic / texlive-luatex 2026.1-1），LuaHBTeX 1.24.0；`RTEX_TEXLIVE_BIN=/usr/bin`、`RTEX_TEXDIR=<克隆>/tex` |
| 系统 | Arch Linux，内核 7.2.11-arch1-1，bubblewrap 0.13.1，Python 3.14.8，cargo-deny 0.20.2 |
| 沙箱 | `spikes/toolchain-sandbox.sh` 新增的 `rtex` 配置，沿用既有资源限制（见下） |

## 做了什么

- 继续上一位执行者未提交的工作：保留冒烟、正对照与 E1 原始日志，复查并修订了 `rtex` 配置，补完负对照、E1 汇总、E3 准备、驱动 README 与本记录。
- `spikes/toolchain-sandbox.sh` 只新增 `rtex` 配置，既有配置的行为不变：
  - 只读挂载 rtex 二进制（`/rtex/bin/rtex`）与它的 `tex/` 支持目录（`/rtex/tex`）、`lualatex`/`luahbtex`/`kpsewhich`/`prlimit` 及其动态库、字体与 fontconfig、texmf 树（`/usr/share/texmf-dist`、`/usr/share/texmf`、`/etc/texmf`、`/var/lib/texmf`）、locale；项目只读挂在 `/project`，构建目录可写挂在 `/work`；没有 `/bin/sh`，没有 `$HOME`，`--unshare-all` 带来独立的网络命名空间，`--clearenv` 清空主机环境。
  - rtex 编译的是“保存的导言区 + 只有正文的快照”，快照与原文件同名，所以 `TEXINPUTS` 必须先查当前目录：`rtex` 配置用 `.:/project:`。第一次正对照没有这一点，沙箱内两份夹具都报 `Two \documentclass`（[首次正对照](evidence/rtex-evaluation/initial-positive/positive-summary.json)）。原先 `TEXINPUTS` 在命令行里设了两次（后者覆盖前者），已改为一个变量只设一次。
  - 新增仅评估用的 `SCHOLIUM_RTEX_EVAL_TEXBIN`：由可信驱动传入一个目录，只读挂到 `/rtex/texbin` 并作为 `RTEX_TEXLIVE_BIN`。负对照 (c)(e) 用它换上 [lift/lualatex.c](../../spikes/rtex-eval/lift/lualatex.c)（给 `lualatex` 加 `--shell-escape`，同时启用 LuaSocket）。文档内容无法设置它，产品代码不得使用它。
- 驱动（Python 标准库，不进根 workspace，没有新增 Rust 代码）：`session.py` 以 JSON 行驱动长驻 `rtex serve`，完整 stdin/stdout 存为 gzip JSONL；`negative.py`/`canary.py`/`runaway.py`/`common.py` 是负对照；`cjk_check.py` 是 E3 准备。

## 冒烟与正对照

命令：`python3 spikes/rtex-eval/smoke.py …`、`python3 spikes/rtex-eval/positive.py … --run-id positive-final`（完整命令见 [README](../../spikes/rtex-eval/README.md)）。正对照在最终版配置上重跑过一次，证据为重跑结果。

| 项 | 观察 | 结果 | 证据 |
|---|---|---|---|
| `rtex doctor`（沙箱外 / 沙箱内） | 两处都找到支持文件与 LuaHBTeX 1.24.0 | Pass | [沙箱外](evidence/rtex-evaluation/smoke-doctor.log)、[沙箱内](evidence/rtex-evaluation/profile-doctor.log) |
| `rtex gen-book --pages 10`、`rtex edit` | 生成 10 页书；编辑后收到段落更新 | Pass | [gen-book](evidence/rtex-evaluation/smoke-gen-book.log)、[edit](evidence/rtex-evaluation/smoke-edit.log) |
| `rtex verify` | 54/54 单元可走实时路径，23142/23142 字形一致，10 页逐页精确 | Pass | [verify](evidence/rtex-evaluation/smoke-verify.log)、[JSON](evidence/rtex-evaluation/verify-result.json) |
| 正对照：10 页书 | 沙箱外/内：初次布局 10 页，编辑走 fast 路径，`ParagraphUpdate` ok，导出收敛；导出 10 页，文本 sha256 均为 `721fdac5…` | Pass | [汇总](evidence/rtex-evaluation/positive-summary.json)、[沙箱外 JSONL](evidence/rtex-evaluation/positive-book-host.jsonl.gz)、[沙箱内 JSONL](evidence/rtex-evaluation/positive-book-sandbox.jsonl.gz) |
| 正对照：手写文章（英文、公式、`\ref`、图） | 同上；1 页，无 `??`，文本 sha256 均为 `8e71b779…` | Pass | [沙箱外文本](evidence/rtex-evaluation/positive-article-host.txt)、[沙箱内文本](evidence/rtex-evaluation/positive-article-sandbox.txt) |

## E2 沙箱：负对照

命令：`python3 spikes/rtex-eval/negative.py --clone ~/.cache/scholium-eval/rtex-v0.0.5 --evidence docs/log/evidence/rtex-evaluation/negative --run-id negative`（约 7 分钟，[运行输出](evidence/rtex-evaluation/negative.log)，[完整汇总](evidence/rtex-evaluation/negative/negative-summary.json)）。

方法：

- 每组先在沙箱外跑同一个 rtex（不加沙箱），再在 `rtex` 配置里跑；两次都是长驻 `rtex serve`：等初次布局 → 在探针段落里编辑一次（让实时路径也执行探针）→ 导出 PDF。
- 驱动每次运行前在 `$HOME/.scholium-e2-<随机>/` 放金丝雀文件，设置主机环境变量 `E2_HOST_ENV_CANARY`，在 `127.0.0.1` 随机端口开一个监听器（有人连上就发一个令牌）；运行结束后全部删除。秘密值是随机令牌，不出现在夹具源码里，路径用另一个随机串，所以回显源码不会被误判为泄漏。`/etc/passwd` 以其第一行为秘密。
- 泄漏检查四个通道：导出 PDF 的文本、构建目录里的全部文件（含日志，PDF 另做文本提取）、stderr、stdout 的 JSON 事件。副作用检查：构建目录外的标记文件、项目目录里的写入、监听器连接数、运行后残留的进程。
- 判定：沙箱外必须至少有一项泄漏或副作用（证明攻击真实有效），否则记为“不确定”并重做；沙箱内任何泄漏、副作用或残留进程即 Fail。
- 两种配置：`default` 是原样的 rtex（LuaTeX 受限 shell escape，不加载 LuaSocket）；`lifted` 用评估启动器加 `--shell-escape`，模拟 TeX 自身限制失效的主机（用户开了 shell escape、rtex 将来加参数或 LuaTeX 漏洞），让探针打到沙箱上。

| 对照 | 配置 | 探针（夹具） | 期望 | 沙箱外观察 | 沙箱内观察 | 结果 | 证据 |
|---|---|---|---|---|---|---|---|
| (a) 读白名单外文件 | default | A1 `io.open("/etc/passwd")`；A2 读 `$HOME` 金丝雀；A3 `os.getenv` 读主机环境金丝雀（[夹具](../../spikes/rtex-eval/fixtures/negative/a-read.tex)） | 读不到，四个通道都没有秘密 | 三项全部读到，出现在 PDF 文本与构建日志里 | 三项都是 `DENIED`（文件不存在、环境变量未设置）；无泄漏 | **Pass** | [外](evidence/rtex-evaluation/negative/negative-a-default-host.pdf.txt)、[内](evidence/rtex-evaluation/negative/negative-a-default-sandbox.pdf.txt)、[内 JSONL](evidence/rtex-evaluation/negative/negative-a-default-sandbox.jsonl.gz) |
| (b) `\input` 绝对路径 | default | `\input{$HOME/…/secret-input.tex}`（[夹具](../../spikes/rtex-eval/fixtures/negative/b-input.tex)） | 读不到 | 金丝雀内容排进 PDF | ``LaTeX Error: File `…' not found``，整篇编译失败、导出失败（失败即关闭）；无泄漏 | **Pass** | [外](evidence/rtex-evaluation/negative/negative-b-default-host.pdf.txt)、[内 JSONL](evidence/rtex-evaluation/negative/negative-b-default-sandbox.jsonl.gz) |
| (c) shell escape | default | C1 `io.popen("cat 金丝雀")`；C2 `os.execute("touch …")`；C3 `\immediate\write18{touch …}`（经 shellesc）；C4 `io.popen("kpsewhich -var-value=E2_HOST_ENV_CANARY")`（[夹具](../../spikes/rtex-eval/fixtures/negative/c-shell.tex)） | 子进程拿不到主机数据，不产生文件 | C1–C3 已被 LuaTeX 受限模式挡住；**C4 生效**：白名单内的 `kpsewhich` 在主机上运行并把环境金丝雀带回 PDF | C4 失败（沙箱里没有 `/bin/sh`），无泄漏、无文件 | **Pass** | [外](evidence/rtex-evaluation/negative/negative-c-default-host.pdf.txt)、[内](evidence/rtex-evaluation/negative/negative-c-default-sandbox.pdf.txt) |
| (c) shell escape | lifted | 同上 | 同上 | 全部生效：`cat` 读出金丝雀，`os.execute` 与 `\write18` 在 `$HOME` 下建了文件，`kpsewhich` 带回环境金丝雀 | C1 无输出，C2 返回 32512（127，找不到 shell），C4 失败；无泄漏、无文件 | **Pass** | [外](evidence/rtex-evaluation/negative/negative-c-lifted-host.pdf.txt)、[内](evidence/rtex-evaluation/negative/negative-c-lifted-sandbox.pdf.txt) |
| (d) 写出构建目录（Lua） | default | D1 `io.open($HOME/…, "w")`；D2 写项目输入目录（主机路径与 `/project`）（[夹具](../../spikes/rtex-eval/fixtures/negative/d-write.tex)） | 不产生文件 | `$HOME` 与项目目录里都写出了文件 | 三处都是 `DENIED`（`$HOME` 不存在、`/project` 只读）；无文件 | **Pass** | [外](evidence/rtex-evaluation/negative/negative-d-lua-default-host.pdf.txt)、[内](evidence/rtex-evaluation/negative/negative-d-lua-default-sandbox.pdf.txt) |
| (d) 写出构建目录（`\openout`） | default | `\immediate\openout` 绝对路径到 `$HOME`（[夹具](../../spikes/rtex-eval/fixtures/negative/d-openout.tex)） | 不产生文件 | 写出了文件（rtex 设了 `openout_any=a`，见发现 1） | 打开失败，整篇编译中止、导出失败；无文件 | **Pass** | [外 JSONL](evidence/rtex-evaluation/negative/negative-d-openout-default-host.jsonl.gz)、[内 JSONL](evidence/rtex-evaluation/negative/negative-d-openout-default-sandbox.jsonl.gz) |
| (e) 网络 | default | `require("socket")` 后连驱动的回环监听器（[夹具](../../spikes/rtex-eval/fixtures/negative/e-network.tex)） | 连不上 | `module 'socket' not found`：LuaTeX 只有 `--socket`/`--shell-escape` 才加载 LuaSocket，探针在沙箱外就失败 | 同左 | 不确定（沙箱外未生效，改用 lifted 重做） | [外](evidence/rtex-evaluation/negative/negative-e-default-host.pdf.txt)、[内](evidence/rtex-evaluation/negative/negative-e-default-sandbox.pdf.txt) |
| (e) 网络 | lifted | 同上 | 连不上 | 连上并收到令牌，监听器记录 6 次连接（后台、实时、导出各编译一次以上） | `connection refused`（独立网络命名空间），监听器 0 次连接 | **Pass** | [外](evidence/rtex-evaluation/negative/negative-e-lifted-host.pdf.txt)、[内](evidence/rtex-evaluation/negative/negative-e-lifted-sandbox.pdf.txt) |
| (f) 死循环 | default | 编辑插入 `\directlua{while true do end}`，等后台排程后再停 5 秒，删掉它并编辑另一段（[夹具](../../spikes/rtex-eval/fixtures/negative/f-runaway.tex)） | 单元超时，编辑继续，CPU 有上界 | 实时路径 5 秒看门狗杀掉服务器并重启；另一段编辑约 0 秒后返回 ok；**但后台整编带着死循环无限运行**：150 秒内布局未恢复，60 秒内导出未完成，采样时该 lualatex 已用 154.8 CPU 秒 | 看门狗 5 秒切断；另一段编辑立即返回 ok；后台两轮各被 `RLIMIT_CPU` 在约 30 秒时杀掉，第 69.1 秒导出成功，PDF 含后续编辑、不含死循环；无残留进程 | **Pass** | [外 JSONL](evidence/rtex-evaluation/negative/negative-f-default-host.jsonl.gz)、[内 JSONL](evidence/rtex-evaluation/negative/negative-f-default-sandbox.jsonl.gz) |

所有运行结束后都没有残留进程，`$HOME` 下的金丝雀目录已删除。沙箱外运行写出的标记文件都在金丝雀目录或驱动的工作目录里，随之删除。

## E1 许可与依赖

命令（用本仓库的 `deny.toml`，advisories 单独运行、不过滤）：

```bash
cargo deny --manifest-path ~/.cache/scholium-eval/rtex-v0.0.5/Cargo.toml --config "$PWD/deny.toml" check --show-stats licenses bans sources
cargo deny --manifest-path ~/.cache/scholium-eval/rtex-v0.0.5/Cargo.toml --config "$PWD/deny.toml" check --show-stats advisories
```

| 检查 | 结果 | 观察 | 证据 |
|---|---|---|---|
| licenses | Pass | 0 错误；8 条警告都是本仓库允许清单里没用到的许可；rtex 自身为 MIT。许可分布见清单 | [策略日志](evidence/rtex-evaluation/deny-policy.log)、[许可清单](evidence/rtex-evaluation/deny-list.log) |
| sources | Pass | 只有 crates.io，没有 git 依赖 | 同上 |
| bans | Fail（3 条） | 都是 `wildcard`：rtex 自己的 `rtex-cli`、`rtex-core`、`rtex-verify` 用不带版本号的路径依赖引用 `rtex-dl`/`rtex-core`/`rtex-verify`，又没有 `publish = false`，所以 `allow-wildcard-paths` 不适用。第三方依赖没有命中 | 同上 |
| advisories（单独报告） | Fail（2 条） | **RUSTSEC-2026-0187**：`lopdf 0.36.0` 解析深层嵌套 PDF 时栈溢出中止（≥0.42.0 修复），经 `rtex-core`（图片缓存读取每轮的 PDF）与 `rtex-verify` 引入；文档可以往 LuaLaTeX 的输出里写原始 PDF 对象，推测能借此让 rtex 进程崩溃，未验证。**RUSTSEC-2026-0192**：`ttf-parser 0.25.1` 不再维护，只经 `rtex-verify` 引入 | [advisories 日志](evidence/rtex-evaluation/deny-advisories.log) |
| 原生代码 | 已登记：Linux 上无 | `cc` 只是 `iana-time-zone-haiku`（仅 Haiku）的构建依赖；`-sys` crate 只有 `core-foundation-sys`（macOS）、`js-sys`（wasm）、`windows-sys`（Windows），Linux 目标下一个都不进依赖图；Linux release 构建的 44 个构建脚本目录里没有任何一个产出 `rustc-link-lib`/`.a`/`.o`；二进制只链接 libc、libm、libgcc_s。rtex-core 自带的 C ABI（`ffi.rs`）是导出接口，不是原生依赖 | [原生代码清单](evidence/rtex-evaluation/native-code.log)、[构建脚本候选](evidence/rtex-evaluation/native-candidates.json) |
| TeX Live 边界 | 满足 | rtex 以子进程调用用户安装的 `$RTEX_TEXLIVE_BIN/lualatex`，文档需要时再调用 bibtex/biber/makeindex；TeX Live 不随 rtex 分发。`rtex verify` 的栅格比较另外调用 Python（`RTEX_PYTHON`），只有 verify 用到 | — |

## E3 准备

命令：`python3 spikes/rtex-eval/cjk_check.py --clone ~/.cache/scholium-eval/rtex-v0.0.5 --evidence docs/log/evidence/rtex-evaluation/e3`（[汇总](evidence/rtex-evaluation/e3/e3-summary.json)）。

任务冻结时 `kpsewhich luatexja.sty` 为空；本次运行时它已存在：`texlive-langjapanese 2026.1-1`（安装时间 2026-10-30 08:08，任务冻结之后），ctex 与 Fandol 字体来自 `texlive-langchinese 2026.1-1`。所以 E3 **不是 Blocked**，只是本轮未判定。

| 夹具 | LuaLaTeX 编译 | 缺字形 | PDF 含中文 | 沙箱外 `rtex verify`（初步） | 证据 |
|---|---|---|---|---|---|
| [ctex（LuaLaTeX）](../../spikes/rtex-eval/fixtures/cjk/ctex.tex) | 退出码 0 | 0 | 是 | 4/4 单元可走实时路径，174/174 字形一致，页面精确 | [编译](evidence/rtex-evaluation/e3/e3-ctex-lualatex.log)、[verify](evidence/rtex-evaluation/e3/e3-ctex-verify.log) |
| [纯 luatexja](../../spikes/rtex-eval/fixtures/cjk/luatexja.tex) | 退出码 0 | 0 | 是 | 4/4，188/188 字形一致，页面精确 | [编译](evidence/rtex-evaluation/e3/e3-luatexja-lualatex.log)、[verify](evidence/rtex-evaluation/e3/e3-luatexja-verify.log) |

两份夹具都含中英文、行内与行间公式、`\ref`。纯 luatexja 的默认日文字体缺很多简体字形，夹具用 `luatexja-fontspec` 选了 Fandol。实时单元占比、单段 p50/p95 和沙箱内运行留给 E3 正式评估。

## 发现与局限

1. **rtex 关闭了 TeX 自己的写保护。** `rtex-core/src/texlive.rs` 给 LuaLaTeX 设 `openout_any=a`（TeX Live 默认是 `p`），所以沙箱外 `\openout` 可以写任何用户可写路径；Lua `io.open` 本来就不受 kpathsea 限制。结合 (a)–(d)，**rtex 不能在沙箱外处理不受信文档**，沙箱是唯一的边界。
2. **TeX 受限 shell escape 不是隔离。** 默认配置下白名单命令（`kpsewhich` 等）仍在主机上执行，C4 借它带回了主机环境变量。沙箱里没有 `/bin/sh`，`io.popen`/`os.execute`/`\write18` 全部失败；即使解除 TeX 限制（lifted）也一样。
3. **后台整编没有超时。** 实时路径有 5 秒看门狗，但后台 pass 只等子进程退出：沙箱外一个死循环单元会无限占用一个 CPU，布局与导出永远不更新。沙箱内由每进程 `RLIMIT_CPU`（30 秒）兜底，但 rtex 先后两轮从带死循环的快照重跑，后台被堵约 60 秒；而且被杀掉的两轮仍以 `LayoutUpdate`、编译状态 `Ok`、没有诊断的形式上报（`wall_ms` 29758、59525）。这属于 E8，需要 rtex 侧的后台超时与失败上报，或宿主对每次编译单独计时。
4. **现有资源限制与长驻会话不匹配（产品化问题，本轮未放宽任何既有配置）。** `timeout <秒>` 是整个会话的墙钟上限，驱动给的是 150 秒（(f) 用 300 秒），到点会杀掉整个 `rtex serve`；`prlimit --cpu=30` 是每进程累计 CPU，长驻的 rtex 与 LuaLaTeX 服务器在长时间编辑后会被杀掉；`systemd-run --scope` 的 MemoryMax=2G、TasksMax=64 作用于整个会话。产品化需要会话级与单次编译级分开的限制（例如每次编译一个 cgroup 或由 rtex 负责超时）。
5. **沙箱内首次布局慢约 8 秒**（文章 0.5 → 8.5 秒，书 1.8 → 9.8 秒，负对照同样约 10 秒）。推测是 `TEXMFVAR` 在每次会话新建的 tmpfs 上，luaotfload 的字体缓存每次重建；未验证。产品化可考虑按用户持久化的字体缓存目录。
6. **挂载面。** `rtex` 配置挂了整棵 texmf 树、字体、`/etc/texmf`、`/var/lib/texmf`、`/var/cache/fontconfig` 与 locale，均只读；没有 `$HOME`、`/etc/passwd`、shell 或网络。项目只读挂载下正对照仍通过，说明 rtex 不需要写项目目录。
7. **评估专用开关。** `SCHOLIUM_RTEX_EVAL_TEXBIN` 只用于 (c)(e) 的 lifted 配置，由可信驱动设置；产品代码不得设置，正式接入时应删除或移出产品路径。
8. **没有覆盖的面**：项目内指向外部的符号链接、读取 `/proc` 下进程信息、超大输出与内存耗尽、bibtex/biber/makeindex 路径（夹具不需要，配置也没挂载）、Windows/macOS 沙箱。`lopdf` 的崩溃风险只做了推断。
9. 沙箱内 Lua `io.open` 失败时错误信息为 `nil`（没有 errno 文本），只影响可读性。

## 门禁

- `bash spikes/verify-stage0.sh security`：见 [运行前](evidence/rtex-evaluation/security-before.log) 与 [运行后](evidence/rtex-evaluation/security-after.log)。运行前 7 项里 5 项因为本槽位的私有 `CARGO_HOME`（`spikes/native-ui/.cargo-home`）没有离线缓存而失败，与沙箱无关；本次先用同一个 `CARGO_HOME` 对 `spikes/untrusted-input`、`spikes/mixed-build`、`spikes/recovery` 执行 `cargo fetch --locked` 补齐缓存再运行，7 项全部通过（untrusted-input、mixed-build compiler isolation、Typst worker isolation、PDF artifact probe isolation、runtime mount allowlist、process tree resource limits、recovery compiler isolation）；此前通过的两项（runtime mount allowlist、process tree resource limits）仍通过。
- `python3 scripts/check-doc-links.py`：通过。
- 没有新增 Rust 代码，Rust 门禁与真实窗口检查不适用。

## 待办

- E3 正式评估：实时单元占比、单段 p50/p95、沙箱内运行（含字体缓存的影响）。
- E8：后台 pass 超时与“被杀仍报 Ok”的问题反馈上游或在宿主侧兜底；会话级与编译级资源限制的设计。
- E1：bans 的路径依赖版本号与 `lopdf` 升级可反馈上游；按实际接入方式（子进程二进制还是库）重新跑一次 `cargo deny`。
- 补测第 8 条列出的未覆盖面。
