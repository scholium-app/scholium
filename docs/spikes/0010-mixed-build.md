# Spike 0010：阶段 0 第 7 项 — 混合构建

> **有效性（2026-09-17 登记）**：阶段 0 第 7 项的原始证据，结论限于已测夹具与宿主范围；契约见 [ADR 0001](../adr/0001-mixed-source-team-editing.md)，后续隔离进展见 [报告 0017](0017-mixed-build-isolation.md)。阶段 0 当前判定见 [报告 0012](0012-exit-criteria.md)。

> 历史记录：以下保留该轮验证结果。2026-09-16 工作区后续实现与复测见
> [报告 0016](0016-working-tree-status.md)，当前出口判定见[报告 0012](0012-exit-criteria.md)。

- 结论：**部分 Pass**（六类混用的成功路径与失败阻断在两种宿主下全部实测通过；但嵌入语义、超长文档、
  增量构建等范围明确未做，见[失败与不确定性](#失败与不确定性)）
- 对应验证项：[路线图阶段 0 第 7 项](../plan/ROADMAP.md)
- 日期：2026-09-16
- 执行者：ation_ciger
- 关联 ADR：[0001 团队单一源码语言与 LaTeX/Typst 混合项目](../adr/0001-mixed-source-team-editing.md)、[0002 原生技术栈与 UI 验证顺序](../adr/0002-native-ui-validation-order.md)
  （本项大量结论指向"嵌入载体与语义保真"需要新 ADR，见[对设计的影响](#对设计的影响)）
- 代码：[`spikes/mixed-build/`](../../spikes/mixed-build)
- 依赖版本：`typst = 0.15.1`、`typst-layout = 0.15.1`、`typst-kit = 0.15.1`（`embedded-fonts` + `scan-fonts`）、
  `typst-svg = 0.15.1`、`typst-pdf = 0.15.1`；TeX Live 2026 / XeTeX 3.141592653-2.6-0.999998；
  poppler 26.08.0（`pdftotext` / `pdfinfo` / `pdftohtml` / `pdfimages`）

## 问题与判据

### 要关闭的"待验证"

[混合源码与团队编辑](../MIXED_SOURCE_EDITING.md) 第 6/7 节声明了混合范围与构建管线，但
"两类源码能否在同一个真实项目里共同产出一份编号、页码、链接都正确的最终件"从未被实测。
本项要回答：**六类混用内容在两种宿主下是否都有可复现的可用路径，失败组合能否被准确阻断，
引用反馈是否必然终止。**

### 判据（动手前写定，不得事后调整）

术语：**宿主**（host）= 最终排版引擎与入口；**组件**（component）= 有独立权威源文件、
由某一引擎编译的混用单元；**plan** = `冻结快照 → 转换/依赖计划` 这一步产出的执行计划。

| 编号 | 判据 | 通过条件 |
|---|---|---|
| J1 表 | 正文 + 跨页表格在两种宿主下各有一个成功夹具 | 编译成功；表格内容经**独立文本抽取**证实出现在 ≥ 2 个物理页；表头在续页重复；表题编号与引用值一致 |
| J2 公式 | 编号公式在两种宿主下各有一个成功夹具 | 编译成功；公式编号从产物/内省读回且与引用值一致；行内公式与独立公式都在 |
| J3 图表 | 宿主原生绘图在两种宿主下各有一个成功夹具；**跨引擎嵌入**每种宿主各一个 | 编译成功；图题编号与引用一致；嵌入物为**矢量**（见 J8），并被宿主真实排版 |
| J4 宏 | 自定义宏在两种宿主下各有一个成功夹具 | 编译成功；宏展开后的文本/数值经独立抽取证实与定义一致；跨语言宏经**声明的桥接合约**（类型化参数）生效 |
| J5 作用域 | 模板/宏作用域在两种宿主下各有一个成功夹具 | 两个作用域定义同名符号且取值不同；两侧输出各自正确；**不泄漏**（互不出现对方取值） |
| J6 引用 | 双向引用 + 最终页码 + 链接在两种宿主下各有一个成功夹具 | 引用编号与 `.aux`/内省读回值一致；页码与"被引用内容真实所在物理页"一致（用逐页文本抽取独立验证）；交付件中链接触发点存在且目标可解析 |
| J7 双宿主 | 每种宿主都要既做宿主也做被嵌入方 | LaTeX 宿主 + Typst 宿主两个项目都产出最终件；跨引擎嵌入双向各至少一个夹具 |
| J8 源码级 | 不是截图嵌入 | 每个组件都有独立权威源文件；组件由**其自身工具链**真实编译；跨引擎载体为矢量（SVG 含路径数据 / PDF 无栅格图像），**不得**是整页位图；宿主是真编译而非图片拼接 |
| J9 自动核验 | 逐夹具断言 | 每个夹具单独打印断言与结果；不允许只报总数；任一夹具的任一断言失败即该夹具 Fail |
| J10 失败夹具 | 六类各有失败夹具 | 故意注入不支持语法/缺失宏/循环引用后，要么 plan 明确拒绝，要么宿主编译失败；**诊断可读**（指出符号/位置/原因）；**不得**静默产出错误结果；失败时不发布最终产物 |
| J11 轮数上限 | 引用振荡有上限 | 构造真实反馈振荡用例；检测到振荡后**停止**并报告；多轮计划有固定上限；收敛用例在达到上限前正常收敛；引擎自身的收敛上限（Typst 5 次内省）被当作硬失败 |

判据在写任何实现代码之前先写定到本文件（首次写入时"结果"以下全部为空，之后未再改动）。

### 明确不在本项范围

- 复杂浮动体（多栏浮动、浮动体跨节重排）、超长文档（>100 页）的规模与性能；
- 增量构建 / 缓存失效；
- 目录、文献（bib）、脚注的跨引擎装配；
- 生成视图与源码视图的编辑往返（属第 3 项）；
- 可访问性标注与 HTML/Markdown 目标。

## 环境

- 平台：Arch Linux，x86_64-unknown-linux-gnu；Rust nightly（edition 2024）。
- 锁定版本：`typst`/`typst-layout`/`typst-kit`/`typst-svg`/`typst-pdf` 均为 `=0.15.1`；
  TeX Live 2026（XeTeX 3.141592653-2.6-0.999998，`xelatex`/`latexmk` 在 PATH）；
  poppler 26.08.0；字体：`Noto Serif`（西文）+ `Noto Sans CJK SC`（中文，经 `fontspec`+`xeCJK`）。
- 新增依赖均为 Apache-2.0（Typst 官方 crate），无 npm/Node.js/WebView，无 GPL/AGPL。
- 构建与运行命令（可直接复制执行）：

```bash
export CARGO_HOME=/home/ation_ciger/Projects/Mogan/scholium/spikes/native-ui/.cargo-home
cd /home/ation_ciger/Projects/Mogan/scholium

cargo build --release --offline --manifest-path spikes/mixed-build/Cargo.toml
cargo clippy --release --offline --all-targets --manifest-path spikes/mixed-build/Cargo.toml
cargo run  --release --offline --manifest-path spikes/mixed-build/Cargo.toml
# 只跑单个夹具（例如只看振荡）：
cargo run  --release --offline --manifest-path spikes/mixed-build/Cargo.toml -- OSC-1-feedback
```

三条命令实测：build 与 run 零警告，clippy `--all-targets` 零警告（`grep -c "^warning"` = 0）。

## 方法与夹具

### 结构

`spikes/mixed-build/`（4500 行左右，单文件均 < 600 行）：

| 文件 | 职责 |
|---|---|
| `ir.rs` | 语言无关 IR：块、语言无关数学树、组件/宏/桥接声明 |
| `plan.rs` | `转换/依赖计划`：能力矩阵校验，运行前拒绝不支持的组合 |
| `generate.rs` / `generate_typst.rs` | 同一份 IR 生成真实 `.tex` / `.typ`（含编号探针） |
| `world.rs` | 最小 Typst `World`：多文件 + 二进制资源（矢量 PDF 图片） |
| `typst_host.rs` | Typst 编译、逐页文本、链接、编号探针、SVG/PDF 导出 |
| `latex.rs` | `xelatex` 驱动、`.aux` 解析、逐页 `pdftotext`、`pdftohtml` 链接、45s 墙钟护栏 |
| `build.rs` / `host.rs` | 有界多轮构建编排 / 产物读回与矢量组件导出 |
| `verify.rs` | 逐夹具断言（引擎读回 + 独立证据） |
| `fixtures/` | 夹具集合（`mod` + `content` + `macros_scope` + `refs` 三个子模块） |

### 构建管线（对应第 7 节管线）

```text
冻结输入快照(IR) → 计划(plan.rs) → 外语组件构建(standalone 源文件 + 各自工具链)
  → 宿主排版(host_source) → 引用/布局桥接(RefTable 多轮回流) → 最终验证(verify.rs)
```

- **源码级重建**：宿主与组件都由 IR 生成真实源码；组件由自己的工具链编译
  （Typst 组件走 crate 内编译，LaTeX 组件走真实 `xelatex`）。
- **跨引擎载体**：Typst 组件导出 SVG（取证）+ PDF（嵌入）；LaTeX 组件直接产 PDF；
  Typst 宿主用 `image("x.pdf")` 嵌入（Typst 0.15 原生支持矢量 PDF 图片），
  LaTeX 宿主用 `\includegraphics{x.pdf}`。全程无位图。
- **引用回流**：每轮把解析出的（编号，页码）写回源码重新生成；解析表稳定即收敛；
  重复出现过的解析表视为振荡，立即停止。
- **独立核验**：每个被标记元素内部嵌入唯一标记串 `MK<符号>`（表头首格 / 图题 / 公式内），
  因此"标记串出现的物理页"就是该元素的物理页；用它比对引擎读回的页码，才是独立核验而非自洽。

### 夹具矩阵（19 个夹具 × 宿主 = 36 次运行）

| 夹具 | 类别 | 宿主 | 期望 | 机制 |
|---|---|---|---|---|
| `T1-longtable` | 1 表 | 双 | 成功 | 88 行 3 列跨页表，表头续页重复 |
| `T1-bad-cols` | 1 表 | 双 | 拒绝 | 行格数 ≠ 声明列数 |
| `E1-equation` | 2 公式 | 双 | 成功 | 行内 + 2 个编号公式 + 编号/页码引用 |
| `E1-unsupported` | 2 公式 | 双 | 拒绝 | 子集外数学命令 |
| `G1-plot-native` | 3 图表 | 双 | 成功 | pgfplots / Typst `curve` 原生绘图 |
| `G2-plot-foreign` | 3 图表 | 双 | 成功 | 跨引擎矢量嵌入（双向） |
| `G3-foreign-multipage` | 3 图表 | 双 | 拒绝 | 外语组件跨页 → 单页载体不支持 |
| `M1-macro` | 4 宏 | 双 | 成功 | 宿主自有宏 + 跨语言宏按合约重实现 |
| `M1-undeclared` | 4 宏 | 双 | 拒绝 | 未声明的宏 |
| `M2-recursive` | 4 宏 | LaTeX | 拒绝 | 递归宏 → 真实 TeX `capacity exceeded` |
| `S1-scope` | 5 作用域 | 双 | 成功 | 两作用域同名宏互不泄漏 |
| `S1-scope-control` | 5 作用域 | 双 | 成功（**期望泄漏**） | 对照实现：关闭隔离后静默取到别家值 |
| `S1-leak` | 5 作用域 | 双 | 拒绝 | 跨作用域用宏且无合约 |
| `S1-conflict` | 5 作用域 | 双 | 拒绝 | 同作用域同名宏重复定义 |
| `R1-refs` | 6 引用 | 双 | 成功 | 宿主↔外语组件双向引用、页码、链接 |
| `R1-dangling` | 6 引用 | 双 | 拒绝 | 悬空引用（防止产物留 `??`） |
| `R1-raw-foreign` | 6 引用 | 双 | 拒绝 | 宿主内联外语原文而未提升为组件 |
| `OSC-1-feedback` | 振荡 | 双 | 拒绝 | 页码反馈改变版面高度 → 无不动点 |
| `OSC-2-engine` | 振荡 | Typst | 拒绝 | 引擎 5 次内省不收敛 |

## 结果

判据逐条对应（"证据"为 `cargo run --release` 的真实输出行，可复现）：

| 判据 | 结果 | 证据 |
|---|---|---|
| J1 表 | **Pass** | 两宿主各 3 页；`含填充行的页数 3 / 总页数 3`；`含表头的页数 3`；`表格页码：读回值 = 标记实际所在页：标记 MKtablong 在 Some(1)，读回 Some(1)`；`表格编号读回：编号 "1"` |
| J2 公式 | **Pass** | LaTeX `eq:int=2/1, eq:mass=1/1`、Typst 同值；`eq:mass/eq:int 页码：读回值 = 标记实际所在页`；`eq:int 的页码引用与真实页一致：查找 第1页` |
| J3 图表 | **Pass** | 原生绘图两宿主各 9 条断言全过；跨引擎嵌入 `组件 g2-draw Typst 编译成功，矢量产物 g2-draw.pdf（10443 字节 PDF，24514 字节 SVG，含 17 条路径）`；`fig:foreign 编号读回：编号 "1"` |
| J4 宏 | **Pass** | `跨语言宏在宿主侧按合约生效：宿主自有宏：42；跨语言宏：42`；`外语组件侧独立编译也得到 42（语义一致）`；`宏名未被原样输出（确实展开）` |
| J5 作用域 | **Pass** | `两个作用域取值各自出现一次（无泄漏）：取值-A 1 次，取值-B 1 次`；对照实现 `对照实现确实泄漏（s-b 取到了 s-a 的值）：取值-A 出现 2 次，取值-B 出现 0 次` |
| J6 引用 | **Pass** | `跨引擎符号编号：宿主引用值 = 组件自身引擎分配的编号：组件自身编号 (1)`；`外语组件内部引用宿主编号（反向引用）`；`存在指向宿主公式页的链接：目标页 Some(1)`；`链接目标均可解析到交付件内的页：无法解析的链接 0 条` |
| J7 双宿主 | **Pass** | 汇总 `覆盖：LaTeX 宿主 18 次、Typst 宿主 18 次`；跨引擎嵌入两个方向各一个成功夹具 |
| J8 源码级 | **Pass** | 每个夹具目录下同时有 `main.tex`/`main.typ` 与组件源文件；`组件独立编译产物是矢量：pdfimages 列出 0 个栅格图像`；`Typst 组件 SVG 含路径数据（不是位图）`；`最终件为矢量（无栅格图像）` |
| J9 自动核验 | **Pass** | 输出末尾的逐夹具表；每个夹具单独判定，逐条打印 `[PASS]/[FAIL] 断言名：观测值` |
| J10 失败夹具 | **Pass** | 11 个失败夹具全部命中预期诊断码（见下表） |
| J11 轮数上限 | **Pass** | OSC-1：`第 1 轮 用[(空)]/观测[pg:probe=1/2] ｜ 第 2 轮 用[1/2]/观测[1/1] ｜ 第 3 轮 用[1/1]/观测[1/2]` → `[reference-oscillation] … 已在第 3 轮停止（上限 4 轮），不发布正式产物`；OSC-2：`[engine-non-convergence] Typst 引擎在 5 次内省后仍未收敛：document did not converge within five attempts；value of the page counter did not converge` |

### 逐夹具结果（不做总数汇总）

```text
  T1-longtable         latex  success         11 条断言  Pass
  T1-longtable         typst  success         11 条断言  Pass
  T1-bad-cols          latex  plan-rejected    4 条断言  Pass
  T1-bad-cols          typst  plan-rejected    4 条断言  Pass
  E1-equation          latex  success         11 条断言  Pass
  E1-equation          typst  success         11 条断言  Pass
  E1-unsupported       latex  plan-rejected    4 条断言  Pass
  E1-unsupported       typst  plan-rejected    4 条断言  Pass
  G1-plot-native       latex  success          9 条断言  Pass
  G1-plot-native       typst  success          9 条断言  Pass
  G2-plot-foreign      latex  success         13 条断言  Pass
  G2-plot-foreign      typst  success         13 条断言  Pass
  G3-foreign-multipage latex  host-failed      8 条断言  Pass
  G3-foreign-multipage typst  host-failed      8 条断言  Pass
  M1-macro             latex  success         10 条断言  Pass
  M1-macro             typst  success         10 条断言  Pass
  M1-undeclared        latex  plan-rejected    4 条断言  Pass
  M1-undeclared        typst  plan-rejected    4 条断言  Pass
  M2-recursive         latex  host-failed      8 条断言  Pass
  S1-scope             latex  success          6 条断言  Pass
  S1-scope             typst  success          6 条断言  Pass
  S1-scope-control     latex  success          6 条断言  Pass
  S1-scope-control     typst  success          6 条断言  Pass
  S1-leak              latex  plan-rejected    4 条断言  Pass
  S1-leak              typst  plan-rejected    4 条断言  Pass
  S1-conflict          latex  plan-rejected    4 条断言  Pass
  S1-conflict          typst  plan-rejected    4 条断言  Pass
  R1-refs              latex  success         15 条断言  Pass
  R1-refs              typst  success         15 条断言  Pass
  R1-dangling          latex  plan-rejected    4 条断言  Pass
  R1-dangling          typst  plan-rejected    4 条断言  Pass
  R1-raw-foreign       latex  plan-rejected    4 条断言  Pass
  R1-raw-foreign       typst  plan-rejected    4 条断言  Pass
  OSC-1-feedback       latex  not-converged    8 条断言  Pass
  OSC-1-feedback       typst  not-converged    8 条断言  Pass
  OSC-2-engine         typst  not-converged    8 条断言  Pass

汇总：36 / 36 个夹具运行通过
```

注意：**36/36 只说明这 19 个夹具在两种宿主下都通过，不代表混合范围已交付**——见下方分类。

### 失败夹具的诊断（可读性证据）

| 夹具 | 诊断码 | 实测诊断 |
|---|---|---|
| `T1-bad-cols` | `table-columns` | `tab:bad 第 9 行有 1 格，声明列数为 3` |
| `E1-unsupported` | `math-call-unsupported` | `数学命令 notacommand 不在受支持命令表内` |
| `G3-foreign-multipage` | `component-multipage-unsupported` | `组件 g2-draw 编译出 2 页；当前受支持载体只有单页矢量嵌入` |
| `M1-undeclared` | `macro-undeclared` | `host 使用了未声明的宏 mystery` |
| `M2-recursive` | `host-compile-failed` | `./main.tex:16: TeX capacity exceeded, sorry [input stack size=10000]` |
| `S1-leak` | `scope-leak` | `组件作用域 s-b 使用了作用域 s-a 的宏 mark；作用域之间不得互相泄漏` |
| `S1-conflict` | `scope-conflict` | `作用域 s-a 内宏 mark 被 s-a 与 s-a 重复定义，最后一个会静默覆盖` |
| `R1-dangling` | `ref-dangling` | `host 引用了未定义的符号 eq:missing` |
| `R1-raw-foreign` | `raw-foreign-unrouted` | `host 是 latex 文件，却直接内联了 typst 原文：$ x = 1 $` |
| `OSC-1-feedback` | `reference-oscillation` | 见 J11 |
| `OSC-2-engine` | `engine-non-convergence` | 见 J11 |

全部失败夹具都验证了 `未发布最终产物：final.pdf 不存在`。

### 混淆矩阵：源码级支持 / 能编译但语义不完整 / 没做

**A. 源码级支持（语义完整，实测通过）**

| 内容 | 路径 | 证据 |
|---|---|---|
| 正文、标题、跨页表格 | 生成宿主原生 `longtable` / `table(repeat-header)` | 3 页、表头重复、页码实测一致 |
| 行内/独立公式与编号 | 生成宿主原生数学 | 两宿主编号均为 1/2，引用与页码一致 |
| 宿主原生绘图 | `pgfplots` / Typst `curve` | 栅格图像 0 个 |
| 自定义宏 | 宿主原生宏 + **声明式**跨语言合约（宿主侧重实现，双端语义一致断言 42） | `宿主自有宏：42；跨语言宏：42` |
| 模板/宏作用域 | 真实 `\begingroup…\endgroup` / `#{ … }` 作用域隔离 | 无泄漏；对照实现确实泄漏 |
| 双向引用、最终页码 | 共享引用表 + 有界多轮回流 | 组件编号来自组件自身引擎；页码取宿主装配位置 |
| 交付件内链接 | 宿主原生链接（`hyperref` / Typst `link`） | `无法解析的链接 0 条` |

**B. 能编译但语义不完整（必须如实标注，不能当成已交付）**

| 内容 | 现状 | 缺失的语义 |
|---|---|---|
| 外语矢量组件嵌入 | Typst 组件 → SVG+PDF；LaTeX 组件 → PDF；宿主按块级图片排版 | 组件内容**冻结**：宿主无法解析组件内部标签；组件内部超链接在嵌入时丢弃；宿主不重排组件内部；文本可选性/可访问文本取决于 PDF 载体而非宿主 |
| 跨引擎引用 | 编号/页码经引用表桥接并冻结为文本 | 链接粒度是**组件位置**，不是组件内部的精确目标；外语组件内引用宿主是文本替换，非活链接 |
| 外语组件跨页 | 明确阻断（`component-multipage-unsupported`） | 尚无跨页装配（`\includepdf` 式或多页载体） |

**C. 没做（本 spike 未覆盖）**

复杂浮动体（多栏浮动、跨节重排）、超长文档（>100 页规模与性能）、增量构建/缓存失效、
目录/文献/脚注的跨引擎装配、行内矢量嵌入（只支持块级）、多页外语组件、组件内部链接保真、
跨语言宏的自动推导（只支持声明式合约）、HTML/Markdown 目标与可访问性、
"标准目标包 / 可重建混合包"的干净环境构建验证、CJK 数学字体的细节调优。

### 产物大小

`spikes/mixed-build/out/`：**1.9 MB**，124 个文件（18 `.tex`、18 `.typ`、42 `.pdf`、4 `.svg`，
其余为 `.aux`/`.log`）；单个夹具目录最大 **176 KB**。`out/` 由 `spikes/mixed-build/.gitignore`
覆盖（`git check-ignore -v` 实测命中），`target/` 由根 `.gitignore` 的 `/spikes/**/target/` 覆盖，
二进制产物均不入库。完整运行一次全部 36 次夹具运行约 **40 秒**（其中大部分是多次 `xelatex` 与 `pgfplots`；已完成 release 编译）。

### 过程发现（对实现有直接价值，全部实测）

1. **Typst 的 `$ x $` 是块级公式**，行内必须写 `$x$`。用带空格的写法生成"行内公式"会让行内公式
   也占用公式计数器，两宿主编号从此错位（本 spike 第一版 `eq:mass` 读回 2 而不是 1）。
2. **Typst 的 `figure` 默认不可跨页**：跨页表格必须 `#show figure: set block(breakable: true)`；
   否则表格在第二页之后被静默截断（观察到 88 行只剩 48 行，且没有任何错误）。
   这是"静默产出错误结果"的典型例子，只有逐页文本断言才能发现。
3. **Typst 的 `#include` 不会把定义泄漏进包含者作用域**（与 LaTeX 的 `\input` 不同）。
   因此"天真实现会静默泄漏"的对照实验在 Typst 侧必须改成**文本内联**才成立；
   反过来这也说明 Typst 的包含语义本身更安全。
4. **Typst 的 `figure` kind 推断**：非表格、非图片的 body 会被推断成别的 kind，
   `counter(figure.where(kind: …))` 选择器必须显式写；自定义字符串 kind 必须给 `supplement`。
5. **LaTeX `\def\x{#1}{…}` 非法**，必须是 `\def\x#1{…}`（参数文本里不能带花括号）。
6. **xelatex 默认字体不含 CJK**：不加 `fontspec`+`xeCJK` 时中文全部缺字，
   `pdftotext` 抽出的是乱码，断言会以"文本找不到"的形式失败。
7. **递归宏的护栏**：`\def\a{\a}\a` 会让 TeX 无限循环而**不报容量错误**，
   必须靠墙钟上限（本 spike 45 s）兜住；`\def\a{\a\a}\a` 才会快速给出
   `TeX capacity exceeded`。正式实现两种都要有：编译器诊断 + 墙钟上限。
8. **裸引用的页码证据**：Typst `#ref(<x>, form: "page")` 默认带 supplement（渲染成"页 2"），
   需要 `supplement: none` 才与 LaTeX `\pageref` 形态一致。

## 失败与不确定性

- **没有失败项**：19 个夹具 × 2 宿主 = 36 次运行全部通过（逐夹具判定，见上表）。
  判据 J1–J11 全部满足。
- **结论定为"部分 Pass"的原因**：通过的是"六类混用各有一条可用路径 + 失败可阻断 + 引用有界"，
  而 [MIXED_SOURCE_EDITING](../MIXED_SOURCE_EDITING.md) 第 6 节要求的
  "复杂浮动体、目录/文献/脚注、跨页装配、可访问文本"等**没有覆盖**，不能据此宣称混合构建已交付。
- **嵌入语义是本项最大的不确定性**：矢量嵌入保住了版面，但丢掉了组件内部的可解析符号、
  超链接与重排能力。本 spike 证明了"能编译、编号页码正确、链接落到组件"，**没有**证明
  "组件内部语义在最终件中仍可解析"。
- **性能与规模未测**：只跑了单页到 3 页的夹具；没有超长文档、没有增量构建、没有并发构建。
- **只在单一平台/单一字体组合上验证**：Linux + Noto CJK + TeX Live 2026。字体缺失时
  会出现缺字（第 6 条发现），正式实现必须显式声明字体来源与回退。
- **"干净环境可构建"未验证**：本 spike 不产出"标准目标包 / 可重建混合包"，
  也没有在干净容器里复现依赖清单。这是阶段 0 出口条件的一部分，需另立验证项。
- **时间护栏是硬编码的**：45 s 的 `xelatex` 上限和 4 轮的引用轮数上限是固定值，
  没有做成可配置的 profile（第 7 节要求"固定一个可配置默认轮数并验证"）。

## 对设计的影响

1. **需要新 ADR：嵌入载体与语义保真度**。本项给出的事实是：矢量嵌入 = 版面保真 + 语义冻结。
   必须在 ADR 里明确"哪些内容允许走载体"（绘图、复杂公式、整块外语模板），
   "哪些内容禁止走载体"（需要被宿主引用/重排/可选中的正文与表格），
   以及载体选型（Typst 0.15 直接吃矢量 PDF，不必再引入 SVG 转换器，这一条可以省掉一个外部依赖）。
2. **引用桥接的粒度要写进合约**：跨引擎引用的链接只能落到组件，不能落到组件内部目标。
   若要更细粒度，需要"组件内锚点导出 + 宿主命名目标注入"，属于第 3 阶段。
3. **能力矩阵要前置到计划层**：本 spike 证明 plan 层拒绝（列数、悬空引用、作用域泄漏、
   未声明宏、外语原文内联、多页组件、子集外语法）成本极低且诊断可读，
   应在正式实现里作为"导出前门禁"，与第 7 节"阻止正式输出"对齐。
4. **多轮构建必须有"收敛判据 + 振荡判据 + 轮数上限"三件套**，缺一不可：
   只判收敛会在振荡时耗尽上限并报出无信息的错误；本 spike 的循环检测直接给出周期证据。
5. **Typst 的包含语义差异**要写进 `MIXED_SOURCE_EDITING.md` 的作用域小节：
   LaTeX `\input` 共享作用域（会泄漏），Typst `#include` 不共享；同一套"作用域隔离"策略
   在两种宿主下的实现不同。
6. **构建护栏**：编译超时（尤其模板/宏可导致无限循环）必须作为一等错误类型，
   与"编译失败"区分开，并在 UI 上给出不同措辞。

## 复现步骤

从干净环境开始（假设 Rust nightly、TeX Live 2026、poppler、`rsvg-convert` 可选）：

```bash
# 1. 依赖与构建
export CARGO_HOME=/home/ation_ciger/Projects/Mogan/scholium/spikes/native-ui/.cargo-home
cd /home/ation_ciger/Projects/Mogan/scholium
cargo build  --release --offline --manifest-path spikes/mixed-build/Cargo.toml   # 零警告
cargo clippy --release --offline --all-targets --manifest-path spikes/mixed-build/Cargo.toml  # 零警告

# 2. 跑全部夹具（约 40 秒），打印逐夹具断言与汇总
cargo run --release --offline --manifest-path spikes/mixed-build/Cargo.toml

# 3. 只看某一类
cargo run --release --offline --manifest-path spikes/mixed-build/Cargo.toml -- OSC-1-feedback
cargo run --release --offline --manifest-path spikes/mixed-build/Cargo.toml -- G2-plot-foreign

# 4. 检查产物（不入库）
ls spikes/mixed-build/out/                       # 每个夹具一个目录：main.tex/main.typ + 组件源 + final.pdf
du -sh spikes/mixed-build/out                     # 约 1.9 MB
pdftotext -f 1 -l 1 spikes/mixed-build/out/T1-longtable-latex/final.pdf -
```

代码清单（供复核）：`spikes/mixed-build/src/{ir,plan,generate,generate_typst,world,typst_host,latex,build,host,verify,diag,main}.rs`
与 `spikes/mixed-build/src/fixtures/{mod,content,macros_scope,refs}.rs`，单文件均 < 600 行。

## 验证门禁复核（2026-09-16）

见[报告 0015](0015-verification-review.md)：已修复失败断言仍返回退出码 0、未知夹具空跑通过、
`pdfimages` 执行失败被算作零栅格图像的问题，并删除重复的“构建诊断”断言。
上文历史断言数量包含重复项；当前版本以实际输出为准。这不扩大本报告的支持范围。
