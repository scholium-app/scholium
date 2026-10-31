# 2026-10-30：rtex 评估第二轮（E3–E8）

本记录只追溯第二轮评估做了什么、怎么验证、观察到什么；评估项与判据见 [LaTeX 工作区规划](../plan/LATEX_WORKSPACE.md)
的 L0（E1–E8），第一轮（E1、E2、E3 准备）见 [第一轮记录](2026-10-30-rtex-evaluation.md)，背景见
[ADR 0041](../adr/ADR-0041-separate-latex-typst-kernels.md)。选型 ADR 由协调者另立，文末“选型建议”只是执行者的建议。
驱动、夹具、栅格化原型与复现命令见 [spikes/rtex-eval](../../spikes/rtex-eval/README.md)，原始证据在
[evidence/rtex-evaluation-2](evidence/rtex-evaluation-2/)。

## 结论摘要

| 项 | 判定 | 一句话 |
|---|---|---|
| E3 中文 | **Pass**（条件：持久字体缓存） | ctex、luatexja、6 页中英混排三份夹具实时单元 4/4、4/4、30/30；各 120 次击键全部走实时路径；p50/p95 为 3.7/5.3、2.9/4.5、5.5/9.8 ms；沙箱内 `rtex verify` 逐字形一致。不带持久字体缓存时首次布局约 13 s，luatexja 会话三次全部被 2 GiB 内存上限杀掉 |
| E4 绘制 | **有条件通过** | 原型把 display list 画成位图：拉丁页 150 dpi 最差 0.45 %；中文页 150 dpi 对 pdftoppm 为 1.10–1.54 %（超 1 %），但对 pdftocairo ≤ 0.68 %、对 hayro ≤ 0.27 %，而 poppler 两个后端之间就差 1.27–1.66 %；300 dpi 全部 ≤ 0.014 %。TikZ 页经 hayro 回退：0.68 %（150 dpi）/ 0.010 %（300 dpi） |
| E5 定位 | **Pass**（说明可支撑的交互） | 页面点击 → 段落（span 字节范围）288/288 命中；行号精确，但行内字节要靠推算；字形级要宿主自己做“字形 ↔ 源码”对齐：标记处 288/288 精确，正文字形 97.8 %/99.1 % 能精确对齐，TFM 数学字形 0 % |
| E6 端到端（宿主侧） | **Pass**（真实窗口留 L1） | 100 页：击键 → ParagraphUpdate p50/p95 10.6/30.9 ms，→ 该单元栅格完成 19.4/52.1 ms；10 页 6.7/18.7 与 12.5/35.2 ms；edit 回复 p50 ≤ 0.7 ms，但 100 页布局装入时有两次约 93 ms 的停顿 |
| E7 非 LuaLaTeX 回退 | **Pass**（测得数字 + 设计说明） | `full` 配置一次整编：XeLaTeX（ctex）1/11/49 页 1.8/1.9/2.2 s，pdfLaTeX 1/9/41 页 0.9/0.7/0.7 s（其中沙箱启动约 0.5 s）；打开项目（两遍）3.5–4.2 s 与 1.4–1.7 s |
| E8 故障 | **未通过（原样）**；宿主兜底后可接受 | 实时引擎被杀 0.7 s 恢复，但在途那次编辑既无更新也不排整编；实时挂起时其他段落 5 s 无更新，挂起段落此后整场会话只走整编；后台挂起无宿主超时时布局 57 s 后才恢复，被杀 pass 两次都报 `Ok`；宿主 8 s 看门狗把它缩到 14.3 s；导言区错误修好后 1.1–1.5 s 恢复；非 UTF-8 边界的偏移让 `rtex serve` panic 退出 |
| 资源限制 | 设计说明 | 会话级 cgroup + 宿主单次 pass 墙钟超时 + 长驻进程不设 RLIMIT_CPU，依据见“资源限制设计” |

## 环境

| 项 | 值 |
|---|---|
| rtex | v0.0.5，commit `e0ad11765ec016846b6e07619081f0528621636b`，沿用第一轮在 `~/.cache/scholium-eval/rtex-v0.0.5` 的构建，未改动；上游代码未进入仓库 |
| TeX Live | TeX Live 2026（Arch）：LuaHBTeX 1.24.0、XeTeX 0.999998、pdfTeX 1.40.29；texlive-langchinese、texlive-langjapanese、pgf |
| 系统 | Arch Linux，内核 7.2.11-arch1-1，20 核，30 GiB；bubblewrap 0.13.1；Python 3.14.8；rustc 1.99.0；cargo-deny 0.20.2；poppler 26.10.0（只作参考栅格器）；ImageMagick 7.1.2 |
| 沙箱 | E3–E6、E8 在 `rtex` 配置内，E7 在 `full` 配置内；资源限制沿用第一轮（会话 scope MemoryMax=2G、TasksMax=64；每进程 RLIMIT_CPU 30 s、AS 4 GiB、FSIZE 64 MiB）。沙箱外数字只作参考并标明 |
| 计时 | 宿主单调时钟。击键时延 = 写 edit 命令之前 → 读到同一 `edit_id` 的 `ParagraphUpdate` 行之后（JSON 解析之前）；rtex 自报的 `timing.total_us` 另列 |

