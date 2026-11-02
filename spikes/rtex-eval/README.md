# rtex 评估驱动（L0：E1–E8）

对应 [LaTeX 工作区规划](../../docs/plan/LATEX_WORKSPACE.md) 的 L0 评估项 E1–E8。结果与判定写在开发记录
[第一轮（E1、E2、E3 准备）](../../docs/log/2026-10-30-rtex-evaluation.md) 与
[第二轮（E3–E8）](../../docs/log/2026-10-30-rtex-evaluation-2.md)，原始证据在
`docs/log/evidence/rtex-evaluation/` 与 `docs/log/evidence/rtex-evaluation-2/`。这里只有评估用的驱动、夹具与
栅格化原型，不是产品代码，不进入根 workspace（`raster/` 有自己的 `[workspace]`）。

当前评估 pin 跟随经评审的 upstream main commit（`a27561f`），包含已合并但尚未发布的 math owners / unicode-math 空根指数补丁；E1/E2/E8 与 E3/E5/E6 复验和 pin 变更记录见
[升级复验记录](../../docs/log/2026-11-02-rtex-v006-pin.md)。唯一版本来源是 [`PIN`](PIN)。

## 前提

- 上游 rtex 视为不受信下载代码：只在仓库外克隆、构建，绝不复制进仓库，也不运行它的 `scripts/install-texlive.sh`。
- 系统 TeX Live（Arch，`/usr/bin/lualatex`）；`bwrap`、`systemd-run --user`、`pdftotext`/`pdfinfo`、`cc`、Python 3（只用标准库）。
- E3 夹具需要 `texlive-langchinese`（ctex、Fandol 字体）与 `texlive-langjapanese`（luatexja）；E4 的 TikZ 夹具需要 pgf。
- 第二轮另需 `pdftoppm`/`pdftocairo`（poppler，只作参考栅格器）、ImageMagick `magick`（PNG→PGM 与证据缩略图）、
  `xelatex`/`pdftex`（E7），以及先构建栅格化原型：`cargo build --release --locked --manifest-path spikes/rtex-eval/raster/Cargo.toml`。

```bash
mkdir -p ~/.cache/scholium-eval
PIN_COMMIT=$(python3 -c 'import json; print(json.load(open("spikes/rtex-eval/PIN"))["commit"])')
PIN_REPOSITORY=$(python3 -c 'import json; print(json.load(open("spikes/rtex-eval/PIN"))["repository"])')
CLONE="$HOME/.cache/scholium-eval/rtex-$PIN_COMMIT"
git clone --no-checkout "$PIN_REPOSITORY" "$CLONE"
git -C "$CLONE" fetch origin "$PIN_COMMIT"
git -C "$CLONE" checkout --detach "$PIN_COMMIT"
(cd "$CLONE" && cargo +1.99.0 build --release --locked)
```

## 一条命令评审并更新 pin

```bash
# 无参数时解析本次 fetched upstream main head；也可传完整 commit SHA。
python3 scripts/rtex-pin.py a27561fdd6ec913f2a80d5b264f8fa7aaa228478
# python3 scripts/rtex-pin.py
```

脚本读取 PIN 的 repository，在仓库外按完整 commit 建 clone 并 locked release build，顺序执行 E1（许可/来源门、bans/advisories
未过滤报告）、E2 全负对照、配置上游 8 秒 pass_timeout 的完整 E8、后台 SIGKILL/timeout 对照，以及普通 CLI 的畸形输入矩阵。
每阶段显示 PASS/FAIL；仅所有证据通过时原子更新 PIN（repository + branch main + commit），任何失败保留旧文件并返回非零。
默认证据保存在外部 cache；可用 `--evidence <新目录>`、`--cache <外部目录>`、`--run-id <新标识>` 指定。
它不提交、不推送、不改 git config；新增 commit 必须是当前 upstream main 的祖先。E2 使用全局残留进程检查，运行期间不要另开 rtex 沙箱实验。

此命令的 E1/E2/E8 只是 pin bump 的故障门；提交前仍需人工复核 E3/E5/E6 回归、ADR/日志变更和完整仓库门禁。
`math_owners` 继续默认关闭；此评审不声称覆盖该可选功能的产品验收。

## 复现（在仓库根目录执行）

