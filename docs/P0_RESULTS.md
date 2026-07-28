# P0 技术验证结果

日期 2026-07-28。环境：Arch Linux 7.1.5，Wayland + fcitx5，rustc 1.98-nightly，
typst 0.15.1，winit 0.30.13。

代码在 `spike/`，按计划属于可弃代码，保留下来只作为实测记录。

```bash
cd spike
cargo run --bin span       # 验证 #1
cargo run --release --bin latency   # 验证 #2
cargo run --bin ime        # 验证 #3a，需人工输入
```

## #1 span 粒度 — ✅ 通过

**问题**：编译产物里的每个字形，能否反查到源码中足够细的位置？
**标准**：分子、分母、上标、根号内容各自可区分。

实测 32 个字形，**全部有 span，零 detached**。

| 结构 | 字形归属 | 可区分 |
|---|---|---|
| `frac(p, q)` | `p`→[58..59]，`q`→[61..62] | ✅ 分子/分母 |
| `x^u + y_v` | `x`→[69..70]，`u`→[71..72] | ✅ 基号/上标 |
| `sqrt(w)` | `w`→[89..90]，√ 与 Shape→[84..91] | ✅ |
| `mat(1,2;3,4)` | 1/2/3/4 各自独立范围 | ✅ 每个单元格 |

结论：**不需要往生成的 Typst 源码里插标记节点**，PLAN §5.1 的退路不必启用。

### 附带解掉的未确认项

`Glyph.span` 的 `(Span, u16)` 中，**`u16` 是 span 内的字节偏移**，
精确位置 = `range.start + offset`。

验证：`"标题"` 的 span 是 [2..8]，两个字形 offset 分别为 0 和 3，
对应字节 2 和 5，正是「标」「题」的起始位置。

两类节点的行为不同：

- **文本节点**：span 覆盖整段文本运行（如 [31..49] 覆盖「与正文混排。」），
  靠 `offset` 细化到单字。
- **数学原子**：span 本身就精确，`offset` 恒为 0。

### 结构性字形的归属

分数线（`Shape`）、根号符号、矩阵定界符都映射到**整个函数调用范围**而非局部。
这是期望行为——点分数线就该选中整个 `frac` 节点。

### 关于 `Span::detached()`

`typst-layout/src/math/shaping.rs` 的 183 和 234 行把 span 填成 `Span::detached()`，
读源码时看起来像是"数学字形没有 span"的风险。实测该路径**一次都没走到**，
应该是缺字形（tofu）的兜底分支。风险不存在。

## #2 编译延迟 — ✅ 通过，余量大

**问题**：单字符编辑后重新编译，能否进 16ms（一帧）？
**标准**：目标 <16ms，可接受 <50ms。

每档 30 次单字符编辑（20 次用于大文档档），报告 p95。

| 规模 | 冷启动 | 无改动重编译 | 单字符编辑 p95 | 判定 |
|---|---|---|---|---|
| 1 页 / 240 B | 2.4ms | 4.3µs | 139µs | ✅ |
| 1 页 / 1.1 KB | 1.7ms | 4.0µs | 270µs | ✅ |
| 3 页 / 3.4 KB | 3.0ms | 6.3µs | 552µs | ✅ |
| 8 页 / 9.1 KB | 6.3ms | 99µs | 1.50ms | ✅ |
| **27 页 / 34 KB** | 22.4ms | 34µs | **5.81ms** | ✅ MVP 目标规模 |
| 71 页 / 93 KB | 62.7ms | 61µs | 16.54ms | ⚠️ 需 debounce |

延迟随文档大小近似线性。

**结论**：MVP 目标规模（30 页）只用掉 5.81ms，约三分之一帧预算。
PLAN §5.3 的第 3 档（块级编译）可以从"P0 不达标就上"降级为
"超大文档的已知手段"，MVP 不需要它。到 ~70 页才越过 16ms，届时 debounce 就够。

**缓存驱逐后反而更快**（p95 46µs vs 552µs）：`comemo::evict(0)` 之后
重建的是精简缓存。说明长时间编辑不会因缓存膨胀而劣化。

## #3a IME 事件通路 — ✅ 通过（人工验证）

**问题**：中文输入法的预编辑串能不能拿到？候选框能不能定位？
**标准**：能收到 `Preedit` 和 `Commit`，`set_ime_cursor_area` 能固定候选框位置。

Wayland + fcitx5 + rime，实测一轮 44 次 `Preedit` / 2 次 `Commit`：

| 项 | 结果 |
|---|---|
| `Ime::Enabled` | ✅ 窗口获得焦点后 12ms 到达 |
| `Ime::Preedit` | ✅ 拼音串增量到达，如 `"w"` → `"wo"` → `"wo f"` → `"wo fu le"` |
| `Ime::Commit` | ✅ 选字后提交 `"我服了"` |
| 候选框位置 | ✅ 跟随 `set_ime_cursor_area`，贴着窗口内的模拟光标 |
| cursor 范围 | ✅ 40 次非空 `Preedit` **全部**带 `Some((0, N))` |

**结论：winit 路线在 Linux 上成立，不需要退回 Qt 6。**

Windows / macOS 尚未验证，排到 P1 的三平台 CI 里做。