## 做了什么

- 驱动（仍只用 Python 标准库，不进根 workspace）：`session.py` 增加接收时间戳、可调的输出预算（100 页布局一行约 14 MB）、转录脱敏（display list 只记字节数，保证证据小）与沙箱 cgroup 的 `memory.peak`；新增 `measure.py`（分位数、脚本化击键、栅格化工作进程、字体缓存预热）、`gen_fixtures.py`、`faults.py` 与 `e3_cjk.py`…`e8_faults.py`。
- 栅格化原型 [`spikes/rtex-eval/raster/`](../../spikes/rtex-eval/raster/Cargo.toml)：独立 Cargo 清单和 `[workspace]`，不是根 workspace 成员，根目录 `cargo metadata` 的成员列表前后一致。读 `rtex serve` 的 JSON display list，用 skrifa 取字形轮廓、tiny-skia 填充；TFM 字体（经典 CM 数学，display list 只给 `cmmi10` 这类名字）按 LuaTeX 后端的做法经 `pdftex.map` 找 Type 1 文件和编码向量，用 skrifa 的 Type 1 解析器画；`hayro` 渲染 PDF 回退；墨迹比较按上游 `docs/correctness.md` 的口径重写（阈值 96、1 像素容差、未匹配墨迹 ≤ 1 %）。依赖 hayro 0.8.0、skrifa 0.46.0、tiny-skia 0.12.0、serde/serde_json；用本仓库 `deny.toml` 跑 licenses/bans/sources/advisories 全部通过，依赖图 98 个 crate，没有 `-sys`/`cc`（[记录](evidence/rtex-evaluation-2/e4/raster-deny.log)）。没有使用上游已不维护的 ttf-parser（RUSTSEC-2026-0192），也没有复制上游代码。
- 沙箱脚本：`spikes/toolchain-sandbox.sh` 只在 `rtex` 配置里新增 opt-in 的 `SCHOLIUM_RTEX_FONT_CACHE`：由可信驱动设置时，把该目录可写挂到 `/rtex/texmf-var` 并设为 `TEXMFVAR`；不设置时行为与第一轮相同。中途试过的只读挂载和“只读缓存在前、tmpfs 在后”的 `TEXMFCACHE` 两种变体都不可用（见 E3），已从脚本删除。
- 夹具（全部原创）：`gen_fixtures.py` 生成中英混排的 ctex 文章（带节、行内与显示公式、`\eqref`、列表、定理）、英文 pdfLaTeX 文章和 TikZ 文章，每段一个唯一标记词 `QZxx` 供 E5 作基准；书用 `rtex gen-book`（Pagella）；第一轮的 ctex、luatexja、手写文章与死循环夹具照用。
- 驱动自身的两个错误在本轮内发现并修正：E5 起初把 page list 里的 `unit` 当作 span id（实际是布局 uid，要用 `units` 命令映射；修正后行与 `placements` 394/394、164/164 一致）；E8 起初的击键函数没有读掉“删除”那次编辑的回复，且等待时会丢掉不匹配的消息，所以 E8 的时间线最后全部从转录（保留每条消息的接收时间）重建。表中数字都来自修正后的运行。

## E3 中文

命令：`e3_cjk.py --run-id e3`、`--followup`、`--warmed-luatexja`（[汇总](evidence/rtex-evaluation-2/e3/e3-summary.json)、[复测](evidence/rtex-evaluation-2/e3/e3-followup.json)、[预热后的 luatexja](evidence/rtex-evaluation-2/e3/e3-luatexja-warmed.json)）。

方法：每份夹具在 `rtex` 沙箱内开一个长驻 `rtex serve`，等收敛的布局，用 `spans` 取单元（不计导言区与 `\end{document}` 尾段）。实时单元占比 = 布局 `eligible_paragraphs` 中的单元 / 全部单元。击键：轮流在各单元正文中部（行内公式之外、一个汉字之前）插入“字”再删除，共 120 次，每次等到这次编辑自己的更新后停 150 ms。`rtex verify`（allow-list、逐字形比较与页面对 PDF）在同一沙箱配置里单独运行一次。

