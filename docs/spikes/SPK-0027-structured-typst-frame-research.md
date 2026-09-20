# SPK-0027：结构化数学排版后端调研

- 日期：2026-09-18
- 状态：进行中，尚未替换左侧编辑器
- 目的：确认现有自绘数学布局是否应由成熟排版库替代，并验证渲染几何能否继续服务编辑交互

## 结论

当前最可行的路线是复用项目已经锁定的 Typst 0.15.1 **结构化 Frame**，而不是继续调节左侧近似常数，也不是把整页 PNG 覆盖到编辑器上。Typst 的公开产物同时包含：

- 递归 `Frame` / `GroupItem`，可携带数学分数、根号、上下标、矩阵产生的线条和子框；
- `TextItem` 的 shaped glyph、advance、offset、字符 range 与 source span；
- 数学字体 MATH 常量驱动的布局（fraction、radical、scripts、fenced、table 等模块）。

这允许同一份排版结果用于绘制和命中测试。现阶段仍需实现“Frame → egui 绘制场景”和源 span → SDG 节点/字符的完整映射，不能把本探针误报为已完成的编辑器集成。

## 实测证据

在 `spikes/typst-mapping` 增加 `frame` 探针，对标准夹具使用当前 Typst 0.15.1 编译：

```text
Frame 结构：1 页，1 个 group，13 个文本 run，19 个 glyph，2 个 shape，52 个 tag
文本 source range：19 个 glyph 带 range，19 个 glyph 带非空 source span
```

同一 spike 的既有复测结果：

- `charmap`：37 个字符，块内宽度插值平均误差 0.24 pt、最大 0.85 pt；反查 36/38 完全一致。
- `render`：锚点区间墨迹存在性 0 个违例，结构顺序检查 0 个违例。

这些结果证明了“排版几何可读出”和“锚点可验证”，不证明字符级光标已经覆盖 ligature、跨行、嵌套数学和空槽位。

## 候选库

| 候选 | 能力 | 许可证/成熟度 | 判定 |
|---|---|---|---|
| Typst 0.15.1 | 完整数学布局、MATH 字体常量、Frame/glyph 几何、现有生成器与缓存 | MIT/Apache 兼容；项目已锁定并有隔离 worker | **首选**，先做结构化 Frame 适配层 |
| ReX (`cbreeden/rex`) | Rust 数学到 SVG，MIT/Apache/BSD | README 明确写着 heavy development、仅用于测试调试；与现有 Typst 映射和字体资源不兼容 | 不作为生产后端；可保留为许可证兼容的参考实现 |
| cosmic-text / Parley / rustybuzz | 文本 shaping、字体 fallback、glyph advance | 宽松许可证，成熟度较好 | 不能替代数学布局；可在原生文本层使用，但不会解决分数/根式/可伸缩括号 |

## 约束与风险

1. Typst 的 `TextItem` glyph range 是 UTF-8 文本片段，glyph 不保证与字符一一对应；映射必须按 cluster/range 处理，不能按 glyph 下标猜字符。
2. Frame 坐标包含 group transform、分页和行内 frame；实现适配层时必须递归累积变换，并保留 page 坐标系单位（pt）。
3. 左侧编辑模型包含空数学槽位，而 Typst 生成结果没有这些“不可见可编辑槽位”；命中和光标仍需由 SDG 槽位映射补齐。
4. 夹具中 `mat(1, 2, 3)` 与用户截图所期待的矩阵语义不一致，且 `$ ... $` 的空格会影响 Typst 的行内/块级行为。视觉比较必须先修正生成语义和受控字体，不能拿错误夹具验收排版。

## 下一步

实现一个隔离的 Frame scene spike：递归输出 page-space 的 Text/Shape/Group 项，使用同一 scene 做绘制和点命中；先覆盖分数、根号、上下标、矩阵及 glyph span，再决定是否替换左侧 painter。现有 native fallback、caret 和编辑路径在该验证通过前保留。

参考：

- [Typst Frame API](https://docs.rs/typst/0.15.1/typst/layout/struct.Frame.html)
- [Typst TextItem/Glyph API](https://docs.rs/typst/0.15.1/typst/layout/struct.TextItem.html)
- [Typst 数学布局源码](https://github.com/typst/typst/tree/main/crates/typst-layout/src/math)
- [ReX README](https://github.com/cbreeden/rex)

## 2026-09-18：Frame scene 隔离实验

限定实验结论 **Pass**；替换左侧编辑器的完整验收仍 **Fail（未实现）**。
平台为本机 Linux，锁定 Typst 0.15.1、egui 0.36.2。未改变阶段出口判定。

新增 `scene.rs`、`scene_probe.rs`、`scene_view.rs`：保留 Typst glyph 和 Shape 绘制项目，
保留 group 层级以保证栅格化一致；独立递归合成 `ancestor * translation * group transform`
用于命中。字形盒来自实际字体轮廓的包围盒，包含 glyph offset；光标 advance 不当作墨迹宽度。
命中通过逆变换判断局部墨迹盒，空白不退化为最近节点。遇到裁剪组、图像、链接明确拒绝。

验证夹具明确区分行内和块级公式，矩阵为 `mat(1, 2; 3, 4)`，包含嵌套根式及旋转公式；
使用内置 Libertinus Serif / New Computer Modern Math，编译警告会使探针失败。
它是排版诊断夹具，不是已修复 SDG 生成器的证明。

实测结果：

- 720 × 481 图像，346320 像素，与原始 Typst frame 的渲染相比差异 **0**。
- 提取 49 个非空字形墨迹盒，14 个公式字母中心命中对应源码 span，页边空白无命中。
- egui 离屏 Context 提交页面纹理，在 0.75 / 1 / 2 倍缩放和非零滚动偏移下校验命中映射。
- 1 项嵌套平移/旋转及框外拒绝的单元测试通过。

首次展平所有组时出现 375 个差异像素；保留绘制分组后为 0。首次直接假定 Typst bbox 的
min/max 在 Y 轴已排序，导致无字形盒；归一化后修正。没有放宽像素门槛。

复现：

```sh
CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" cargo run --release --offline --manifest-path spikes/typst-mapping/Cargo.toml -- scene
CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" cargo test --release --offline --manifest-path spikes/typst-mapping/Cargo.toml scene::tests
```

证据：[原图](evidence/SPK-0027/scene/reference-0.png)、[场景重绘](evidence/SPK-0027/scene/scene-0.png)、
[明确语义的夹具](evidence/SPK-0027/scene/fixture.typ)。

边界：egui 接的是由 scene 重建 frame 后经 typst-render 生成的页面纹理，并非 egui 自行重塑形。
图像和命中几何同源，但当前没有真实窗口点击、SDG NodeId 映射、字素/合字内部 caret、空槽位、
Shape 所属节点命中、选区和 IME；本次也没有新接隔离 worker。这些是替换左侧之前必须完成的工作。
现有字符 chunk 插值不因本实验而获得精确性背书。
