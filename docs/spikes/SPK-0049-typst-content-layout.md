# Spike 0049：Typst 结构 Content 直接布局入口

- 结论：**Pass（仅公开结构输入布局入口）**；不构成编辑内核或增量分页验收。
- 对应验证项：[路线图阶段 0 第 1/2 项](../plan/ROADMAP.md)：结构编辑与 Typst 映射。
- 日期：2026-09-30
- 执行者：Codex（用户指定深改 Typst 路线）
- 关联提案：[ADR 0032](../adr/ADR-0032-typst-edit-kernel.md)
- 后续判据：[Typst 编辑内核计划](../plan/TYPST_EDIT_KERNEL.md)

## 问题与判据

是否能跳过源码生成/解析/eval，直接使用 Typst 已有排版算法处理原生结构输入？
动手前判据已写入 `spikes/typst-edit-session/README.md`：

1. 中英文与分数 Content 经公开 `layout_frame` 得到非空 Frame；`World::source` 调用为零。
2. 空分母与填入分母都成功布局，填入后 Frame 指纹改变。
3. 重复同一 Content 布局指纹相同，记录字形、线条与 detached span。
4. 对 40 次不同修改记录布局与 Frame 遍历耗时，仅作为性能观测，不设通过阈值。

这些判据不要求 caret、HitMap、完整页面、源图对照、GPU 或实时屏幕输入，不能扩大 Pass 范围。

## 环境

- Rust 1.96.0 (`ac68faa20 2026-05-25`)，release optimized。
- Typst / typst-layout / typst-kit 0.15.1，comemo 0.5.1；独立 Cargo.lock 固定传递依赖。
- Linux 7.2.7-arch1-1，Intel i7-13650HX，20 逻辑 CPU；无 GUI/GPU/IME 参与。
- 使用 typst-kit embedded + system 字体集合，默认 Library 样式；字体扫描与初次布局在采样外。
  系统字体不是跨机器受控资产，本数据不能推广到缺少相同字体的环境。
- 读取上游源码来自本机 registry 的固定 0.15.1 版本，没有修改 registry：
  - `typst-layout/src/flow/mod.rs` SHA256：
    `0bfbbf95bf5754970aa52da8e4742346d161fec4c4e2d02a28d7f07a2504d246`。
  - `typst-library/src/math/ir/item.rs` SHA256：
    `381d2e859d4e8ec753c7482ed3f0b354b7723ba42a19a35f6cbc53425e597ac3`。

## 方法与夹具

独立程序直接构造 `TextElem`、`FracElem(x, denominator)`、`EquationElem` 与 `ParElem`。
段落是“中文与 English，行内分数：” + 公式 + 后缀，宽 420 pt、最大高 2000 pt。
第一组分母为空字符串，第二组为 `2`；二者仍是正常 Typst Content，没有自定义 Hole。

World 的 source 方法每次递增计数并返回 NotFound：若布局要求读取源码，会观察到读计数
或失败。Engine 使用 EmptyIntrospector、默认 Library 样式与 Locator::root。
Frame 递归遍历 Text/Shape/Group，并检查 warnings/delayed errors 为空。

40 次采样各重新构造不同编号后缀的 Content，计时包括布局与 Frame 遍历，不包括 Content
构造、初次字体扫描、栅格化、IPC、UI 帧或显示器。它是单段后端探针，没有维护多段场景。

## 结果

原始输出：[content-layout.log](evidence/SPK-0049/content-layout.log)。

| 判据 | 结果 | 证据 |
|---|---|---|
| 直接结构布局，不读源码 | Pass | `source_reads=0`；World::source 不提供任何可读取正文 |
| 空/填入分母均成功 | Pass | 空：21 glyphs、1 shape；填入：22 glyphs、1 shape |
| 重复指纹一致，修改指纹变化 | Pass | 程序内断言；任一不满足会非零退出 |
| 暖态不同输入的耗时记录 | Pass（采样完成） | 40 样本，布局 + Frame 遍历 p50 **0.903 ms**、p95 **0.938 ms** |

两组 Frame 宽均为 420 pt，高均为 7.562 pt。每个字形的 span 都是 detached：
空分母 21/21，填入分母 22/22。没有分母的字形，不等于存在分母的可编辑槽位。

Frame 指纹只比较同一进程内的同内容与修改；没有把跨进程 hash 当图像 golden。
第一次探索运行 p50/p95 为 0.978/1.003 ms，同样不包含 UI/屏幕链路。
探针格式检查、release Clippy（`-D warnings`）、仓库 Rust 文件规模与 diff whitespace 检查通过。

## 失败与不确定性

- **没有编辑身份或空槽几何。** 直接构造的元素都无来源 span，输出的 glyph 无法直接映射
  到 Scholium 节点；空分母不会自动变成带 slot ID 的 HitMap。这正是第一批 fork 补丁的任务。
- 原生构造与对应 `.typ` 源码的像素等价尚未测；没有检查数学节点外框、合字内部 caret、
  多行、RTL 或原生窗口。重复 hash 只检查本夹具布局稳定，不代替视觉对照。
- 单段约 1 ms 不能声称 256 段全文编辑约 1 ms，不能声称已实现局部分页、增量 math IR
  或按键当帧可见。没有测资源上限、复杂 show/query/state 和会话长时内存。
- EmptyIntrospector 与 Locator::root 仅适用于当前无全局依赖的独立夹具；产品会话必须
  管理不碰撞的结构身份、Location 和完整 introspection 依赖，不能照抄本探针的全局上下文。
- 程序只布局固定受信 Content。直接布局可能执行 show/context，本测量不构成安全隔离验证。

## 对设计的影响

公开 `layout_frame` 已足以证明 Content 输入入口存在；不必先 fork 顶层 compile 才验证方向。
更深的数学入口与行布局在私有模块中，MathItem 带 arena/style 借用；要扩展持久会话、
Hole、origin 和几何，需要显式 fork 接口，而不能把借用 IR 直接保存为文档模型。

建议首先完成计划 K1 的身份与空槽补丁，再验证统一几何、Content 依赖和分页检查点。
ADR 0032 保持 Proposed，现有产品与阶段出口判定不变；没有默认开启新的编辑路径。

## 复现步骤

原始无 fork 版本保留在 `fb62d37`。后续报告 0050 为主探针加入隔离 fork，当前使用
独立 stock reference workspace 重跑相同公开入口夹具；历史耗时仍仅对应本报告原始运行。

```sh
cargo run --release --locked --manifest-path spikes/typst-edit-session/reference/Cargo.toml
cargo fmt --manifest-path spikes/typst-edit-session/reference/Cargo.toml -- --check
cargo clippy --release --locked --manifest-path spikes/typst-edit-session/reference/Cargo.toml --all-targets -- -D warnings
```

本次复用已有编译目录以减少构建时间：

```sh
CARGO_TARGET_DIR="$PWD/spikes/render-latency/target" cargo run --release --offline --locked --manifest-path spikes/typst-edit-session/reference/Cargo.toml
```

离线命令需要锁定依赖已经缓存；干净机器用第一条联网命令。