| 夹具 | 实时单元 | 击键路由 / 状态 | 击键 → 更新 p50 / p95（最大） | rtex 自报 p50 / p95 | 沙箱内 `rtex verify` | 结果 | 证据 |
|---|---|---|---|---|---|---|---|
| ctex（LuaLaTeX，1 页） | 4/4（3 段 + 1 标题） | 120 fast / 120 ok | 3.7 / 5.3 ms（11.8） | 3.1 / 4.7 ms | 4/4 单元，174/174 字形一致，1/1 页精确，带采集包的 PDF 与干净构建相同 | **Pass** | [击键](evidence/rtex-evaluation-2/e3/e3-ctex-sandbox-edits.json)、[verify](evidence/rtex-evaluation-2/e3/e3-ctex-verify-sandbox.json) |
| 纯 luatexja（1 页） | 4/4 | 120 fast / 120 ok | 2.9 / 4.5 ms（358，首次） | 2.3 / 3.5 ms | 4/4，188/188 一致，1/1 页精确 | **Pass**（须预热缓存，见下） | [击键](evidence/rtex-evaluation-2/e3/e3-luatexja-sandbox-warmed-copy-edits.json)、[verify](evidence/rtex-evaluation-2/e3/e3-luatexja-verify-sandbox.json) |
| 中英混排 ctex 文章（6 页） | 30/30（18 段、9 环境、3 标题） | 120 fast / 120 ok | 5.5 / 9.8 ms（11.5） | 4.2 / 8.3 ms | 30/30，5510/5510 一致，6/6 页精确 | **Pass** | [击键](evidence/rtex-evaluation-2/e3/e3-mixed-sandbox-edits.json)、[verify](evidence/rtex-evaluation-2/e3/e3-mixed-verify-sandbox.json) |
| 同上，沙箱外（参考） | 30/30 | 120 fast / 120 ok | 4.8 / 10.0 ms（97） | 3.9 / 8.2 ms | — | 参考 | [击键](evidence/rtex-evaluation-2/e3/e3-mixed-host-edits.json) |

字体缓存（第一轮发现 5）。“首次布局”= 启动 `rtex serve` 到第一条 `LayoutUpdate`；内存为沙箱 scope 的 `memory.peak`：

| 设置 | ctex | 混排 6 页 | luatexja | 证据 |
|---|---|---|---|---|
| 每会话新 tmpfs（现状，两次） | 12.8 / 13.2 s，1.44–1.45 GiB | 14.0 / 14.6 s，1.49–1.50 GiB | **3 次全部被 OOM 杀掉**（scope 峰值 2 GiB，`journalctl` 记 `oom-kill`），会话 EOF | [复测](evidence/rtex-evaluation-2/e3/e3-followup.json) |
| 持久目录，可写，第一次（冷） | 13.5 s | 20.6 s | OOM 同上 | 同上 |
| 持久目录，可写，第二次（暖） | 1.4 s，0.78 GiB | 1.7 s，0.81 GiB | — | 同上 |
| 暖缓存的会话副本（宿主复制 23–74 ms 后可写挂载） | 1.5 s，0.79 GiB | 1.7 s，0.81 GiB | 1.7 s，0.91 GiB | 同上、[预热后](evidence/rtex-evaluation-2/e3/e3-luatexja-warmed.json) |
| 只读挂载暖缓存（第一次运行时的变体） | 失败：luaotfload `no writeable cache path, quiting`，布局 `Failed` | 同左 | — | [转录](evidence/rtex-evaluation-2/e3/e3-cache-ctex-ro-warm.jsonl.gz) |
| 只读缓存在前、tmpfs 在后（`TEXMFCACHE`） | 11.5 s（字体名数据库只在可写路径查找，仍重建） | — | — | 一次试验，未留证据 |
| 沙箱外（用户自己的缓存，参考） | 1.3 s | 1.5 s | 1.4 s | 同上 |

- 首次布局慢约 12 s 正是字体缓存所致（第一轮发现 5 得到证实）：三个并发的 LuaLaTeX（实时引擎、待命引擎、采集 pass）各自从头建 luaotfload 缓存，CPU 约 38–43 s、内存 1.4–1.5 GiB；luatexja 再加 Fandol 字体时超过 2 GiB。
- 可用的做法是“可信预热 + 会话副本”：先用单个普通 LuaLaTeX 进程在沙箱里预热一个按用户保存的缓存目录（luatexja 18.3 s，缓存 21.7 MB），每次会话前宿主复制一份可写挂载；文档只能写这份副本，写不到共享缓存。持久目录直接可写挂载同样快，但任何文档都能改写下一个项目要读的缓存（含 Lua 字节码），不建议。
- luatexja 预热前的那次“副本”运行（[记录](evidence/rtex-evaluation-2/e3/e3-followup.json) 中 `sandbox-cache-copy`）用的是被 OOM 打断后残留的半成品缓存，数字（p50/p95 4.1/5.0 ms）只作旁证，表中采用预热后的运行。

## E4 绘制

命令：`e4_draw.py --run-id e4`（[汇总](evidence/rtex-evaluation-2/e4/e4-summary.json)）。方法：每份夹具在沙箱内跑到收敛，汇集各页 page display list，用原型栅格化；参考为该布局自己的 PDF（`bg/layout-N.pdf`）用 poppler `pdftoppm -gray`（判据参考）、`pdftocairo`（第二参考）与 hayro（候选回退）渲染。每页两轮栅格，第一轮含字体加载（冷），第二轮为暖。

