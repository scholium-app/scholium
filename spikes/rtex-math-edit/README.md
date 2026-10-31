# rtex 页面内数学编辑评估（T22 / E9）

这是评估驱动，不是产品编辑器。结论与 M1–M7 数字见
[开发记录](../../docs/log/2026-10-31-rtex-math-edit.md)，原始结果见
[summary.json](../../docs/log/evidence/rtex-math-edit/summary.json)。本轮完整目标 **Fail**：
标签不改变输出，非空公式字形可映射，但空槽、结构插入后的新来源 ID 和错误布局保留未通过。

## 前提

- Linux、bubblewrap、可用的 `systemd-run --user`、系统 TeX Live 2026（LuaLaTeX、ctex、amsmath、
  amssymb、unicode-math、Latin Modern Math、Fandol）、poppler 的 `pdftoppm`。
- Python 3 与系统 `fontTools`（本机 4.68.0，读取 OpenType cmap/MATH 表，不拷贝字体或上游实现）。
- 原 E1–E8 克隆 `~/.cache/scholium-eval/rtex-v0.0.5` 已在冻结 SHA 构建；本驱动只读它。
- E4 栅格原型：`spikes/rtex-eval/raster/target/release/rtex-eval-raster`。
- 根 Rust workspace 不增加成员或依赖。补丁中的 Rust 只在外部 rtex workspace 构建。

## 完整复现（仓库根目录）

```bash
# 仅操作独立的 ~/.cache/scholium-eval/rtex-math-edit。
# 目录不存在时从上游克隆，锁定 e0ad11765ec016846b6e07619081f0528621636b。
python3 spikes/rtex-math-edit/apply.py --build
cargo build --release --locked --manifest-path spikes/rtex-eval/raster/Cargo.toml
python3 spikes/rtex-math-edit/test_driver.py
cargo fmt --manifest-path ~/.cache/scholium-eval/rtex-math-edit/Cargo.toml --all -- --check
cargo test --locked --manifest-path ~/.cache/scholium-eval/rtex-math-edit/Cargo.toml -p rtex-dl

# 每个 --run-id 必须是尚不存在的目录；不删除旧结果或 target。
python3 spikes/rtex-math-edit/evaluate.py --run-id replay \
  --evidence docs/log/evidence/rtex-math-edit --live
python3 spikes/rtex-math-edit/recovery.py --run-id replay-recovery \
  --evidence docs/log/evidence/rtex-math-edit
python3 spikes/rtex-math-edit/structural_edit.py --run-id replay-structural \
  --evidence docs/log/evidence/rtex-math-edit
python3 spikes/rtex-math-edit/analyse.py --evidence docs/log/evidence/rtex-math-edit
python3 spikes/rtex-math-edit/provenance.py --evidence docs/log/evidence/rtex-math-edit
python3 scripts/check-doc-links.py
```

`apply.py` 拒绝错误的 HEAD、额外修改和不匹配的文件内容；通过临时 Git index 从基线与补丁重建 blob，
逐文件验证工作副本。不会 reset/clean 外部克隆，不操作原 E1–E8 克隆。重复运行是幂等的。
测试运行前单进程在既有 `rtex` 沙箱配置内预热字体缓存，随后会话复用本 spike 的缓存目录。
这里只有可信原创夹具；产品必须使用 ADR 0043 的每会话缓存副本，不能照搬本实验的共享可写缓存。

## 文件与测量

