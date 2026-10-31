# 2026-10-31：rtex 页面内数学编辑评估（T22，E9 M1–M7）

本记录追溯一次允许修改 rtex 的评估，不修改产品承诺、不作选型 ADR。
背景为 [ADR 0043](../adr/ADR-0043-rtex-live-preview.md)、[ADR 0029](../adr/ADR-0029-direct-page-editing.md)
与 [LaTeX 工作区 L0/L3](../plan/LATEX_WORKSPACE.md)。E1–E8 的 [第一轮](2026-10-30-rtex-evaluation.md)
和 [第二轮](2026-10-30-rtex-evaluation-2.md)没有验证结构数学编辑。
驱动、限定语法和完整复现命令见 [spike README](../../spikes/rtex-math-edit/README.md)。

## 结论

**完整目标 Fail；按本轮实现，L3 的 TeXmacs 式结构数学编辑不可采用。**
“不可采用”限定于本轮补丁与驱动，不是证明 LuaTeX 原理上不能实现。
引擎端标记已消除 E5 的 TFM 字形来源缺口，输出身份和微型夹具的实时性通过；阻塞项是空槽几何、
结构插入后来源表过期，以及错误编译仍发布新页面。不能凭 100% 的静态字形映射把这些项目判为通过。

| 项目 | 方法与数字（每种字体各 12 个公式） | A / B 与判定 | 原始证据 |
|---|---|---|---|
| M1 输出身份 | CM/TFM、unicode-math，tagged/untagged 四次 verify 均 12/12 单元、232/232 字形；页面 1/1 exact；捕获 PDF=干净 PDF。两对完整编译的 300 dpi 全页 PGM SHA-256 完全相同，238 个字形/规则项也相同 | **Pass**，A 否决门未触发；B 使用未改动引擎 | [CM A](evidence/rtex-math-edit/cm-A-verify.json)、[CM 基线](evidence/rtex-math-edit/cm-base-verify.json)、[Unicode A](evidence/rtex-math-edit/unicode-A-verify.json)、[Unicode 基线](evidence/rtex-math-edit/unicode-base-verify.json)、[完整编译身份](evidence/rtex-math-edit/summary.json) |
| M2 映射覆盖率 | A 每种字体 73/73（100%），字体解码与来源标签逐项一致；B 读取未补丁页面，TFM 编码表/OpenType MATH 变体解码后均 69/73（94.52%）精确 | **A Pass（冻结夹具）/ B Conditional**；B 仍漏配绘制顺序不同的结构 | [逐字形评分与真值](evidence/rtex-math-edit/summary.json)、[源码范围](evidence/rtex-math-edit/cm-source-map.json) |
| M3 结构与空槽 | A 每种字体 115/123 节点有字形并集框，8 个缺失；B 109/123。6 个空槽的框均为 0/6，另缺空分式及整个空公式的框 | **A/B Fail**；没有用占位墨迹改变完整编译来“补足” | [A 页面及 noad 观察](evidence/rtex-math-edit/cm-A-pages.json)、[Unicode](evidence/rtex-math-edit/unicode-A-pages.json)、[缺失节点](evidence/rtex-math-edit/summary.json) |
| M4 光标语义 | A 每种字体点击/左右/上下 378/380 断言通过，空 num/den 两条上下断言失败；B 字形点击 128/134。x→z 同字节替换 2/2 通过；x→非空分式后点击 b：CM 返回 85 而应为 94，Unicode 返回 142 而应为 151，新 num/den 槽均没有生成 | **A/B Fail**；静态遍历和一次替换不能覆盖任意结构编辑 | [CM 断言](evidence/rtex-math-edit/cm-caret-checks.json)、[Unicode 断言](evidence/rtex-math-edit/unicode-caret-checks.json)、[结构插入反例](evidence/rtex-math-edit/structural-edit-summary.json) |
| M5 时延 | 两字体×两后端×四种模式×两节奏×20 次=640 次，全部 fast/ok。A 稳定输入到栅格 p95：CM inline/display/equation/align 为 3.391/7.061/7.459/16.892 ms，Unicode 为 3.408/7.221/7.348/12.757 ms | **Conditional**：小夹具宿主流水线通过，未测真实窗口/大文档；A 的结构观察和 JSON 开销可见 | [CM A 样本](evidence/rtex-math-edit/cm-A-keystrokes.json)、[CM 基线](evidence/rtex-math-edit/cm-base-keystrokes.json)、[Unicode A](evidence/rtex-math-edit/unicode-A-keystrokes.json)、[Unicode 基线](evidence/rtex-math-edit/unicode-base-keystrokes.json) |
| M6 中间状态与恢复 | 每字体 17 个完整状态合并编译均 exit 0；命令前缀只缓冲，最终只发送 alpha。8 次非法状态试验中继续键入 q，修复后 8/8 实时出现 q，修复/还原的导出快照 16/16 精确；恢复 live 1.619–3.312 ms。但错误编译会发布含变化页面的 CompiledWithErrors | **Conditional（宿主事务/输入保留）/ Fail（原样引擎保留最后良好布局）** | [CM 状态编译](evidence/rtex-math-edit/cm-intermediate.log)、[Unicode](evidence/rtex-math-edit/unicode-intermediate.log)、[最终恢复试验](evidence/rtex-math-edit/recovery-summary.json) |
| M7 数学内中文 | ctex 下 `text{中文}` 两种字体各 2/2 字形有精确源范围，UTF-8 每字 3 字节；M1 中逐字形及完整编译身份同时通过 | **Pass（4/4 中文字形）** | [CM 源码](../../spikes/rtex-math-edit/fixtures/cm.tex)、[A 字形与范围](evidence/rtex-math-edit/cm-A-pages.json)、[Unicode](evidence/rtex-math-edit/unicode-A-pages.json) |

