# Spike 0035：跨语言矢量基线、链接注释与精确目的地

- 日期：2026-09-19；基线 `196b7db` 加本轮修复。
- 结论：**Pass（下述载体与语义子集）**。
- 平台：Linux / Arch，XeLaTeX TeX Live 2026、Typst 0.15.1、Poppler 26.08.0。
- 新依赖：lopdf 0.38.0，MIT，纯 Rust，默认特性关闭；决策见 [ADR 0018](../adr/ADR-0018-vector-fidelity-bridge.md)。

## 实现与支持矩阵

| 桥接 | 内容与行为 | 不支持时 |
|---|---|---|
| Include | 同语言源码、引擎原生标签 | 异语言拒绝 |
| Convert | 显式转换已有 IR 的段落、数学、引用、标题、分页、表格；行内仅文本/行内数学/引用 | Raw、宏、嵌套组件等拒绝，不伪称外语像素保真 |
| Inline Vector | 两种源引擎独立编译，紧致单行 PDF + 测得的 descent，宿主保持源尺寸和基线 | 块级内容/宏拒绝 |
| Block Vector | 源引擎 PDF 原样绘制；覆盖层按同一缩放和平移重建 Link 矩形、XYZ 目标、HTTP(S) 与跨组件引用 | 非链接批注/表单/附件、活动动作、未知 URL、跨页、旋转/裁剪、重复/嵌套放置拒绝 |

行内 Vector 修正了上一轮“声明 Vector、实际转换成宿主公式”的问题。Convert 与 Vector 的选择现在显式。
通过独立沙箱 worker 提取 PDF 几何；生成的 `.tex`/`.typ` 覆盖层可随标准目标包在干净环境重建。
PDF 底部原点与 Typst 顶部原点、TeX pt 与 PDF bp 均显式换算。

## 有区分度的验证

- 双宿主单行 x 与引用，分别覆盖 Convert、Vector；检查最终 PDF 两侧文字与公式同行。
- 双宿主比较简单 g 与带根号、上标、嵌套分数、下标分母的 g：同源字体基线误差不超过
  Poppler 取整的 1px；深公式测得的下沉量增加，紧致页高超过本夹具必需的 20bp。
- 最终 PDF 截图人工检查：根号、上标和最底层分母完整显示。第一次 Typst 默认行高曾裁掉上下内容，
  已改为字形 bounds，并保留修复前截图。
- 同一组件内两个分离目标；宿主引用、组件内部引用及组件回指宿主；最终 PDF 目标坐标按源 PDF 的
  缩放/平移核对（0.3bp 容差），点击矩形核对（1.2bp，含工具链取整），外部 URL 保留。
- XeLaTeX 空透明 hyperref 框丢失注释、Typst PDF 目标重复上移 10pt 都曾被矩形验证抓出，已修复。
- Poppler 独立验证外部链接矩形实际覆盖源组件中的 `EXTERNAL` 文字，避免仅覆盖层自身坐标相互一致的假通过。
- 合成 PDF 正负对照覆盖 HTTPS、未知/本地/JavaScript URI、Launch，以及 Text/Widget/FileAttachment/
  RichMedia 注释拒绝；畸形 PDF 拒绝。声明 Convert 后的 Raw 不可静默接收。
  无编号的行内公式不能作为编号/页码引用目标，明确拒绝而不输出问号。

证据归档：[evidence/SPK-0035](evidence/SPK-0035/)。测试、完整混合夹具、干净导出包、Clippy、许可结果
为 **23 个测试通过、36/36 混合夹具、28/28 干净包重建**；Clippy、格式、许可/来源及脚本自测通过。
原始结果以目录内日志为准。一次完整夹具运行期间并行重编译替换了正在运行的 driver，导致 current_exe 无法复制；
该失败日志保留，最终回归使用固定二进制顺序运行，不混充产品编译错误。
首次整 crate 格式检查发现旧文件及增量格式差异；最终统一运行 rustfmt，并抽出构建初始状态初始化，
避免格式化导致旧构建函数超出 Clippy 行数上限。最终格式与 Clippy 门禁通过，初次失败日志保留。

## 明确保留的范围

这不是任意源码解析器、任意 PDF 导入器或 PDF/UA 支持。Raw 在 Vector 内可交由源引擎执行，
但生成 PDF 必须满足载体/注释白名单。任意跨页外语流式正文、所有数学宏、PDF 批注编辑、结构标签、
完整读屏和可访问数学不在本次 Pass 中。不能把文本抽取成功当成阅读顺序或无障碍通过。

## 复现

```sh
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"
cargo test --release --offline --manifest-path spikes/mixed-build/Cargo.toml
cargo clippy --offline --manifest-path spikes/mixed-build/Cargo.toml --all-targets -- -D warnings
spikes/mixed-build/target/release/scholium-spike-mixed-build
spikes/mixed-build/target/release/scholium-spike-mixed-build --packages
```

编译完成后再顺序执行后两条，避免替换运行中的可执行文件。实验 snapshot 中新增 `Convert` 枚举；
未作正式格式兼容性承诺。旧的 Inline + Vector 现在实际走源引擎载体，不再默默转换。