| 文件 | 职责 |
|---|---|
| `fixtures/cases.json`、`fixtures/cm.tex`、`fixtures/unicode.tex` | 12 个原创公式，各有 CM/TFM 与 Latin Modern Math 版本；后两者是未标记源码 |
| `source.py` | 限定宏集合解析、UTF-8 字节范围、原源码到带属性的实验副本；未知宏拒绝处理 |
| `patches/0001-observe-math-source-ownership.patch` | 观察转换前 noad 与输出字形；附加 `math_edit` 元数据，支持 Lua→Rust 与 JSON/二进制往返 |
| `patches/0002-allow-generated-attribute-tags.patch` | 允许实验生成的 `\attribute` 通过数学及 `\text` 白名单；不是产品安全策略 |
| `alignment.py` | B：读系统 `texmital/texmsym/texmext.enc`、OpenType cmap/MATH 变体表，以序列匹配对齐真实未补丁字形流 |
| `caret.py` | 点击左右半字形、源码结构顺序的 ←/→、num/den 与 sub/sup 的 ↑/↓ |
| `runtime.py` | 复用 E6 的有界 JSON 行会话和常驻栅格器；既有沙箱配置完全不变 |
| `evaluate.py` | M1 完整编译与 verify、M2/3/4、M5 稳定/突发击键、M6 结构状态、M7 中文 |
| `recovery.py` | 在非法状态继续输入 q，修复后核对实时字形与导出源码字节；检查实际 `export-src` 快照 |
| `structural_edit.py` | 实测 x→分式后来源表过期导致错误光标的反例 |
| `analyse.py` | 用 A 作评分真值/裁剪范围，对独立未补丁字形流评分 B，汇合最终故障证据 |
| `test_driver.py` | 测量目标、UTF-8、语法拒绝与装饰字形序列的合同检查 |

M1 的 verify 同时覆盖 tagged/untagged 的实时→捕获、页面→PDF、捕获 PDF→干净 PDF；另将两个完整编译
的单页 PDF 在 300 dpi 栅格化，比较整个 PGM 的 SHA-256。PDF 时间戳/ID 不用于等同性判断。

M2 分母为公式产生的全部字形（包括根号、矩阵括号与求和号，不含公式编号和正文）。A 标签和字体解码逐项核对；
B 输入来自未补丁的页面，按相同字形位置配对裁剪，之后剥离来源 ID。B 使用已知公式裁剪，是有利条件，
不声称自动定位公式也通过。结构宏的装饰字形归属该宏范围，矩阵两个括号归属同一个 matrix 节点。

M3 的节点框是其后代实际字形框的并集（sp，y 向下）。没有以占位字形改变空槽的输出，也没有伪造空槽框：
六个空槽没有几何，明确记 Fail。原始 noad 观察附在节点上；语义种类和源码范围来自限定宿主解析，
不是宣称引擎已输出独立、完整、能恢复所有空位置的数学语义树。

M4 规定水平方向按解析后的源码结构顺序遍历，区别于 TeX 的绘制顺序；包括宏 token 两端、脚标、
根内表达式、围栏、矩阵 cell 边界。空 cell 无法点击；矩阵上下导航与任意宏展开不在这个小模型中。
同字节替换 x→z 通过，但新分式插入后没有刷新来源表，新分子/分母无法定位，反例单独保留。

M5 每种字体、每个后端、每种公式模式、每种节奏各 20 次，合计 640 次。先用两个不计时的插入/删除预热栅格器。
稳定输入在上次更新后停 150 ms；突发每次停 30 ms、每 5 次停 1.5 s。每次等待自己的更新，
不是不等待回复的输入洪峰。计时为宿主单调时钟的写命令→更新→300 dpi 单元栅格完成；不含窗口呈现。
该数字来自 1 页/12 公式的微型夹具，不外推长文档。20 个样本的 p95 为最近秩统计，保留全部原始样本。

M6 的 17 个完整状态在每种字体下合并编译（四类结构事务与最终 `\alpha`）。`\al` 等前缀只留在宿主；
这与故意绕过缓冲发送非法状态是两组实验。后者按错误布局事件、修复更新及导出快照判断，不以“进程没死”代替通过。

## 限制

- 原始源码是权威；带标记副本只作实验。`src_range` 指向未标记版本，尚无增量位置转换/修订号合同。
- 来源表在导言区注册，结构插入后会过期。时延中的重复插删只测布局耗时，不声称每次插入后的新 atom ID 都正确。
- 为保持 `Item` 的渲染分支不变，用 display list 的附加 `math_edit` 表表达节点和 Glyph→source ID 归属，
  未增加 `Item::MathNode` 枚举分支；外部 JSON lines 通道不变。
- 空槽几何、任意插入后的稳定 ID、完整结构光标和错误布局保留仍是未通过项；本 spike 不把它们标记为完成。
- 不含窗口、IME、选择/删除、宏定义展开、跨页公式、生产沙箱或安全回归验收；不发布上游问题或 PR。
