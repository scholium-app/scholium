# Scholium /「注疏」— 实施方案

科学写作桌面应用。原生 UI，Typst 排版内核，结构化数学编辑。

取代 `../../plans/RIIR.md`（Typst 源码编辑路线）和 `../../plans/STRUCTURED_MATH_TYPST.md`
（Tiptap + 双渲染器路线）。旧的 `mogan-rs` 与 `mogan` 均不复用任何代码。

**名称**：scholium 是古典手稿页边的注疏。欧几里得《几何原本》的 scholia
是希腊数学史最重要的传世材料之一。中文名「注疏」。

**定名状态**：crates.io / npm / `scholium.dev` / `scholium.io` 均未被占用（2026-07-28 查）。
GitHub 同名个人空号存在，org 需用变体。**商标未检索，正式发布前必须补 USPTO + CNIPA。**

## 1. 为什么重做

旧方案把 Typst 当**导出器**：

```
AST → Typst 源码 → SVG → 塞进 iframe/图片控件
```

屏幕上的字形和 AST 节点之间没有对应关系，于是点击无法定位、光标无法进入分母、
矩阵单元格无法高亮。`STRUCTURED_MATH_TYPST.md` 因此被迫引入 KaTeX 做编辑态渲染，
结果是两套排版引擎、两种排版结果，所见永远非所得。

而全部辨识度就在结构化数学编辑手感上。管线隔断了这个能力，产品就没有存在理由。

实际完成度也印证了问题所在——排版闭环一行未通，而 `mogan-ai` 已经建好了目录：

| 模块 | 计划状态 | 实际 |
|---|---|---|
| `mogan-typst::Compiler` | 阶段 1 核心 | 全是 `Ok(())` 空壳，`typst` crate 未被调用 |
| `mogan-typst::MoganWorld` | impl `World` | 空 struct，无 impl |
| `mogan-core::DocumentTree` | 结构化 schema | `{ root: String }` |
| `mogan-convert` | 1500+ 映射规则 | 返回空串 |
| `mogan-ai` / `mogan-bib` | 阶段 4/6 | 空 struct |
| `Preview.svelte` | Typst SVG 预览 | 写死的假页面 |
| 前端数学原型 | 阶段 2-4 | 唯一真有内容，~3.7k 行 |

## 2. 新方案：Typst 当排版库，不当导出器

```
文档 AST（唯一真相）
   │  serialize，同时产出 SourceMap: byte range → NodeId
   ▼
Typst source（内存中，用户永不可见）
   │  typst::compile
   ▼
PagedDocument → Frame → FrameItem
   │  逐字形坐标 + Glyph.span
   ▼
Span → byte range → 查 SourceMap → NodeId
   │
   ▼
vello 直绘 + 命中测试 + 光标定位 + 选区
```

单引擎。编辑态和最终排版是同一次 layout 的产物。

### 已验证的 API 事实（typst 0.15.1）

```rust
// typst::layout::Frame
pub fn items(&self) -> Iter<'_, (Point, FrameItem)>   // 位置相对 frame 左上角

// typst::layout::FrameItem
Group(GroupItem)                    // 嵌套 frame + 变换/裁剪
Text(TextItem)                      // 一段 shaped text（enum 层无 Span）
Shape(Shape, Span)                  // ← 带 Span：分数线、根号、表格线
Image(Image, Axes<Abs>, Span)       // ← 带 Span
Link(Destination, Axes<Abs>)
Tag(Tag)

// typst::text::TextItem
pub font: FontInstance, pub size: Abs, pub fill: Paint,
pub text: EcoString, pub glyphs: Vec<Glyph>, ...

// typst::text::Glyph  ← 关键
pub id: u16,
pub x_advance: Em, pub x_offset: Em,
pub y_advance: Em, pub y_offset: Em,
pub range: Range<u16>,      // 在所属 TextItem.text 内的字节范围
pub span: (Span, u16),      // 源码位置 + 偏移
```

**每个字形都带 span**，粒度足够，无需往生成的源码里插标记节点。
`Span → 源码字节范围` 用 `typst_syntax::Source::range(span)`。

未确认：`span` 元组第二个 `u16` 的语义（文档未说明，推测是 ligature 内偏移），P0 实测。