| 夹具 | 页 | 150 dpi：display list 对 pdftoppm（最差） | 对 pdftocairo / 对 hayro | pdftoppm 对 pdftocairo | 300 dpi：对 pdftoppm | 跳过字形 | 结果 |
|---|---|---|---|---|---|---|---|
| 10 页书（Pagella，OpenType 数学） | 10 | 0.024 % | 0.0008 % / 0.002 % | 0.016 % | 0.009 % | 0 | **Pass** |
| 手写文章（Latin Modern + CM 数学 Type 1，picture 环境） | 1 | 0.26 % | 0.11 % / 0.13 % | 0.35 % | 0.000 % | 0 | **Pass** |
| ctex | 1 | **1.54 %** | 0.63 % / 0.07 % | 1.66 % | 0.009 % | 0 | 150 dpi 对 pdftoppm 超限；300 dpi Pass |
| 纯 luatexja | 1 | **1.34 %** | 0.68 % / 0.27 % | 1.27 % | 0.010 % | 0 | 同上 |
| 中英混排（6 页中 5 页 > 1 %） | 6 | **1.50 %** | 0.52 % / 0.06 % | 1.55 % | 0.014 % | 0 | 同上 |
| TikZ 文章第 1 页（正文） | 1 | 0.45 % | 0.17 % / 0.15 % | 0.44 % | 0.072 % | 1（见下） | 墨迹 Pass |
| TikZ 文章第 2 页（图，`exact: false`，布局给出 `pdf_fallback`） | 1 | display list 58 %（画不出路径，预期） | — | 0.51 % | — | 0 | **回退 Pass**：hayro 对 pdftoppm 0.68 %（150 dpi）/ 0.010 %（300 dpi） |

- 中文页在 150 dpi 超过 1 % 不是 display list 的位置误差：同一页 display list 对 hayro 只差 0.03–0.27 %，对 pdftocairo 0.37–0.68 %，而 poppler 自己的两个后端彼此差 1.27–1.66 %——宋体细笔画的抗锯齿覆盖率正好落在墨迹阈值附近，参考栅格器之间的差异已经超过 1 % 预算。300 dpi（HiDPI 屏幕的实际密度）时所有组合都 ≤ 0.035 %。上游用 MuPDF 作参考，本仓库排除 AGPL 的 MuPDF，没有对照。严格按“150 dpi、对 pdftoppm”的字面判据，中文页不通过；产品按 2 倍密度绘制时满足。
- TikZ 第 1 页那个“跳过字形”是一条字体记录全为空（id 1，字符 `P`）的字形，PDF 里那里也没有墨迹，按墨迹判据不影响；但它说明 display list 里可能出现无法描述的不可见项，宿主应计数而不报错。
- 截图：[混排第 1 页](evidence/rtex-evaluation-2/e4/e4-mixed-page1-dl-vs-poppler.png)、[ctex](evidence/rtex-evaluation-2/e4/e4-ctex-page1-dl-vs-poppler.png)、[TikZ 回退页](evidence/rtex-evaluation-2/e4/e4-tikz-page2-hayro-vs-poppler.png)（左为本原型或 hayro，右为 pdftoppm，缩到 75 dpi）。

计时（每页，单线程）：

| | 150 dpi | 300 dpi |
|---|---|---|
| display list 栅格，暖（书每页约 2200 字形） | p50 4–30 ms（书 18–30 ms，两次运行；单页文章 4–9 ms） | 书 p50 51 ms |
| display list 栅格，冷（含字体加载） | 拉丁 OpenType 27 ms；含 CJK 或 Type 1 的首页 0.41–0.96 s（Fandol 字体解析、`kpsewhich` 查 `pdftex.map`） | 0.38–0.86 s |
| hayro 渲染 PDF 页（不含加载） | p50 6–20 ms，最大 34 ms | p50 10–37 ms |
| pdftoppm 进程（参考，含启动） | p50 38–54 ms | 48–97 ms |

## E5 定位

命令：`e5_locate.py --run-id e5`（[汇总](evidence/rtex-evaluation-2/e5/e5-summary.json)、[书的样本](evidence/rtex-evaluation-2/e5/e5-book10-samples.json)、[混排的样本](evidence/rtex-evaluation-2/e5/e5-mixed-samples.json)）。

rtex 给宿主的映射：`spans` 给出单元 id ↔ 源码字节范围；page display list 的每一行带布局 uid 与行号（`units` 命令把 uid 映射到 span），行内是带位置的字形（含字符码、`MATH` 行内公式边界）；`ParagraphUpdate` 用 `fragments` 把一个单元的行放到页上。没有逐字形的源码偏移。

基准：书（插入 48 个标记词，11 页）与混排文章（24 个，6 页）中每个标记词的 4 个字形，共 288 个样本；字形中心作为“点击点”，标记词首字节加偏移作为“光标字节”。

| 粒度 | 页面点击 → 源码 | 源码光标 → 页面 | 结果 |
|---|---|---|---|
| 单元（段落、标题、环境） | 288/288 命中所在 span；span 长度 p50 460 / 528 字节，最大 1583 字节 | 288/288 的真实字形落在该单元行框之内 | rtex 原生支持 |
| 行 | 行号由 display list 直接给出；但行 → 字节没有，按比例推算误差 p50 28 / 16 字节、p95 34 / 50 字节 | 按比例推算行：书 100 %、混排 80 % 落在正确行，p95 误差 16.4 pt | 页面 → 行原生；源码 → 行要推算或对齐 |
| 字形 | 宿主用 difflib 把单元的字形字符与源码对齐：288/288 精确到字节 | 同一对齐反查：288/288 精确到字形（0 pt） | 要宿主实现；正文字形 97.8 %（书）/ 99.1 %（混排）能精确对齐，TFM 数学字形 0 %（290 个，无 Unicode） |

