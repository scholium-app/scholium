# Spike 0034：源码组件行内排版与内部引用修复

- 日期：2026-09-19；基线 `427c73c` 加本轮修复。
- 结论：**Pass（同语言源码 include 范围）**；条件 8 仍部分满足。

## 发现与修复

同语言 include 组件的符号原先被当成异语言组件：宿主引用生成 `comp:<组件>` 锚点，
但源码 include 没有这个矢量组件锚点；组件反向引用宿主又退化为文本数字。
新增真实 rebuild 测试先复现缺失链接，再修复为宿主与 include 共用引擎原生标签空间。
同时从宿主最终编译收集 include 符号的编号/页码，避免只收集宿主自有符号。
矢量组件仍沿用独立编译与组件锚点，未扩大其内部语义承诺。

另发现 Inline + Include 未约束实际块类型。新增 `inline-content-unsupported`，仅允许
单行文本、行内公式与引用；标题、分页、独立公式、原始源码及未验证宏调用等拒绝。
跨语言 Inline + Vector 仍以 `inline-embed-unsupported` 明确拒绝。

## 有区分度的验证

两种宿主通过真实 `--rebuild` 编译同一结构夹具，产物检查运行在 PDF 沙箱：

- 第 1 页宿主符号引用第 2 页组件内部符号，第 2 页反向引用第 1 页宿主；逐页检查 PDF 链接目标。
- `INLINELEFT`、源码组件中的行内公式 x、`INLINERIGHT`：检查两侧文字同一 top、公式 x 在两者
  横坐标之间且纵向框重叠，避免仅源码中有 `$x$` 就宣布行内成功。
- PDF 中不得出现未解析的 `??`。人工核对双宿主第一页，没有额外换行。
- 两条计划层测试覆盖块级内容拒绝与跨语言行内矢量继续拒绝；已加入常规 items 分段。

[LaTeX PDF](evidence/SPK-0034/Latex/final.pdf)、[Typst PDF](evidence/SPK-0034/Typst/final.pdf)、
[LaTeX 图](evidence/SPK-0034/Latex/page1.png)、[Typst 图](evidence/SPK-0034/Typst/page1.png)与 links.xml 保留。
两种引擎字体/尺寸设置本就不同，不把这些图片作像素一致性证明。

失败过程保留：`before.log` 为真实链接失败，`inline-before.log` 为缺少行内类型诊断；
`inline-links.log` / `final-links.log` 是取证测试先未处理 Poppler 的斜体标签、再未处理数学 Unicode 𝑥
导致的失败。修正只涉及证据读取，不改变实际公式排版。

本轮混合构建 **36/36** 回归、双宿主新集成场景、2 条门禁测试、全 targets Clippy、脚本自测通过。
未重跑完整阶段套件或 28 个导出包；日志见 [evidence/SPK-0034](evidence/SPK-0034/)。

## 保留边界

这是同语言源码组件验证，不是跨语言矢量载体的行内 baseline/depth 协议。
链接验证到最终 PDF 的正确页，未测同页多个目的地的精确坐标、嵌入矢量内部注释重建或无障碍语义。
任意原始模板、嵌套 include、所有数学尺寸/换行组合仍未覆盖；不提升阶段出口。

## 复现

```sh
CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" cargo test --release --offline \
  --manifest-path spikes/mixed-build/Cargo.toml --test include_links -- --nocapture
CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" cargo test --release --offline \
  --manifest-path spikes/mixed-build/Cargo.toml --bin scholium-spike-mixed-build fidelity_tests
```
