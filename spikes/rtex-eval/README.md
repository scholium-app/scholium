# rtex 评估驱动（L0：E1 / E2 / E3 准备）

对应 [LaTeX 工作区规划](../../docs/plan/LATEX_WORKSPACE.md) 的 L0 评估项 E1–E3。结果与判定写在开发记录
[2026-10-30 rtex 评估](../../docs/log/2026-10-30-rtex-evaluation.md)，原始证据在
`docs/log/evidence/rtex-evaluation/`。这里只有评估用的驱动与夹具，不是产品代码，不进入根 workspace。

## 前提

- 上游 rtex 视为不受信下载代码：只在仓库外克隆、构建，绝不复制进仓库，也不运行它的 `scripts/install-texlive.sh`。
- 系统 TeX Live（Arch，`/usr/bin/lualatex`）；`bwrap`、`systemd-run --user`、`pdftotext`/`pdfinfo`、`cc`、Python 3（只用标准库）。
- E3 夹具需要 `texlive-langchinese`（ctex、Fandol 字体）与 `texlive-langjapanese`（luatexja）。

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

`lift/lualatex.c` 只通过可信驱动设置的 `SCHOLIUM_RTEX_EVAL_TEXBIN` 进入沙箱，文档内容无法选择它；产品代码不得设置该变量。