- 可支撑的交互：跳到段落（双向）可以直接做；跳到行在页面 → 源码方向可以直接做，源码 → 页面要宿主对齐；字符级光标与选区在正文里靠宿主对齐可以做到，公式内部只能到“这个公式”（`MATH` 边界）或近似位置；连字（fi）与断词连字符处为近似（插值）。
- 标记词本身唯一，对齐在标记处最容易；全文覆盖率（上表）才反映一般正文的情况。

## E6 端到端时延（宿主侧）

命令：`e6_latency.py --run-id e6`（[汇总](evidence/rtex-evaluation-2/e6/e6-summary.json)）。还没有 LaTeX 窗口，所以只测宿主侧流水线：写 edit（击键）→ 读到 `ParagraphUpdate` → 交给常驻的 E4 原型把该单元的行栅格化（150 dpi，字体缓存已暖）。两种节奏各 120 次：稳定（每次更新后停 150 ms，后台 pass 多被去抖）与突发（10 次一组、组间停 1.5 s，后一组在后台 pass 进行中输入）。会话使用预热过的字体缓存副本。真实窗口内从击键到画面更新的测量（纹理上传、呈现）留到 L1。

| 夹具 | 节奏 | 击键 → ParagraphUpdate p50 / p95（最大） | → 单元栅格完成 p50 / p95（最大） | 其中栅格 p50 / p95 | rtex 自报 p50 / p95 | edit 回复 p50 / p95（最大） | 结果 |
|---|---|---|---|---|---|---|---|
| 10 页书 | 稳定 | 6.7 / 18.7 ms（32） | 12.5 / 35.2 ms（56） | 4.4 / 13.1 ms | 5.5 / 17.6 ms | 0.5 / 1.9 ms（4.9） | Pass |
| 10 页书 | 突发 | 8.8 / 26.4 ms（33） | 18.5 / 47.3 ms（57） | 6.0 / 16.9 ms | 7.7 / 25.1 ms | 0.7 / 0.9 ms（3.1） | Pass |
| 100 页书 | 稳定 | 10.6 / 30.9 ms（125） | 19.4 / 52.1 ms（133） | 6.7 / 19.2 ms | 8.5 / 29.3 ms | 0.7 / 1.0 ms（93） | Pass |
| 100 页书 | 突发 | 8.4 / 22.8 ms（110） | 16.7 / 41.8 ms（114） | 5.7 / 15.3 ms | 7.3 / 18.2 ms | 0.5 / 1.7 ms（96） | Pass |

- 240 次击键全部走实时路径、状态 ok；收敛布局：10 页 2.2 s、100 页 9.5 s（暖缓存）；会话内存峰值 0.62 / 1.04 GiB。
- 两次约 93–96 ms 的 edit 回复停顿都发生在 100 页布局装入的同一时刻（转录里紧挨着 `LayoutUpdate`），而那两条布局只带 0–1 页 display list，所以是 rtex 装入布局时持锁，不是管道里的大行。子进程接入时宿主的 UI 线程不受影响，只是那一次段落更新晚约 0.1 s。
- “栅格完成”含 Python 把事件转发给原型的序列化开销；产品里宿主直接用二进制 display list 时会更低。

## E7 非 LuaLaTeX 回退（整编预览）

命令：`e7_fallback.py --run-id e7`（[汇总](evidence/rtex-evaluation-2/e7/e7-summary.json)，每次编译的日志在 `e7-<引擎>-<页数>/`）。每次编译都是 `full` 配置里的一次沙箱调用；“冷”= 新的输出目录跑两遍（第一遍没有 aux，第二遍解决引用），“热”= 改动一个词后带着上次的 aux 跑一遍，三次取中位数。空沙箱启动（`kpsewhich --version`）中位 0.51 s，已含在下表里。

| 引擎与夹具 | 实际页数 | 冷：第 1 遍 / 第 2 遍 / 打开合计 | 热（一次预览刷新）中位 | 改动出现在 PDF | 结果 |
|---|---|---|---|---|---|
| XeLaTeX，ctex 中英混排 | 1 | 1.76 / 1.72 / 3.47 s | 1.78 s | 是 | Pass |
| XeLaTeX，ctex 中英混排 | 11 | 1.97 / 1.85 / 3.82 s | 1.91 s | 是 | Pass |
| XeLaTeX，ctex 中英混排 | 49 | 2.03 / 2.15 / 4.18 s | 2.18 s | 是 | Pass |
| pdfLaTeX，英文（CM） | 1 | 0.80 / 0.91 / 1.71 s | 0.91 s | 是 | Pass |
| pdfLaTeX，英文（CM） | 9 | 0.93 / 0.71 / 1.65 s | 0.67 s | 是 | Pass |
| pdfLaTeX，英文（CM） | 41 | 0.71 / 0.70 / 1.41 s | 0.72 s | 是 | Pass |

- `full` 配置不挂载 pdflatex。没有改配置：驱动把系统 pdfTeX 复制进可信的 `SCHOLIUM_SPIKE_TOOLS` 目录，用一个只设置 `TEXMFCNF`/`TEXMFROOT` 的包装脚本启动（kpathsea 按二进制所在目录推导配置位置）。产品若支持 pdfLaTeX，`full` 配置要显式加入 pdflatex。
- 热编译中有两次日志要求再跑一遍（标签移动）；预览只取一遍时，引用可能落后一遍。

