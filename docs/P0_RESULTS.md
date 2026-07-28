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

## #3b vello 渲染 Typst Frame — ✅ 通过

**问题**：Typst 排版产出的字形能不能直接用 vello 画出来？
**标准**：字形位置正确，数学结构（分数线、根号、矩阵）形状正确。

这验证的是 PLAN §2 管线的最后一段（Frame → vello scene → 屏幕）。
配合 #1 验证的 Frame → NodeId 反查，整条管线闭环。

实测一页含分数、根号、矩阵、求和的文档：
**92 字形 / 5 shape / 68 text run / 7 group / 嵌套最深 2 层**，全部走完绘制路径。
公式渲染人工确认正确。

### 接口映射

| vello 需要 | Typst 提供 | 转换 |
|---|---|---|
| `FontData::new(Blob<u8>, u32)` | `Font::data()` / `Font::index()` | `Blob::new(Arc::new(data.to_vec()))` |
| `Glyph { id: u32, x: f32, y: f32 }` | `Glyph.id: u16` + offset/advance | `u32::from`，坐标乘字号 |
| `.font_size(f32)` | `TextItem.size: Abs` | `.to_pt()` |
| `BezPath` | `Geometry::{Line,Rect,Curve}` | `CurveItem` 与 kurbo 路径模型一一对应 |

`vello::Glyph` 实际定义在 `vello_encoding`，由 vello 顶层重导出。

### 三个必须处理的坐标问题

1. **Y 轴方向相反。** Typst 的 `y_offset` / `y_advance` 是 **Y-up**，vello 是 **Y-down**，
   要取反。
2. **字形绝对位置要自己累加。** Typst 只给每字形的 `x_offset` + `x_advance`，
   沿 x 累加才得到绝对位置。
3. **`Group` 的变换要逐层叠加**（`transform * translate`），否则嵌套结构（矩阵、分式）错位。

这三处任一处错了，表现都是"能画出来但位置不对"，不会报错。

## 字体：`typst-assets` 没有 CJK 字体

`typst_assets::fonts()` 只捆了 New Computer Modern / Libertinus / DejaVu Mono，
**一个 CJK 字体都没有**。中文优先的产品必须自带字体，PLAN §3 的资产清单
（New Computer Modern + Noto Serif CJK，均 OFL）不是可选项。

### 这是一次静默降级

缺字体时 Typst **不报任何错**：编译成功、span 正确、延迟正常，
只是返回 `id=0` 的 `.notdef` 字形。只有画到屏幕上才看得出问题。

#1 的输出里其实已经有证据——每个汉字的字形 `id` 都是 `0`，
而同一份输出里数学字形是 3548、6600 这些真实索引。当时没警觉，
因为验的是 span 映射，那部分确实通过了。

加载系统 `NotoSerifCJK-Regular.ttc`（含 5 个 face）后：

| | 加载前 | 加载后 |
|---|---|---|
| 字体数 | 17 | 22 |
| 「标」的字形 id | 0（`.notdef`） | 21038 |
| 「题」的字形 id | 0 | 44083 |
| span 统计 | 32 字形 / 零 detached | **完全一致** |
| 27 页编辑 p95 | 5.81ms | 6.34ms（+9%） |
| 27 页冷启动 | 22.4ms | 31.0ms |

span 统计一字不差，印证了缺字体不影响反向映射，#1 的结论仍然成立。
延迟代价约 9%，在预算内。

### 对 P1 的要求：启动时主动自检

已在 spike 的 `SpikeWorld::assert_coverage` 里实现——加载完字体后探关键码位
（一个常用汉字 + 一个数学符号），用 `FontInfo.coverage.contains()` 查。

`scholium-layout` 要保留这个自检。**把静默降级变成启动时的显式警告**，
否则这类问题只能靠肉眼在渲染结果里发现。

## 三个同类模式：静默失败

P0 至今踩到三个，全都不报错、不 panic，只能靠肉眼或额外断言发现：

| 现象 | 真因 | 防御 |
|---|---|---|
| 窗口完全不出现，收不到 IME 事件 | Wayland 下不提交渲染缓冲区窗口不显示 | 起步就画底色 |
| 候选框飘到窗口左下角 | `set_ime_cursor_area` 在焦点前调用，`text_inputs` 为空集合，循环体不执行 | 只在 `Ime::Enabled` 之后调 |
| 汉字渲染成方块/空白 | 缺 CJK 字体，返回 `.notdef` | 启动时探关键码位 |

第一个最危险——它伪装成"winit 的 IME 坏了"。如果当时据此判 3a 失败去换 Qt 6，
就是被一个渲染时序问题误导着推翻了框架选择。

**P1 的启示：凡是"配置/资源/时序不对但不报错"的路径，都要主动加断言。**

## API 陷阱记录

typst 0.15.1 的实际 API 与 docs.rs 展示的**不一致**，以下均为实测修正：

### typst 0.15.1

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

### vello 0.9 / wgpu 29

| 以为 | 实际 |
|---|---|
| `wgpu` 要单独加依赖 | vello 重导出了，`use vello::wgpu` |
| `Line`/`Rect` 有 `into_path` | 需 `use vello::kurbo::Shape as _` 让 trait 在作用域 |
| `get_current_texture() -> Result` | **wgpu 29 改成枚举** `CurrentSurfaceTexture`，有 `Timeout`/`Occluded`/`Outdated` 变体，该跳帧而非 panic |
| `Color::to_rgb().to_vec4()` | `to_vec4` 挂在 `Color` 上 |
| `AlphaColor::new` 能推类型 | 推不出色彩空间，要写 `AlphaColor::<Srgb>::new` |
| `Font::ttf()` 可用 | 在 `typst::text::Font` 上取不到（`ttf_parser` 未作为直接依赖）；用 `FontInfo.coverage.contains()` 代替 |

累计 14 处签名与文档/直觉不符。

**教训：查 `~/.cargo/registry/src/` 里的源码比查 docs.rs 可靠。**

对 P1 的实际含义：`scholium-layout` 是 PLAN §4 里特意隔离出来的、唯一接触 typst 的 crate，
这个隔离设计现在验证是对的——所有签名变动都被挡在一个 crate 内。
但工期上要给"查真实签名"预留比照文档写代码更多的时间。

## spike 自身的两个 bug（值得记）

1. 延迟测试最初在 `byte_len / 2` 处插字符，落进了 `sum` 中间变成 `sxum`，
   编译报 `unknown variable`，测出来就不是排版延迟。改成锚定正文里的固定串。
2. `src[mid..]` 在 `mid` 不是字符边界时 panic（文档含 CJK）。切片前必须先对齐。

两条都属于"测量代码本身出错导致数据无意义"，比被测对象出错更难发现。
