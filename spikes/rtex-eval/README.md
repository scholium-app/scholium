# rtex 评估驱动（L0：E1–E8）

对应 [LaTeX 工作区规划](../../docs/plan/LATEX_WORKSPACE.md) 的 L0 评估项 E1–E8。结果与判定写在开发记录
[第一轮（E1、E2、E3 准备）](../../docs/log/2026-10-30-rtex-evaluation.md) 与
[第二轮（E3–E8）](../../docs/log/2026-10-30-rtex-evaluation-2.md)，原始证据在
`docs/log/evidence/rtex-evaluation/` 与 `docs/log/evidence/rtex-evaluation-2/`。这里只有评估用的驱动、夹具与
栅格化原型，不是产品代码，不进入根 workspace（`raster/` 有自己的 `[workspace]`）。

## 前提

- 上游 rtex 视为不受信下载代码：只在仓库外克隆、构建，绝不复制进仓库，也不运行它的 `scripts/install-texlive.sh`。
- 系统 TeX Live（Arch，`/usr/bin/lualatex`）；`bwrap`、`systemd-run --user`、`pdftotext`/`pdfinfo`、`cc`、Python 3（只用标准库）。
- E3 夹具需要 `texlive-langchinese`（ctex、Fandol 字体）与 `texlive-langjapanese`（luatexja）；E4 的 TikZ 夹具需要 pgf。
- 第二轮另需 `pdftoppm`/`pdftocairo`（poppler，只作参考栅格器）、ImageMagick `magick`（PNG→PGM 与证据缩略图）、
  `xelatex`/`pdftex`（E7），以及先构建栅格化原型：`cargo build --release --locked --manifest-path spikes/rtex-eval/raster/Cargo.toml`。

```bash
mkdir -p ~/.cache/scholium-eval
git clone --depth 1 --branch v0.0.5 https://github.com/HenryXiaoYang/realtime-tex ~/.cache/scholium-eval/rtex-v0.0.5
git -C ~/.cache/scholium-eval/rtex-v0.0.5 rev-parse HEAD   # e0ad11765ec016846b6e07619081f0528621636b
(cd ~/.cache/scholium-eval/rtex-v0.0.5 && cargo build --release --locked)
```

## 复现（在仓库根目录执行）

每个驱动在 `spikes/rtex-eval/.work/<run-id>/` 下建工作目录（已被 `.gitignore` 忽略，目录已存在时拒绝运行），
证据写到 `--evidence` 指定的目录。从干净状态复现时先删除 `spikes/rtex-eval/.work/`。

```bash
CLONE=~/.cache/scholium-eval/rtex-v0.0.5
EV=docs/log/evidence/rtex-evaluation
# 冒烟（沙箱外）：doctor、gen-book --pages 10、一次 edit、verify；生成正对照用的 10 页夹具
python3 spikes/rtex-eval/smoke.py --clone $CLONE --evidence $EV
# 正对照：10 页书与手写文章，沙箱外与沙箱内各开一个长驻 rtex serve，编辑一段后导出 PDF 并比较
python3 spikes/rtex-eval/positive.py --clone $CLONE --evidence $EV --run-id positive-final
# 负对照 (a)–(f)：每项先在沙箱外跑（攻击必须生效），再在沙箱内跑；约 7 分钟
python3 spikes/rtex-eval/negative.py --clone $CLONE --evidence $EV/negative --run-id negative
# E3 准备：luatexja/ctex 是否可用、两份中文夹具能否编译、一次沙箱外的 rtex verify
python3 spikes/rtex-eval/cjk_check.py --clone $CLONE --evidence $EV/e3
# E1：用本仓库的 deny.toml 检查 rtex 的锁定依赖（advisories 单独运行，结果不过滤）
cargo deny --manifest-path $CLONE/Cargo.toml --config "$PWD/deny.toml" check --show-stats licenses bans sources
cargo deny --manifest-path $CLONE/Cargo.toml --config "$PWD/deny.toml" check --show-stats advisories
```

第二轮（E3–E8，全部在 `rtex` 沙箱配置内；E7 用 `full` 配置）：