### 两个坑，都是调用时序问题

**坑 1：Wayland 下不提交缓冲区，窗口根本不显示。**

最初的 spike 只创建窗口不绘制，结果窗口完全不出现，`Ime::Enabled` 也永远收不到
（拿不到焦点）。X11 下空窗口会显示，Wayland 不会——合成器没有内容可合成。

加 softbuffer 画一块底色后窗口立刻可见，`Ime::Enabled` 12ms 就到。

**这个坑的危险在于它伪装成"winit 的 IME 坏了"。** 如果当时据此判 3a 失败去换 Qt 6，
就是被一个渲染时序问题误导着推翻了框架选择。

**坑 2：`set_ime_cursor_area` 必须在 `Ime::Enabled` 之后调用。**

在 `resumed()` 里调用无效，候选框落到窗口左下角（fcitx5 默认位置）。

原因在 winit 源码 `platform_impl/linux/wayland/window/state.rs:1044`：

```rust
for text_input in self.text_inputs.iter() {
    text_input.set_cursor_rectangle(x, y, width, height);
    text_input.commit();
}
```

`text_inputs` 在窗口拿到焦点前是空集合，循环体一次都不执行——**调用静默变成空操作，
没有任何错误返回**。上一层 `window/mod.rs:604` 还有个 `if window_state.ime_allowed()`
的早退，也是静默的。

正确做法：在 `Ime::Enabled` 之后调，且每次光标移动都重推。后者不只是为了绕开这个坑，
真实编辑器里光标随输入移动，本来就必须每帧重推。

### `Preedit` 的 cursor 字段语义

`Ime::Preedit(text, cursor)` 的 `cursor: Option<(usize, usize)>` 是**指向 `text` 内部的
两个字节偏移**，不是文档光标。它告诉你输入法希望预编辑串的哪一段被高亮、插入点画在哪。

fcitx5 + rime 给的一律是 `Some((0, N))`，`N` == 字节长度，即"整串一段，插入点在末尾"。

这个字段存在是因为某些输入法会给**子段**——日文输入法在转换阶段会把预编辑串切成
"已确定 / 正在转换 / 未转换"几段，每段下划线样式不同。中文输入法一般不用这个粒度。

对实现的要求：`(0, N)` 是常态，按"整串一段 + 末尾插入点"绘制即可，
但数据结构上不要把它硬编码成整串，保留分段能力。

### 空串 Preedit 是清除信号

`Preedit("")` 且 `cursor=None`，在 `Commit` 前后各出现一次。这是正常的状态转移，
不是异常：

```
Preedit("wo fu le")  → Preedit("") + cursor=None  → Commit("我服了") → Preedit("")
```

实现时按状态机处理。**空串 + `cursor=None` 不能当作"输入法没给光标范围"的缺陷**——
spike 最初的判据就犯了这个错，把 4 次空串算成需要兜底，报了误警。
判据应该是"非空 `Preedit` 是否缺 cursor"。

## #3b vello 渲染 Typst Frame — 未开始

接口层面已查证可对接，无缺口：

| vello 需要 | Typst 提供 | 转换 |
|---|---|---|
| `FontData::new(Blob<u8>, u32)` | `Font::data()` / `Font::index()` | `Blob::new` 包一层 |
| `Glyph { id: u32, x: f32, y: f32 }` | `Glyph.id: u16` + offset/advance | `u32::from`，坐标乘字号 |
| `.font_size(f32)` | `TextItem.size: Abs` | `.to_pt()` |

`vello::Glyph` 实际定义在 `vello_encoding`，由 vello 顶层重导出。

## API 陷阱记录

typst 0.15.1 的实际 API 与 docs.rs 展示的**不一致**，以下均为实测修正：

| docs.rs 显示 | 实际 |
|---|---|
| `FileId::new(None, VirtualPath)` | `FileId::new(RootedPath)`，`RootedPath::new(VirtualRoot::Project, vpath)` |
| `VirtualPath::new() -> Self` | 返回 `Result<Self, PathError>` |
| `World::today(Option<i64>)` | `Option<Duration>`（`typst::foundations::Duration`） |
| `Library::default()` | 需 `use typst::LibraryExt` 在作用域内 |
| `typst::layout::PagedDocument` | 在 `typst-layout` crate，需单独加依赖 |
| `doc.pages` 字段 | 私有，用 `doc.pages()` 方法 |
| `Source::range(span)` | `Source::range(SpanNumber, Option<SubRange>)`；更好用的是 `typst::WorldExt::range(span)` |
| `VirtualPath::as_rootless_path()` | 已弃用，改 `get_without_slash()` |

**教训：查 `~/.cargo/registry/src/*/typst-*-0.15.1/src/` 里的源码比查 docs.rs 可靠。**

## spike 自身的两个 bug（值得记）

1. 延迟测试最初在 `byte_len / 2` 处插字符，落进了 `sum` 中间变成 `sxum`，
   编译报 `unknown variable`，测出来就不是排版延迟。改成锚定正文里的固定串。
2. `src[mid..]` 在 `mid` 不是字符边界时 panic（文档含 CJK）。切片前必须先对齐。

两条都属于"测量代码本身出错导致数据无意义"，比被测对象出错更难发现。