**旧 revision 门（设计说明，未写产品代码）**：

1. 项目 session 维护单调递增的源码 revision；每次整编在开始前取快照并记下其 revision `r`，编译在快照副本上进行。
2. 单飞加合并：同一时刻至多一个整编在跑、一个在等；新编辑只更新“待编 revision”为最新值，不排队多份。按上表，一次刷新约 0.7–2.2 s，去抖 300–500 ms 即可。
3. 结果门：结果的 `r` 小于等于当前已显示的 revision 时丢弃；否则替换显示。显示的 `r` 小于当前源码 revision 时，预览标注“整编预览 · 第 r 版，源码已更新”；失败的编译不替换画面，只在诊断面板给出错误并标明它属于哪个 revision。
4. 引用未收敛（日志要求再跑）时自动再跑一遍同一快照，期间预览标注“引用待更新”。
5. 预览标识始终显示来源（“整编预览 · XeLaTeX”）以区别 LuaLaTeX 的实时路径。

## E8 故障

命令：`e8_faults.py --run-id e8`（畸形命令、rtex 被杀；[汇总](evidence/rtex-evaluation-2/e8/e8-summary-e8.json)），`--run-id e8-fix2 --only crash,hang`、`--run-id e8-fix3 --only preamble`（[汇总](evidence/rtex-evaluation-2/e8/e8-summary-e8-fix2.json)、[汇总](evidence/rtex-evaluation-2/e8/e8-summary-e8-fix3.json)）。全部在 `rtex` 沙箱内；时间线来自各会话转录。

| 故障 | 宿主观察到什么 | 编辑是否继续 | 恢复时间 | 结果 | 证据 |
|---|---|---|---|---|---|
| 实时 LuaLaTeX 被 `kill -9` | 23 ms 后 `EngineState Restarting`（“server closed the response channel”）→ `Starting` → 0.70 s `Ready`；**在途的那次编辑（路由 fast）既没有 `ParagraphUpdate` 也没有 `BackgroundScheduled`，之后 15 s 内也没有后台 pass**，画面停在旧内容直到下一次编辑或布局 | 是：edit 回复立即返回，引擎恢复后的下一次击键 0.12 s 内 ok | 引擎 0.7 s；丢失的更新要宿主补救 | **Fail**（静默丢失） | [转录](evidence/rtex-evaluation-2/e8/e8-crash.jsonl.gz) |
| 实时挂起（段内 `\directlua{while true do end}`） | 5.0 s 时 rtex 看门狗杀掉实时引擎（`Diagnostics`：“engine watchdog: no response within 5s”），0.3–0.5 s 后 `Ready`；挂起段落此后路由为 background，理由 `EngineFailed … stays on the full compile until the preamble changes` | 输入不阻塞；但挂起期间别的段落的编辑（0.5 s 时发出）**一直没有自己的更新**，5 s 内整个实时路径停摆 | 实时 5.3 s；挂起段落整场会话只走整编 | 部分通过 | [无看门狗](evidence/rtex-evaluation-2/e8/e8-hang-nowatchdog.jsonl.gz) |
| 后台挂起，无宿主超时（第一轮发现 3 复测） | 去掉死循环后 26.9 s 和 56.6 s 各来一条 `LayoutUpdate`：`compile: Ok`、`wall_ms` 29578 与 59318、没有诊断——两次都是被 RLIMIT_CPU 杀掉的 pass（同一个带死循环的快照被 rtex 重跑了一遍）；57.4 s 时才有收敛的正确布局 | 段落实时更新继续；布局 57 s 不更新 | 57.4 s | **Fail**（证实 #22：被杀 pass 报 `Ok`） | 同上 |
| 后台挂起，宿主每 pass 8 s 墙钟看门狗 | 看门狗在 13.4 s、21.5 s 各杀一次 pass（各活跃 8.1 s）；rtex 照样把它们报成 `Ok`（`wall_ms` 8004、16101）；14.3 s 时收敛 | 同上 | 14.3 s（= 2 × 8 s 看门狗 + 一次正常 pass） | 兜底有效，仍需宿主自己标注“超时” | [有看门狗](evidence/rtex-evaluation-2/e8/e8-hang-watchdog8.jsonl.gz) |
| 导言区：缺包 `\usepackage{scholiumnosuchpkg}` | `Diagnostics`（File not found、Emergency stop）+ 布局 `CompiledWithErrors`（页面保留上一版）；实时引擎每约 1.8 s 重启一次、每次 `Failed`，一直循环到修好 | 是：正文编辑路由 background，理由 `NoContext` | 修好后 1.1 s 实时 ok，1.4 s 布局 Ok | Pass（重启循环浪费 CPU） | [转录](evidence/rtex-evaluation-2/e8/e8-preamble-missing-package.jsonl.gz) |
| 导言区：未定义命令 `\scholiumundefinedmacro` | 引擎照常 `Ready`；布局 `CompiledWithErrors`，诊断“Undefined control sequence”第 5 行 | 是：正文编辑仍走实时路径（首次 0.14 s ok） | 修好后 1.3 s 实时 ok，1.5 s 布局 Ok | Pass | [转录](evidence/rtex-evaluation-2/e8/e8-preamble-undefined-command.jsonl.gz) |
| 畸形命令 | 未知文件、`../../etc/passwd`、坏 JSON、未知命令：都返回 `reply: error`，会话存活；`start > end`、越过文件尾、负数 start 被**静默截断**后照常执行（负数被当成 0，变成一次导言区编辑）；`set_document` 接受项目外路径：`../../e8-escape.tex` 写到了快照目录之外（沙箱里落在 `/work` 下），沙箱外的对照中绝对路径被原样写到主机该路径；**偏移落在多字节字符中间时 rtex panic（`start of range should be a character boundary`），`rtex serve` 以 101 退出** | panic 后会话丢失 | 需要重启会话 | **Fail**（宿主必须校验） | [汇总](evidence/rtex-evaluation-2/e8/e8-summary-e8.json)、[stderr](evidence/rtex-evaluation-2/e8/e8-malformed.stderr.log) |
| `rtex serve` 被 `kill -9` | 2 ms 内 stdout EOF，沙箱外层退出码 137；无残留进程（bwrap 的 PID 命名空间一起收走） | 否，需要宿主重启会话 | 重启到首次布局：tmpfs 13.4 s，暖缓存副本 1.6 s | Pass（有缓存时） | [转录](evidence/rtex-evaluation-2/e8/e8-killed.jsonl.gz) |