## 3. 技术栈

| 层 | 选择 | 版本 | 责任 |
|---|---|---|---|
| 窗口/事件/IME | winit | 0.30+ | 跨平台窗口、键鼠、`Ime` 事件 |
| GPU | wgpu | 匹配 vello | vello 后端 |
| 2D 渲染 | vello | 0.9 | 字形/路径/裁剪绘制 |
| UI 文本布局 | parley | 0.11 | 面板、菜单标签、行内文本框（**不用于文档排版**） |
| 排版 | typst | 0.15 | 文档 layout、Frame、PDF |
| 菜单 | muda | — | 原生菜单栏 |
| 文件对话框 | rfd | — | 原生 open/save |
| 无障碍 | accesskit | — | 屏幕阅读器 |
| 错误 | thiserror | — | 不用裸 String |
| 序列化 | serde + serde_json | — | `.schol` 文档格式 |

不用完整 UI 框架：编辑区 100% 自绘，重量级框架的控件用不上，只会付抽象成本。
菜单和对话框用 muda/rfd 补齐，这是唯一两块真正缺的原生能力。

typst 0.15.1 要求 edition 2024 / MSRV 1.92。

## 4. Crate 划分

```
scholium/
├── crates/
│   ├── scholium-doc/        文档 AST、Math AST、NodeId、EditOp（纯逻辑，无 IO）
│   ├── scholium-serialize/  AST → Typst source + SourceMap
│   ├── scholium-layout/     typst World、编译缓存、Frame → 绘制原语 + 反向映射
│   ├── scholium-input/      乐高规则、Tab 循环、命令注册表、keymap
│   ├── scholium-render/     vello 绘制、命中测试、光标/选区几何
│   ├── scholium-agent/      AI 接入层（接口先行，实现后置 → 见 §6）
│   ├── scholium-shell/      winit 事件循环、IME、菜单、对话框、面板
│   └── scholium-import/     LaTeX / Markdown / .tm 导入（1.0 之后）
└── assets/fonts/            New Computer Modern（数学）+ Noto Serif CJK，均 OFL
```

依赖方向严格单向：`doc ← serialize ← layout ← render ← shell`，`agent` 只依赖 `doc`。

`scholium-doc` 和 `scholium-serialize` 不依赖 winit/wgpu/vello/网络，可纯单元测试，
是覆盖率的主力，也是 AI 层能被离线测试的原因。

## 5. 核心机制

### 5.1 SourceMap

序列化时记录每个 AST 节点写入的字节区间：

```rust
pub struct SourceMap {
    entries: Vec<(Range<usize>, NodeId)>,   // 按 start 排序，允许嵌套
}

impl SourceMap {
    /// 返回包含该字节范围的最内层节点
    pub fn innermost(&self, range: Range<usize>) -> Option<NodeId>;
}
```

序列化器是唯一写入方，区间天然精确，不存在解析猜测。

反查：`Glyph.span` → `Source::range()` → `SourceMap::innermost()` → `NodeId`。

**边界情况**：Typst 自己生成的内容（公式编号、定理计数器、模板产出文字）
span 指向 Typst 库文件而非我们的源码，`innermost()` 返回 `None`。
这类区域标记为「派生内容」，只读、不可点入——正是期望行为。

### 5.2 光标模型

光标住在 AST 里，不住在 layout 里。layout 是派生物。

```rust
pub struct Cursor {
    path: Vec<usize>,     // 从 doc 根到目标节点的索引路径
    offset: usize,        // 节点内字符偏移（文本）或槽位（结构）
}
```

- **屏幕 → 光标**（点击）：遍历 Frame 找最近字形 → span → NodeId → 结合 `Glyph.range` 定位字内偏移。
- **光标 → 屏幕**（画 caret）：光标转成源码字节偏移，在 Frame 里找 `range` 覆盖该偏移的字形，取坐标。

结构导航（进分母、跳出根号、矩阵单元格移动）完全在 AST 上做，不碰 layout。

### 5.3 编译节流