每个驱动在 `spikes/rtex-eval/.work/<run-id>/` 下建工作目录（已被 `.gitignore` 忽略，目录已存在时拒绝运行），
证据写到 `--evidence` 指定的目录。重跑使用新的 run-id；保留已有实验数据。

```bash
CLONE="$HOME/.cache/scholium-eval/rtex-$(python3 -c 'import json; print(json.load(open("spikes/rtex-eval/PIN"))["commit"])')"
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
# E8：实时引擎被杀、实时/后台挂起、导言区错误、畸形命令、rtex serve 被杀
python3 spikes/rtex-eval/e8_faults.py --evidence $EV2/e8 --run-id e8
```

当前 pin 的 CLI 没有暴露 `SessionConfig.pass_timeout`。E8 的配置超时复验用 [`pass-config.rs`](pass-config.rs)
作为仓库外评估进程目标，链接精确 commit 的 `rtex-core` 并通过 JSON 行协议驱动；它将 `pass_timeout` 设为 8000 ms，
不启动宿主单次编译看门狗。普通 CLI 覆盖 malformed、引擎恢复和整编行为，完整配置故障矩阵记录在升级日志中。

以下从仓库根目录执行，将精确 commit 的源码导出到新的仓库外目录，并只在那里追加一个评估 binary target；`Cargo.lock` 不变：

```bash
CONFIG_SRC="$HOME/.cache/scholium-eval/rtex-$PIN_COMMIT-pass-config-src"
mkdir "$CONFIG_SRC"
git -C "$CLONE" archive "$PIN_COMMIT" | tar -xf - -C "$CONFIG_SRC"
printf '\n[[bin]]\nname = "rtex-pass-config-eval"\npath = "%s/spikes/rtex-eval/pass-config.rs"\n' "$PWD" >> "$CONFIG_SRC/crates/rtex-cli/Cargo.toml"
CARGO_TARGET_DIR="$CONFIG_SRC/target" cargo build --release --locked --offline --manifest-path "$CONFIG_SRC/Cargo.toml" --bin rtex-pass-config-eval
EVAL_BIN="$CONFIG_SRC/target/release/rtex-pass-config-eval"
python3 spikes/rtex-eval/e8_faults.py --clone "$CLONE" --binary "$EVAL_BIN" --pass-timeout-ms 8000 --evidence "$EV2/e8-config" --run-id e8-config
python3 spikes/rtex-eval/v006_pass_faults.py --clone "$CLONE" --binary "$EVAL_BIN" --evidence "$EV2/e8-pass" --work "$HOME/.cache/scholium-eval/e8-pass-work"
```

`--offline` 需要先完成上面的锁定构建以备齐依赖；二进制只用于实验沙箱。CLI 直接运行 E8 时使用上游默认 300 秒，
不能把它当作显式 8 秒配置；真实宿主必须使用能配置该字段的上游接口或等待 CLI 暴露参数。

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
| `PIN` | 上游 repository、branch main 与经评审的精确 commit；所有默认 clone 路径由 commit 推导 |
| `pass-config.rs` | 仅评估用子进程，为缺少 CLI 参数的 `SessionConfig.pass_timeout` 设值；不进入产品依赖图 |
| `v006_contracts.py`、`test_v006_contracts.py` | E8 超时/杀进程/畸形输入结果的 fail-closed 判定测试（沿用 v0.0.6 时的文件名，main 复验也复用） |
| `raster/` | E4/E6 原型（独立 Cargo 清单）：skrifa 取字形轮廓（OpenType/CFF，TFM 字体经 `pdftex.map` 找 Type 1），tiny-skia 填充；hayro 渲染 PDF 回退；与上游 `rtex verify --raster` 同口径的墨迹比较（1 像素容差）|

`lift/lualatex.c` 只通过可信驱动设置的 `SCHOLIUM_RTEX_EVAL_TEXBIN` 进入沙箱，文档内容无法选择它；产品代码不得设置该变量。
E7 的 pdfLaTeX 同理：`full` 配置不挂载 pdflatex，驱动把系统 pdfTeX 复制进可信的 `SCHOLIUM_SPIKE_TOOLS` 目录，并用一个只设置
`TEXMFCNF`/`TEXMFROOT` 的包装脚本启动（kpathsea 按二进制所在目录找配置）。