宿主侧单次 pass 墙钟超时原型（`faults.PassWatchdog`）：每 0.25 s 从主机 `/proc` 看会话的进程树，把“不是实时引擎（命令行不含 `rtex-serve`）且持续占用 CPU”的 LuaLaTeX 视为一次后台 pass，连续活跃超过限值就 `SIGKILL`；待命引擎等待正文时不占 CPU，不会被误计时。它把后台挂起的恢复从 57 s 缩到 14 s，但 rtex 会用同一个快照再跑一遍（2 × 限值），而且被杀的 pass 仍报 `Ok`，所以宿主要自己把“被看门狗杀掉的那次布局”标成失败并显示原因。第一轮发现 3（被杀 pass 报 `Ok`）得到证实。

## 资源限制设计（第一轮发现 4）

依据：

- 会话级 `MemoryMax=2G` 在冷字体缓存时被 luatexja 打满（E3，三次 OOM），暖缓存时峰值 0.8–1.0 GiB（100 页书 1.04 GiB）。
- 每进程 RLIMIT_CPU 30 s 作用于所有进程，包括长驻的 `rtex` 与实时引擎；本轮最长的会话（E6 100 页，约 2 分钟编辑）没有被它杀掉，但长时间编辑一定会累计到 30 s。后台挂起时它是唯一的兜底，代价是 2 × 30 s 的布局停摆（E8）。
- `timeout <秒>` 是整个会话的墙钟上限，到点杀掉整个 `rtex serve`，与“长驻会话”矛盾。
- rtex 的实时路径有 5 s 看门狗，后台 pass 没有（#22）；pass 被杀后报 `Ok` 并重跑同一快照。

建议（设计，未实现）：

| 层 | 限制 | 方式 |
|---|---|---|
| 会话（一个项目一个 `rtex serve` + 实时引擎 + 待命引擎 + pass） | 内存上限（暖缓存时 2 GiB 够用，冷缓存要 3 GiB 或先预热）、`MemorySwapMax=0`、`TasksMax=64`、`CPUWeight` 低于界面进程；**不设**墙钟上限，**不对长驻进程设** RLIMIT_CPU | 沿用 `systemd-run --user --scope`；会话的存活由宿主心跳（定期 `status`，超时即重启会话）判断 |
| 单次后台 pass / 导出 | 墙钟超时（如 `max(30 s, 5 × 上次成功 pass)`），超时即杀并把该布局标为失败、给出原因；CPU 用量记账 | 宿主看门狗（E8 原型）先行；上游 #22 落地后改用 rtex 自己的超时与失败上报；若要内核级强制，可给会话 scope `Delegate=yes`，由宿主把 pass 进程放进子 cgroup 设 `cpu.max`/`memory.max` |
| 一次性整编（E7 的 XeLaTeX/pdfLaTeX、导出） | 保持现有的每进程限制（墙钟 + RLIMIT_CPU + AS/FSIZE） | 现状即可，每次编译一个沙箱 |
| 字体缓存 | 按用户保存、可信预热、每会话复制一份可写挂载 | E3 的做法；预热本身是一次性整编，用一次性限制 |

## 发现与局限