1. **乐观绘制**：按键立刻应用到 AST，用上一帧 layout 近似定位，先出画面。
2. **debounce 编译**：停顿 ~8-16ms 触发 Typst 编译，拿到真 layout 后校正。
3. **块级编译**（仅当 P0 实测全量不达标）：只编译当前 block，全量放到 idle。

Typst 内部 hash-based memoization，未改动内容命中缓存。实际延迟见 P0。

### 5.4 IME

中文产品的最大工程风险，也是选 winit 而非完整框架的主要代价。

winit 给 `WindowEvent::Ime { Preedit(String, Option<(usize, usize)>) | Commit(String) }`。
需自己做：

- preedit 串作为**临时覆盖节点**插入 AST，不进 undo 栈。
- 重新 layout + 绘制，带下划线。
- `Window::set_ime_cursor_area()` 定位候选框，否则候选窗飘到屏幕角落。
- `Commit` 时替换为正式节点，进 undo 栈。

三平台行为不一致，Linux 还要区分 fcitx5 / ibus。预算 1-2 周，P0 必须验证。

## 6. AI 接入（接口先行，实现后置）

**设计原则：AI 不是外挂功能，是编辑操作的第二个来源。**

`mogan-ai` 那种「providers/ 里放三个空 struct」的做法是错的——它把 AI 当成一个
调 HTTP 的边缘模块，而真正的难点是让 AI 安全地改文档。所以这一层从 §5 的
`EditOp` 通道设计起，P1 就把接口定下来，实现推到 MVP 之后。

### 6.1 单一变更通道

所有编辑——键盘、菜单、AI——都必须经过同一个操作类型。这是 AI 能力的前提：

```rust
// scholium-doc
pub enum EditOp {
    InsertText   { at: Cursor, text: String },
    DeleteRange  { range: Selection },
    ReplaceNode  { id: NodeId, with: Node },
    InsertNode   { at: Cursor, node: Node },
    WrapNode     { id: NodeId, wrapper: NodeKind },   // 如给公式套 frac
    SetAttr      { id: NodeId, key: AttrKey, value: AttrValue },
}

/// 一批原子应用的操作，undo 栈的最小单位
pub struct Transaction {
    ops: Vec<EditOp>,
    origin: Origin,
}

pub enum Origin { User, Agent { session: AgentSessionId }, Import }
```

`Origin` 让 AI 的改动可被单独标识：撤销时可整批回退，UI 上可高亮"这段是 AI 改的"，
审计时可区分来源。这是事后加不进去的东西，所以 P1 就要有。

### 6.2 AI 侧接口

```rust
// scholium-agent —— P1 只定义 trait 和类型，不写实现

/// AI 读文档：把 AST 投影成 LLM 能消费的形式
pub trait DocumentView {
    /// 带 NodeId 标注的文本投影，让模型能指名道姓地引用节点
    fn outline(&self) -> Vec<(NodeId, NodeKind, String)>;
    fn node_text(&self, id: NodeId) -> Option<String>;
    /// 数学节点导出为 LaTeX——LLM 对 LaTeX 的掌握远好于 Typst
    fn math_as_latex(&self, id: NodeId) -> Option<String>;
    fn selection(&self) -> Option<Selection>;
}

/// AI 写文档：产出 EditOp，由宿主决定是否应用
pub trait Agent {
    fn capabilities(&self) -> AgentCapabilities;
    async fn propose(&self, req: AgentRequest) -> Result<Proposal, AgentError>;
}

pub struct Proposal {
    transaction: Transaction,
    rationale: String,
    confidence: Option<f32>,
}

/// 宿主对提案的处置——AI 永不直接写 AST
pub enum Disposition { Apply, ApplyAsSuggestion, Reject }
```

关键约束：**Agent 只能返回 `Proposal`，不能直接拿到 AST 的可变引用。**
所有 AI 改动都要经过宿主的 `Disposition` 关卡，默认走 `ApplyAsSuggestion`
（渲染成建议态，用户逐条接受）。这条边界一旦破了就再也收不回来。

### 6.3 预留的能力位

`AgentCapabilities` 是位标记，实现可以逐个点亮，接口不用改：