## 环境与补丁

- 基线为 `e0ad11765ec016846b6e07619081f0528621636b`。原 `~/.cache/scholium-eval/rtex-v0.0.5`
  始终只读，最终 Git 状态干净；实验独立克隆为 `~/.cache/scholium-eval/rtex-math-edit`，不在主 workspace。
- Arch Linux，内核 `7.2.11-arch1-1`，LuaHBTeX 1.24.0 / TeX Live 2026，bubblewrap 0.13.1，rustc 1.99.0，
  fontTools 4.68.0。TeX 包版本、最终可执行文件和补丁 SHA-256 见 [环境清单](evidence/rtex-math-edit/environment.json)。
- 全部文档执行在原有 `rtex` 配置内：2 GiB 会话 cgroup、TasksMax=64、无网络、项目只读、构建目录可写；
  仍有每进程 CPU 30 s 和会话 300/600 s 上限，未改沙箱脚本。单进程先预热字体缓存，时延另预热栅格器。
- [补丁 0001](../../spikes/rtex-math-edit/patches/0001-observe-math-source-ownership.patch)：100 行新增、1 行删除，
  4 个文件。其中自写 Lua observer 58 行；在 `pre_mlist_to_hlist_filter` 观察 noad 子树，
  在既有 hlist 遍历读取属性与字形坐标。`math_edit.nodes` 给出源范围、父节点、种类与字形并集框，
  `math_edit.glyphs` 给出 Glyph 的 owner ID；noad 的 kind/role/临时父键仅供诊断。
  Rust 只增加可选元数据及私有二进制记录 0x40 的读写和往返测试。
- [补丁 0002](../../spikes/rtex-math-edit/patches/0002-allow-generated-attribute-tags.patch)：2 行新增，
  1 个文件，将 attribute 加入 math/text 白名单，使生成的标记（包括 `text` 内）走实时路径。
  这是实验开关，不是允许产品接受任意属性原语的安全裁决。
- 合计 **102 行新增、1 行删除，5 个上游文件**。补丁只包含自写增量与少量上下文，没有 vendor 上游实现。
  [apply 记录](evidence/rtex-math-edit/apply.log)确认临时 index 重建的每个 blob 与实验副本一致。

宿主解析器给原始、未标记源码中的 atom/slot 编号，属性 65000 专用于实验。已有组不额外包裹；单 token
脚标改为等价的带括号脚标（例如 2→{2}）。公式结束前把属性复位，防止公式编号继承最后的 atom ID。
语义层级由限定宿主解析给出，noad 观察补充实际引擎结构；不能把字形框并集称为完整引擎数学树。

外部仍走 JSON lines；为了不修改所有渲染分支，本轮用 display list 的可选 `math_edit` 元数据表，
没有新建 `Item::MathNode` 枚举分支。内部 Lua→Rust 原有二进制帧增加 0x40 元数据，之后正常随 JSON 交付。

## 映射、空槽与编辑反例

夹具覆盖行内、display、equation、align、嵌套分式/脚标、带次数根号、left/right、pmatrix、sum 上下限、
mathbb、ctex 中文和重复 x。每种字体 73 个公式字形，含根号、伸展括号和求和号；正文与公式编号不在分母中。
字体解码特别处理了 Latin Modern Math 的私用字符码：通过实际 glyph index、cmap 和 MATH 变体反查，
没有把私用字符当成 Unicode 源字符，也没有靠 A 的标签替 B 解码。