1. **字体缓存是首次布局与内存的主因。** 每会话 tmpfs 时首次布局约 13 s、内存 1.4–1.5 GiB，luatexja 超过 2 GiB 被杀；暖缓存时 1.4–1.7 s、0.8–0.9 GiB，与沙箱外相当。luaotfload 要求缓存可写，只读挂载直接失败。
2. **E4 的 150 dpi 判据对中文页太敏感。** 参考栅格器之间的差异（1.27–1.66 %）已超过 1 % 预算；display list 本身与 hayro、pdftocairo 一致。建议判据改为“对至少两个独立参考 ≤ 1 %”或按 2 倍密度评估。
3. **TFM 字体要宿主自己解析映射文件。** display list 对经典 CM 数学字体只给 TFM 名；原型经 `pdftex.map` + Type 1 画出，首次加载 0.4–1.0 s（含 `kpsewhich` 子进程，产品应缓存路径）。
4. **定位原生只到单元与行。** 字形级要宿主做字形 ↔ 源码对齐；公式内部只有近似。
5. **JSON 行的 display list 很大。** 100 页的首个 `LayoutUpdate` 一行约 14 MB（每页约 136 KB）；rtex 在 100 页布局装入时会让 edit 回复停顿约 93 ms。产品接入宜用二进制 display list（C ABI 已提供），或请上游给 `rtex serve` 一个二进制/旁路通道。
6. **故障时的静默。** 实时引擎崩溃或挂起时，在途的编辑没有任何事件；被杀的后台 pass 报 `Ok`；挂起过的段落整场会话被降级。宿主要在 `EngineState Restarting` 后主动 `request_layout` 或重发受影响单元，并把看门狗杀掉的布局标为失败。
7. **rtex 信任宿主的偏移与路径。** 非字符边界偏移会 panic 杀掉会话；越界偏移被静默截断；`set_document` 的路径原样拼接（沙箱外可写任意位置）。宿主必须只发已校验的、项目内的相对路径与字符边界上的偏移。文档内容里的 `\input{../../…}` 没能触发同样的越界写（rtex 没有加载该文件）。
8. **缺包时实时引擎重启循环。** 导言区缺包期间约每 1.8 s 重启一次实时引擎，持续消耗 CPU。
9. **测量局限。** E6 不含真实窗口；E4 的计时在两次运行间相差约 1.5 倍（书 18 ms 与 30 ms，另一次运行同时有别的负载）；E5 的基准只在标记词处；E7 的冷编译仍复用系统格式文件与 fontconfig 缓存，不是“全新机器”的冷启动；所有数字来自一台 20 核机器。

## 门禁

- `python3 scripts/check-doc-links.py`：通过。
- `bash spikes/verify-stage0.sh security`（改过 `toolchain-sandbox.sh`）：见 [运行日志](evidence/rtex-evaluation-2/security.log)。
- 栅格化原型（独立清单）：`cargo fmt -- --check`、`cargo clippy --locked --all-targets -- -D warnings`、`cargo deny … check licenses bans sources advisories` 通过。
- 根 workspace 没有改动（`cargo metadata` 成员与改动前一致），根目录的 Rust 门禁与真实窗口检查不受影响，未运行。

## 选型建议

建议：**采用 rtex 作为 `live` 适配层背后的子进程（`rtex serve` 或同一会话的 C ABI 辅助进程），并以宿主侧兜底为前提。** 不建议进程内链接，也不建议在没有下列条件时接入。

条件：

1. 只在 OS 沙箱内运行（E2 前提不变）。
2. 字体缓存：按用户保存、可信预热、每会话复制一份可写挂载；否则首次布局约 13 s，中文项目可能被内存上限杀掉。
3. 宿主侧单次 pass 墙钟超时与“被杀即失败”的标注，直到上游 #22 修复；会话心跳与崩溃后自动重启。
4. 宿主在转发前校验编辑：偏移在 UTF-8 字符边界且不越界，路径是项目内的规范化相对路径；在 `EngineState Restarting/Failed` 后补发布局请求。
5. 资源限制按“会话级 cgroup + 单次 pass 超时、长驻进程不设 CPU 秒上限”重做（上节）。
6. 绘制：display list 栅格（含 `pdftex.map` + Type 1）+ hayro 作为 PDF 回退，按设备像素密度（≥ 2 倍）绘制；非精确页与 TikZ 页走回退。
7. 传输：评估二进制 display list（C ABI）或请上游提供 `rtex serve` 的二进制通道，避免 14 MB 的 JSON 行；E1 的 lopdf 安全公告（#24）与 bans 问题跟踪上游。
8. 定位：段落级直接用；字形级由宿主对齐实现，公式内部按“整个公式”处理。

不满足 2–4 时，中文 LaTeX 项目与长文档的实时预览不可靠，应退回纯整编预览（E7 表明 XeLaTeX/pdfLaTeX 整编预览在 1–50 页时为 0.7–2.2 s 一次刷新）。

## 待办

- 协调者据 E1–E8 另立选型 ADR。
- 可反馈上游（未提交）：在途编辑在引擎崩溃/挂起后无事件；被杀 pass 重跑同一快照（补充 #22）；非字符边界偏移 panic；越界偏移静默截断；`set_document` 接受项目外路径；缺包时实时引擎重启循环；`rtex serve` 的二进制 display list 通道；布局装入时 edit 回复停顿。
- L1：真实窗口内的 E6（纹理上传与呈现）；产品路径上复测 E2 负对照与本轮的字体缓存做法。
- 用至少两个参考栅格器或 2 倍密度修订 E4 判据的措辞（规划文档由协调者决定）。