```rust
pub struct AgentCapabilities {
    pub math_from_prose: bool,      // "求 x 平方的导数" → 公式节点
    pub latex_paste: bool,          // 粘贴 LaTeX/截图 OCR → 结构化数学
    pub rewrite: bool,              // 改写选中段落
    pub explain: bool,              // 解释选中公式（只读，不产生 EditOp）
    pub structure_fix: bool,        // 修矩阵维度、补缺失定界符
    pub translate: bool,
    pub cite_suggest: bool,         // 需 §P5 文献库
}
```

### 6.4 分工

| 时点 | 做什么 |
|---|---|
| P1 | `EditOp` / `Transaction` / `Origin` 落地。所有键盘输入走这条通道 |
| P1 | `scholium-agent` 建 crate，只有 trait 定义 + `Proposal` 类型 + 单元测试用的 `MockAgent` |
| P2-P4 | 每加一种文档结构，同步补 `DocumentView` 的投影（顺手，几乎零成本） |
| **MVP 之后** | 接真实 provider（HTTP/SSE）、建议态 UI、prompt 工程 |

用 `MockAgent` 就能在没有任何网络代码的情况下测通「提案 → 处置 → 应用 → 撤销」全链路。
这是把接口先行做实的验收标准，不是写完 trait 就算完。

### 6.5 明确不做

- 不在 P1-P4 写任何 HTTP/SSE 代码。
- 不做 API key 管理（等有真 provider 再说，届时走系统 keyring，不落明文配置）。
- 不做本地模型推理。
- Agent 不获得文件系统或 shell 访问权，只能操作当前文档 AST。

## 7. 分阶段计划

### P0 — 技术验证（1-2 周，代码可弃）

不搭架子，只回答三个能否定整个方案的问题。

| # | 验证内容 | 通过标准 | 不通过的退路 |
|---|---|---|---|
| 1 | 硬编码含数学的 Typst，编译，遍历 Frame，打印每个字形的坐标 / span / range | span 粒度到单个数学原子（分子、分母、上标各自可区分） | 序列化时插标记节点提高分辨率 |
| 2 | 1 / 10 / 30 页文档，单字符编辑后重编译，测延迟 | 目标 <16ms，可接受 <50ms | 降级块级编译 |
| 3 | winit 空窗口 + 中文输入法，取 preedit、绘制、定位候选框 | Linux + Windows 可用 | **重新考虑 Qt 6**（IME 成熟度是 Qt 最强项） |

第 3 项失败则框架选择需重新讨论——这也是把它放在写任何产品代码之前的原因。

### P1 — 骨架（3-4 周）

- winit + wgpu + vello 窗口，能画文字和图形。
- `scholium-doc` 最小 AST：`doc / paragraph / heading / text`。
- **`EditOp` / `Transaction` / `Origin` 通道**，所有输入走它（见 §6.1）。
- `scholium-serialize` + SourceMap。
- `scholium-layout`：真实 `impl World`、字体加载、编译缓存。
- Frame → vello scene 绘制。
- 光标 + 命中测试 + 键盘输入 + IME 中文输入。
- undo/redo。
- **`scholium-agent` trait 定义 + MockAgent 全链路测试**（见 §6.4）。
- CI：fmt / clippy / test / 三平台 build。

**验收**：用中文输入法打一段中英混排，光标点击定位准确，撤销重做正常，
排版质量就是 Typst 的排版质量；`MockAgent` 能提交提案并被撤销。

### P2 — 数学核心（4-5 周）

Math AST（`row / symbol / frac / script / root / delimited / accent / bigop`）、
数学环境（行内/单行/多行）、结构内导航、结构化选区、数学结构命中测试与 caret 定位。

**验收**：能输入 `x^2`、`1/2`、`sqrt(x)`、`sum_(i=1)^n`，光标能进分母打字，
能选中整个分数删除，全程所见即所得。

### P3 — 输入手感（3-4 周）

命令注册表 + keymap（替代 Scheme 命令层）、乐高符号缓冲规则（`<`+`=` → `≤`）、
Tab 循环等价类（`a → α → ℵ`）、结构变体循环（分数 普通/小型/显示/斜线；
定界符 无/圆/方/竖/花）、符号面板。

**此处为 MVP**，累计约 12-15 周。

### P4 — 矩阵与多行（3 周）