```bash
EV2=docs/log/evidence/rtex-evaluation-2
cargo build --release --locked --manifest-path spikes/rtex-eval/raster/Cargo.toml
cargo deny --manifest-path spikes/rtex-eval/raster/Cargo.toml --config "$PWD/deny.toml" check licenses bans sources advisories
# E3：三份中文夹具的实时单元占比、120 次击键 p50/p95、沙箱内 rtex verify、字体缓存矩阵（约 12 分钟）
python3 spikes/rtex-eval/e3_cjk.py --evidence $EV2/e3 --run-id e3
python3 spikes/rtex-eval/e3_cjk.py --evidence $EV2/e3 --run-id e3-followup --followup        # luatexja 复测与缓存矩阵
python3 spikes/rtex-eval/e3_cjk.py --evidence $EV2/e3 --run-id e3-warmed --warmed-luatexja   # 先预热缓存再跑 luatexja
# E4：display list 栅格 vs poppler（splash 与 cairo）与 hayro，150/300 dpi；TikZ 页走 hayro 回退
python3 spikes/rtex-eval/e4_draw.py --evidence $EV2/e4 --run-id e4
# E5：标记词为基准的 页面点击→源码字节 与 源码光标→页面位置
python3 spikes/rtex-eval/e5_locate.py --evidence $EV2/e5 --run-id e5
# E6：10/100 页书，击键 → ParagraphUpdate → 原型栅格化该单元（稳定输入与突发输入两种节奏）
python3 spikes/rtex-eval/e6_latency.py --evidence $EV2/e6 --run-id e6
# E7：XeLaTeX（ctex）与 pdfLaTeX 夹具 1/10/50 页在 full 配置内的冷/热整编时间
python3 spikes/rtex-eval/e7_fallback.py --evidence $EV2/e7 --run-id e7
# E8：实时引擎被杀、实时/后台挂起（有无宿主看门狗）、导言区错误、畸形命令、rtex serve 被杀
python3 spikes/rtex-eval/e8_faults.py --evidence $EV2/e8 --run-id e8
```

`spikes/toolchain-sandbox.sh` 的 `rtex` 配置新增仅供可信驱动使用的 `SCHOLIUM_RTEX_FONT_CACHE=<目录>`：把该目录可写挂到
`/rtex/texmf-var` 并设为 `TEXMFVAR`（luaotfload 字体缓存跨会话保留）；不设置时行为不变（每会话 tmpfs）。E4–E6 与 E8 的会话
先用 `measure.warm_font_cache` 在沙箱里跑一次普通 LuaLaTeX 预热该目录，再开会话。

## 文件

| 文件 | 作用 |
|---|---|
| `session.py` | `rtex serve` 的 JSON 行驱动；沙箱模式经 `spikes/toolchain-sandbox.sh` 的 `rtex` 配置启动，完整 stdin/stdout 记为 gzip JSONL |
| `smoke.py`、`positive.py` | 冒烟与正对照 |
| `negative.py` | 负对照矩阵与判定：沙箱外必须泄漏或产生副作用，沙箱内必须没有 |
| `canary.py` | 在 `$HOME` 下放置随机金丝雀文件、主机环境变量金丝雀与本机回环监听器；结束时删除；扫描 PDF 文本、构建目录全部文件、stderr 与 JSON 事件四个通道 |
| `runaway.py` | 负对照 (f)：死循环单元被切断后编辑继续 |
| `common.py` | 夹具拼装、等待、残留进程检查 |
| `lift/lualatex.c` | 仅评估用的 LuaLaTeX 启动器：附加 `--shell-escape`，模拟 TeX 自身限制失效的主机，使 (c)(e) 打到沙箱而不是 LuaTeX；由 `negative.py` 编译到 `.work/` |
| `cjk_check.py` | E3 准备检查 |
| `fixtures/article.tex` | 正对照文章（英文、公式、`\ref`、图） |
| `fixtures/negative/` | 负对照共用导言区与每项一个正文；`@CANARY_DIR@`、`@PROJECT_DIR@`、`@PORT@` 由驱动填入 |
| `fixtures/cjk/` | E3 夹具：ctex（LuaLaTeX）与纯 luatexja |
| `gen_fixtures.py` | 第二轮的确定性原创夹具：中英混排 ctex 文章（E3/E5/E7）、英文 pdfLaTeX 文章（E7）、TikZ 文章（E4）；每段一个唯一标记词 `QZxx` 供 E5 作基准 |
| `measure.py` | 分位数、脚本化击键（插入/删除交替）、栅格化工作进程、单次沙箱命令、字体缓存预热 |
| `e3_cjk.py` … `e8_faults.py` | 第二轮各评估项驱动，见上方命令 |
| `faults.py` | E8：从主机 `/proc` 找实时引擎、后台 pass 与 rtex 进程；宿主侧单次 pass 墙钟看门狗原型；带时间戳的事件收集 |
| `raster/` | E4/E6 原型（独立 Cargo 清单）：skrifa 取字形轮廓（OpenType/CFF，TFM 字体经 `pdftex.map` 找 Type 1），tiny-skia 填充；hayro 渲染 PDF 回退；与上游 `rtex verify --raster` 同口径的墨迹比较（1 像素容差）|

`lift/lualatex.c` 只通过可信驱动设置的 `SCHOLIUM_RTEX_EVAL_TEXBIN` 进入沙箱，文档内容无法选择它；产品代码不得设置该变量。
E7 的 pdfLaTeX 同理：`full` 配置不挂载 pdflatex，驱动把系统 pdfTeX 复制进可信的 `SCHOLIUM_SPIKE_TOOLS` 目录，并用一个只设置
`TEXMFCNF`/`TEXMFROOT` 的包装脚本启动（kpathsea 按二进制所在目录找配置）。