B 以实际未补丁页面为输入；A 只提供公式裁剪范围和评分真值。M1 已证明两侧字形/规则位置一致。
这对 B 是有利条件，自动裁剪和任意宏匹配未验证。B 按源码顺序对齐字形绘制顺序；余下漏配集中在
sup/sub 的顺序以及带次数根号，不能靠最近字节补齐后宣称精确。B 的结构框由已对齐字形向宿主父节点取并集，
不产生引擎额外几何；它也没有空槽。

A 缺失的 8 个节点是：空分式公式、该 frac、其 num/den、空 sup、空 sub、两个空矩阵 cell。
其中真正的空槽为 6 个。实现保留空槽源位置，但没有加入空 hbox/占位字形，也没有把邻近框猜成精确空槽；
结果是没有可命中或可画光标的框，M3 按 0/6 判 Fail。规则/box→空 noad 槽的几何出口没有完成。

光标按源结构顺序左右遍历，num/den、sub/sup 用共同父节点上下跳转。非空字形左右半区点击、同字节 x→z
重排可工作；空格、选择、IME、矩阵上下导航和一般宏展开不在小模型中。新结构插入的失败则是实测：
将 inline 的 x 替换为非空分式后，引擎仍返回 ok，分母 b 的 owner 却是旧 x 的 ID；导言区的来源表没有更新，
num/den 没有独立 ID，点击 b 回到旧 x 的字节。不能把“字体 glyph→源字符正确”当成动态结构编辑已通过。

## M5 原始时延口径与数字

每格为 p50/p95（ms）。U 是写 edit→同 edit_id 的 ParagraphUpdate，R 是写 edit→该单元 300 dpi 栅格完成。
全部统计仅包含最后一轮修正目标且预热后的 20 个样本，非计时预热另做两次插删；原始样本保留最大值与路由。
稳定节奏每次更新后停 150 ms；突发每次停 30 ms，每 5 次停 1.5 s，并等待每次回复，不是排队洪峰。

| 字体 / 模式 / 节奏 | 基线 U | A U | 基线 R | A R |
|---|---|---|---|---|
| cm / inline / steady | 1.137/1.360 | 1.995/2.468 | 1.831/2.289 | 2.824/3.391 |
| cm / inline / burst | 1.021/1.222 | 2.069/2.738 | 1.561/1.844 | 2.945/3.647 |
| cm / display / steady | 1.151/1.424 | 3.873/5.530 | 1.946/2.275 | 5.204/7.061 |
| cm / display / burst | 1.258/1.414 | 3.760/5.490 | 2.256/2.537 | 4.962/7.191 |
| cm / equation / steady | 1.861/2.019 | 4.192/5.837 | 2.908/3.233 | 5.727/7.459 |
| cm / equation / burst | 1.494/1.957 | 4.998/5.796 | 2.522/3.229 | 6.558/7.342 |
| cm / align / steady | 5.135/6.234 | 13.089/14.901 | 6.384/7.653 | 14.739/16.892 |
| cm / align / burst | 4.618/6.298 | 13.080/14.706 | 5.882/7.539 | 14.759/16.553 |
| unicode / inline / steady | 1.109/1.313 | 2.008/2.553 | 1.896/2.176 | 3.081/3.408 |
| unicode / inline / burst | 1.226/1.342 | 1.995/2.566 | 2.045/2.273 | 3.010/3.631 |
| unicode / display / steady | 1.236/1.474 | 3.938/5.550 | 2.393/2.610 | 5.445/7.221 |
| unicode / display / burst | 1.339/1.502 | 3.979/5.565 | 2.499/2.667 | 5.511/7.018 |
| unicode / equation / steady | 1.651/2.068 | 4.358/5.722 | 2.593/3.269 | 5.859/7.348 |
| unicode / equation / burst | 1.885/2.093 | 4.156/5.968 | 3.065/3.295 | 5.399/7.553 |
| unicode / align / steady | 4.494/5.508 | 8.939/10.997 | 5.923/7.037 | 10.492/12.757 |
| unicode / align / burst | 3.909/5.158 | 9.270/9.820 | 5.106/6.598 | 10.880/11.636 |

`Reason::DisplayMath` 在当前 `eligibility.rs` 只有声明和测试中的占位引用，判定流程没有使用它。
这不意味着 display 全部回退：四种模式在两种字体/两个后端的最终 640 次击键均为 fast + ok。
display/equation/align 实际包含在可重排单元里，align 的结构处理开销明显较高。

数字只描述 1 页/12 公式、宿主 JSON 转发与常驻栅格器，不含纹理上传或真实窗口。未扩大到 E6 的 10/100 页，
20 样本的尾部统计也不足以替代产品 SLO。M5 判 Conditional，未以微型数字覆盖 ADR 0043 的窗口门禁。
最终 Rust 元数据 encoder 的往返修复只在 FFI 的 `to_binary` 出口使用，JSON serve 的热路径不调用它；
M5 热路径代码未变，最终构建另复测 verify 与故障恢复。补丁格式化也不改变测量逻辑。