矩阵 / 分段函数 / 对齐公式的 AST 与编辑：单元格导航、`Alt+方向` 插入行列、
空单元格 Backspace 语义。

### P5 — 持久化与导出（2-3 周）

`.schol` JSON 格式（serde）保存/打开、PDF 导出（Typst 原生）、最近文件、
崩溃恢复、Typst 编译错误 → span → NodeId → 节点级诊断标注。

### P6 — 应用外壳（3-4 周）

muda 菜单、rfd 对话框、文件树、大纲、偏好设置、accesskit 无障碍、i18n（中/英）。

### P7 — 打包（2-3 周）

三平台打包、自动更新、代码签名。

### 1.0 之后

AI provider 实装 + 建议态 UI、结构化搜索替换、LaTeX/Markdown/`.tm` 导入、文献管理。

## 8. 明确不做

| 不做 | 理由 |
|---|---|
| 移植任何 C++/Scheme 代码 | 纯重写，也让许可证保持自由（见 §9） |
| Scheme 解释器 / `.scm` 宏兼容 | 命令注册表 + Typst 模板替代 |
| 协作编辑 | CRDT 复杂度爆炸 |
| 1.0 内做格式转换 | 转换器是无底洞 |
| Typst 源码编辑模式 | 与结构化编辑是两个产品 |
| 双渲染器 | 单引擎是本方案核心前提 |
| 1.0 内接真实 AI provider | 接口先行、实现后置（§6.4） |

## 9. 许可证

- Typst 是 **Apache-2.0**，单向兼容进 GPL-3 但不强制。
- 只要不移植 TeXmacs/Mogan 任何代码，Scholium 可自由选许可证（建议 Apache-2.0 或 MIT/Apache 双授权）。
- 一旦移植原项目代码，必须 GPL-3。**建议严格不碰原代码**，保留选择权。
- 字体：New Computer Modern 与 Noto 系列均 OFL，可随应用分发。
- `.tm` 导入若将来要做，只读文件格式不构成衍生作品，但不要抄解析器实现。
- CI 加 `cargo-deny` 做许可证门禁。

## 10. 风险表

| 风险 | 概率 | 影响 | 处理 |
|---|---|---|---|
| IME 在某平台不可用 | 中 | 高 | P0 验证；失败则换 Qt 6 |
| 全量编译延迟超标 | 中 | 中 | P0 实测；降级块级编译 |
| span 粒度不足 | 低 | 高 | 已查文档确认逐字形带 span；不足则插标记节点 |
| typst 0.16 breaking change | 高 | 中 | 锁次版本号；`scholium-layout` 是唯一接触点 |
| AI 层被提前拉进 MVP | 中 | 中 | §6.5 硬性边界；P1 只有 trait |
| 无障碍工作量被低估 | 中 | 低 | accesskit 排 P6，不阻塞 MVP |
| 又一次范围失控 | **高** | **高** | 硬性规定：P3 结束前不碰转换/文献/AI 实现 |
| 商标冲突 | 未知 | 高 | 发布前必须查 USPTO + CNIPA（quire 就是在这翻的车） |

最后两条是最大的风险。前两份计划都是被范围拖死的，不是被技术拖死的。

## 11. 开发规范

- 分支 `username/<type>/<desc>`，提交 `<type>: <简述>`，直接 `git push`。
- Edition 2024，`pub` 类型实现 `Debug`，有 `new()` 就有 `Default`，错误用 `thiserror`。
- CI 门禁：fmt / clippy `-D warnings` / test / `cargo crap` CRAP>30 / audit / doc。
- `scholium-doc`、`scholium-serialize`、`scholium-agent` 要求高覆盖率（纯逻辑，无 IO 借口）。

## 12. 定名待办

- [x] crates.io `scholium` 空（2026-07-28）
- [x] npm `scholium` 404
- [x] `scholium.dev` / `scholium.io` 无 NS 记录，很可能未注册
- [ ] 抢注 `scholium.dev` + `scholium.io`
- [ ] GitHub org（同名个人空号占位，需用变体如 `scholium-app`）
- [ ] **商标检索 USPTO + CNIPA**
- [ ] 中文名「注疏」实际使用感受确认
