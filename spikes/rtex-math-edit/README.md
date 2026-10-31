# rtex 页面内数学编辑评估（T22 / T22b）

这是评估驱动，不是产品编辑器。[开发记录](../../docs/log/2026-10-31-rtex-math-edit.md)保留两轮结果。
第二轮确认第一轮三个阻塞来自驱动：冻结来源表、只取字形并集、没有显示门。
修复后引擎能力判为**有条件可行**；驱动仍有Unicode 空 degree 缺口，完整验收仍 Fail。

## 前提与复现

Linux、bubblewrap、systemd user scope、TeX Live 2026（ctex/amsmath/amssymb/unicode-math/Latin Modern Math）、
Poppler、Python 3/fontTools/Pillow。既有 E1–E8 克隆 `~/.cache/scholium-eval/rtex-v0.0.5` 只读；
独立补丁克隆 `~/.cache/scholium-eval/rtex-math-edit` 固定在
`e0ad11765ec016846b6e07619081f0528621636b`。不增加根 workspace 依赖，不改沙箱。

在仓库根执行；每个 run-id 必须是新目录，evidence 应使用新目录，不覆盖历史：

```bash
python3 spikes/rtex-math-edit/apply.py --build
cargo build --release --locked --manifest-path spikes/rtex-eval/raster/Cargo.toml
python3 spikes/rtex-math-edit/test_driver.py
python3 spikes/rtex-math-edit/test_round2.py
cargo fmt --manifest-path ~/.cache/scholium-eval/rtex-math-edit/Cargo.toml --all -- --check
cargo test --locked --manifest-path ~/.cache/scholium-eval/rtex-math-edit/Cargo.toml -p rtex-dl
python3 spikes/rtex-math-edit/round2.py --run-id replay-r2-identity --evidence /tmp/t22b-new/identity --phase identity
python3 spikes/rtex-math-edit/round2.py --run-id replay-r2-structural --evidence /tmp/t22b-new/structural --phase structural
python3 spikes/rtex-math-edit/round2.py --run-id replay-r2-gate --evidence /tmp/t22b-new/gate --phase gate
python3 spikes/rtex-math-edit/round2.py --run-id replay-r2-spot --evidence /tmp/t22b-new/spot --phase latency
python3 spikes/rtex-math-edit/provenance.py --evidence /tmp/t22b-new
python3 scripts/check-doc-links.py
```

`apply.py` 在临时 index 顺序重建三份补丁，再核对现存文件哈希/路径；不 reset/clean 外部克隆。
本地已有克隆已用该方法核对。字体缓存沿用评估目录；产品仍须 ADR 0043 的每会话缓存副本。
第一轮复现必须使用提交 `d339f40f518d678f494a894c2e98ab5c3a97ecd1` 的驱动和两个补丁，
其证据不变；旧 `evaluate.py/recovery.py/structural_edit.py/analyse.py` 是历史驱动，不能混用新 metadata schema。

## 第二轮方法

- `revision.py` 选择 per-revision atom id，宿主持有 `(unit, revision)` 来源树。宿主为新节点分配不重复 ID，
  每次编辑重解析并替换整个 tagged unit；保存/光标/编辑始终用 untagged UTF-8 byte，插入标签和等价脚本括号
  有双向 byte map。公式两种属性均复位。未知宏拒绝解析。
- `0003` 移除引擎 preamble registry，输出原始属性值、glyph/packed-field box/Math boundary；语义树留在 host。
  原生 packed empty box 的属性受转换时公式 reset 影响；故只在真正空 sub_mlist 中插入零尺寸 user_defined marker。
  不向非空字段前置 marker，避免改变 clean_box 的 italic correction。
- `geometry2.py` 按当前来源树 materialize；空矩阵格、空公式用 Math-on 坐标，CM 空 degree 用内部 Math-on。
  host 最小 hit width 2pt、高 6pt；`overlay.py` 在 PDF 栅格副本画绿色轮廓，PDF 保持不变。
  Unicode 空 degree marker 仍未到达输出，按 Fail 记录。嵌套结构字段标签覆盖问题已由回归测试修正。
  transient phantom 可在空槽放入临时不可见占位，但会改变宽高/间距与 identity，未实现。
- `display_gate.py` 只接受当前 revision 且 `ParagraphUpdate.status=ok` 或 `LayoutUpdate.compile.state=Ok`。
  raster 和 geometry 作为一个 shown 对象替换，dirty/stale 不调用 raster。fingerprint 是渲染输入与 geometry 的哈希，
  不是像素哈希；错误期间以 shown 对象身份完全相同证明保留旧 raster；gate 另保存不可变 PGM 实际字节并记录其 SHA-256。M1 独立使用全页像素哈希。
- `round2.py` 的 14 公式 M1 同时跑四次 verify、tag/plain 全编译全页 300dpi 像素与 glyph/rule exact 比较。
  34 次真实结构替换/字体，逐步检查点击/左右/num-den 与 sub-sup 上下、空槽可用性、map、host plain save，
  再导出 rtex tagged buffer 核对。plain save 是 host 的权威输出，不冒充 rtex 自动剥除标签。
- `round2_live.py` 重跑原 8 次非法试验（字体×补丁/基线×两错误），非法期间继续输入 q，记录显示事件、诊断、
  真正常驻 300dpi raster、修复和导出。另四模式各 20 个 spot 样本/字体，包含整单元重标记耗时；不包含窗口。

最终证据在 [第二轮 M1](../../docs/log/evidence/rtex-math-edit/round2/accepted/identity-summary.json)、
[M4](../../docs/log/evidence/rtex-math-edit/round2/accepted/structural-summary.json)、
[M5](../../docs/log/evidence/rtex-math-edit/round2/accepted/latency-summary.json)、
[M6](../../docs/log/evidence/rtex-math-edit/round2/bitmaps/gate-summary.json)。
`round2/final` 和根部探针保留了中间诊断（包括早期 gate revision 绑定错误），不是最终验收。

## 范围

限定宏与单页原创夹具；没有任意宏展开、选择/删除完整语义、矩阵上下导航、跨页公式、长文档、IME、真实窗口、
生产沙箱安全验收。M2/B 和 M7 沿用第一轮冻结测量；本轮没有重测 host-only B 对齐。
根 Rust/window gates 按 T22 不适用；外部 rtex Rust 只跑自己的 workspace 检查。不合并，不发上游 PR/issue。