## M6：事务、错误页面与输入保留

分式、脚标、根号、矩阵行的插入以完整事务产生合法源码；各阶段独立完整公式合并为每种字体一份 17 状态夹具，
LuaLaTeX 均 exit 0。命令名逐字符输入在宿主缓冲，仅完成 alpha 后送给引擎；al 没有送入。
这是构造器与编译有效性的证据，不是新结构光标恢复已通过。

另故意绕过宿主缓冲，分别发送未闭合花括号和未定义命令，随后在非法状态里继续输入 q，删去非法前缀。
两种字体×两个后端×两个错误，共 8 次。结果均继续接受 edit，修复后实时出现 q；用 `export_pdf` 导出的
`export-src/main.tex` 与预期 qx 源码逐字节相同，删除 q 后也与初始源码逐字节相同（16/16 比较）。
8 次修复到 live 为 1.619–3.312 ms；每次的完整导出有独立 PdfExported/Ok，不能拿 live 时延代表整编时延。

但是错误阶段返回 Diagnostics 和 `LayoutUpdate compile=CompiledWithErrors`，在这些阶段发布了新页面。
A 的未闭合花括号试验也发布了错误页；基线首个未闭合状态有时 pages_changed=0，继续键入 q 后同样发布错误页。
所以原样引擎不保证“错误期间保持最后良好页面”，宿主必须拒绝错误布局替换，并保留与 revision 匹配的旧画面。
没有声称引擎崩溃、CPU 超时或任意宏恢复也通过；那些仍属于 E8/L1。

## 测量驱动纠正与证据范围

- 初次 observer 调用了当前 LuaTeX 没有的 getattribute 拼写，改为与上游相同的别名回退。
  [debug 日志](evidence/rtex-math-edit/debug.log)保留失败；后续 M1/布局均重新运行。
- 初次基线 display 的字母 a 错落在 frac 命令名内。修正为分子核之后，用相同夹具重跑所有字体/模式，
  并在测试中断言每个击键目标都是 x/a。初次含无效目标/冷栅格的统计只留作
  [exploratory-summary](evidence/rtex-math-edit/exploratory-summary.json)，不进入最终表。
- 初次恢复探针误用了不存在的 export 命令，随后又把后台 `src` 快照误作导出快照。
  [旧恢复探针](evidence/rtex-math-edit/recovery-probe-summary.json)不是输入丢失的证据；
  最终以正确的 export_pdf 和 export-src 重跑 8 次，事实见 recovery-summary。
- 测量后为 B 加入根号/矩阵装饰与 OpenType MATH 解码，并对保存的**未补丁**页面重新评分；
  这些修正不修改 TeX 或时延样本。最终评分可用 analyse.py 重算。

## 验证、范围与后续建议

- `python3 scripts/check-doc-links.py`：Pass。
- `python3 spikes/rtex-math-edit/test_driver.py`：Pass，4 个测量合同测试。
- `python3 spikes/rtex-math-edit/apply.py --build`：Pass，补丁内容与冻结基线重建一致，外部 release 构建通过。
- 外部 `cargo fmt --manifest-path ~/.cache/scholium-eval/rtex-math-edit/Cargo.toml --all -- --check`：Pass。
- 外部 `cargo test --locked --manifest-path ~/.cache/scholium-eval/rtex-math-edit/Cargo.toml -p rtex-dl`：Pass，4 个测试，
  包含新增的 ownership 二进制及 JSON 往返；见 [测试记录](evidence/rtex-math-edit/rtex-dl-tests.log)。
- 根 workspace 的 Rust、cargo-deny、构建与 window-checks 未运行：T22 明确只在新增 workspace Rust 时要求，
  本轮没有改主 workspace、生产模块、UI 或沙箱配置；没有启动桌面窗口，也没有声称窗口验收通过。
- 只修改新 spike、本日志及其证据目录；没有改 crates、ADR、计划、项目提案或共享索引；没有上游公开操作。

协调者若继续 L3，需先形成可验收的三个增量：每请求/revision 的来源表更新与源码偏移转换；
不改变完整编译输出的空 noad/slot 几何出口；错误编译结果的宿主显示门和稳定结构光标协议。
应先用本轮的空分式与新分式反例作为失败门，再扩大宏集合和窗口测试。
本轮可支持的正面结论仅为：**非空、冻结结构上的精确字形归属与有限光标操作可行；完整页面内结构编辑尚不可行。**
