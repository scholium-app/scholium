# 报告 0047：结构坐标改造独立验证（E1 / S2）

- 结论：**Pass（S2 范围）**，附 1 项移交 S4 的范围缺口。
- 对应：[E1 实施计划](../plan/E1_STRUCTURAL_CURSOR.md) S2、[ADR 0031](../adr/ADR-0031-structural-cursor.md)、
  [报告 0046](SPK-0046-editor-usability-audit.md)。本报告不改变阶段出口判定。
- 日期：2026-09-27。
- 执行者：coord-verifier（**独立于实现者 cursor-impl 与 Lead**）。
- 关联 ADR：[ADR 0031](../adr/ADR-0031-structural-cursor.md)（双向链接）。

**结论：实现者的全部声明经独立复核成立；0 项缺陷、0 项判据篡改。**

核验基线：`59eca0c`（报告 0046 的原始证据提交）。
工作树：HEAD 为 S2 未提交改动（`git status --short` 见附录）。
验证者未修改任何实现或测试代码；`caret.rs` 的 sha256 在核验前后一致
（`b3e19b62…2183407a`）。

原始日志见 [证据目录](evidence/SPK-0047/)。

---

## 1. 逐项结论

| # | 核验项 | 结论 | 证据 |
|---|---|---|---|
| 1 | 字节偏移不再是光标权威 | **PASS** | 见 §2 |
| 2 | 审计判据未被篡改 | **PASS** | 见 §3 |
| 3 | `caret.rs` 投影对抗性验证 | **PASS** | 见 §4（24 项自建用例全绿） |
| 4 | 跨块 `DeleteRange` 原子性 | **PASS** | 见 §5 |
| 5 | 门禁独立复跑 | **PASS** | 见 §6（format/size/clippy/rustdoc 全 0） |
| 6 | 契约数与零回归 | **PASS** | 见 §7（13→16，零回归，独立复算） |
| 7 | 原生窗口 5/8 零回归 | **PASS** | 见 §8 |
| 8 | b13 常规回归测试 | **缺口（不属 S2）** | 见 §9 |

---

## 2. 编辑请求是否全部由结构 caret 生成

**PASS。** 全仓库只有一条构造编辑请求的路径：

```
crates/scholium-app/src/page_editor/command.rs:112   snapshot.request(BlockEdit::ReplaceRange { .. })
  ↑ 唯一的 BlockEdit 构造点
crates/scholium-app/src/page_editor/input.rs:231     let Some(request) = evaluated.request(snapshot)
crates/scholium-app/src/page_editor/input.rs:254     state.pending_edit = Some(request)
crates/scholium-app/src/page_editor.rs:397/402       （工具栏入口，同一条路径）
```

`Evaluated::request`（`command.rs:109`）的字节来自 `position_of`，即
`caret → byte` 方向。因此**不存在"先拼字符串、再由解析器猜回结构"的写入路径**。

`EditorState`（`page_editor.rs:22`）不再持有任何 `usize` 全局字节偏移字段；
权威是 `selection: Selection`（`Caret` 三元组）。其余 `usize` 字段经逐项确认为合法：

- `page_editor.rs:139/140` `select_bytes(anchor: usize, caret: usize)` — `#[cfg(test)]` 仅测试用，
  内部先经 `from_global_byte` 转为结构 caret，不存字节。
- `state.rs:160` `FocusTarget::Block { caret: usize }` — 是**字符**偏移（`native_text.rs:277`
  用 `chars().count()` 夹取），不是字节光标。

字节偏移的其余用法经核查全部是只读派生视图，且**不驱动编辑**：

| 位置 | 用途 | 是否驱动编辑 |
|---|---|---|
| `input.rs:368` `selection_bytes` | 剪贴板复制 | 否（读） |
| `input.rs:397/426` | 方向键/Home/End 导航：字节 → `from_global_byte` → caret | 否（`key_event` 返回值只并入 `edited`，用于 `ensure_visible`） |
| `render.rs:65/174/187` | 命中测试与绘制 | 否 |
| `navigation.rs:33` | 目标页定位 | 否 |

`command.rs` 的跨块能力是**显式**的而非退化为整块：`Evaluated{first,last,start,end}`
两端点分属不同块，`replace_range` 的 `ordered()` 按（块索引，投影偏移）排序后
分别 `locate` 到首块与尾块。

---

## 3. 判据是否被篡改

**PASS，未发现任何篡改。** 改动仅限 3 个文件、`+27 −12`：

```
crates/scholium-app/src/session/usability_audit/editing.rs   |  4 ++--
crates/scholium-app/src/session/usability_audit/mod.rs       | 20 ++++++++++++++++--
crates/scholium-app/src/session/usability_audit/rendering.rs | 15 +++++++--------
```

独立复核方法（不止肉眼看 diff）：

1. **期望值/断言语义逐条程序化比对。** 用括号配平提取 baseline 与 HEAD 中每个
   `audit!` 用例体内所有的 `assert*!` 期望字面量，逐用例比对：
   结果 `total structural diffs in case bodies/expected values: 0`。
2. **用例名与数量。** baseline 与 HEAD 的 `audit!` 名称列表**完全一致**：
   `editing.rs` 18 项、`rendering.rs` 13 项、`performance.rs` 1 项，共 32，
   顺序亦一致（`identical_names=True`）。
3. **`#[ignore]` 未增删。** `git diff -U0 … | grep '^[-+].*ignore'` 无输出；
   `audit!` 宏本身（`mod.rs` 的 `#[ignore = "Opt-in usability contract; …"]`）未改动。
4. **`painted_text` / `texts()` / `snapshot()` 未被放宽。** 对三个函数做花括号配平
   提取函数体后比对：`texts: identical=True`、`painted_text: identical=True`、
   `snapshot: identical=True`。
5. **机械改写验证。** 对 baseline 施加**唯一允许的**改写
   （`h.state.page_editor.caret` → `h.caret_byte()`）后与 HEAD 做 diff：
   `editing.rs` diff **为空**；`rendering.rs` 仅剩 3 处**纯格式重排**
   （`assert_eq!` 参数折行），语义不变。
6. `mod.rs` 的改动只有：`select()` 改为经 `select_bytes` 路由、
   新增 `caret_byte()` 取值器，以及相应文档注释。**无判据改动。**

---

## 4. `caret.rs` 投影对抗性验证

方法：将 `caret.rs` **逐字复制**到验证者独占目录
（`.verify/s2b/adversarial/`，仅把 `pub(crate)` 放宽为 `pub`），
用正则反向归一后与原文比对 `normalized identical: True`，确认副本忠实。
自建 **24 项**对抗用例，**全部通过**（另 3 项契约语义用例，见 §5）。
该目录在验证后**已删除**，未留在仓库。

| 攻击点 | 结果 | 说明 |
|---|---|---|
| 多字节字符中间 offset（`中` 的 1/2） | PASS | `clamp` 下取整到合法边界，不 panic、不切非法边界；`中x` 的 offset 1/2 → 0，offset 3 → 3 |
| 空块（`content` 为空） | PASS | 任意 `inline`/`offset` 均归一到 `(0,0)`；`projected_len_block == 0` |
| 零宽节点（`Text("")`/`Math("")`） | PASS | 全字节扫描均返回块内合法 caret，`offset` 不越界 |
| 共享边界 `Text("ab")` 紧邻 `Math("x")` | PASS | 见下 |
| `clamp` 幂等 | PASS | 7 组结构 × 全 offset 空间，`clamp(clamp(x)) == clamp(x)` 且三阶稳定 |
| **转义往返** `Text("$")` → `\$` | PASS | 见下 |
| `projected_len_block == markup().len()` | PASS | 含转义的三元穷举 500+ 组 + 混合节点四元穷举 4000+ 组，全部相等 |

**共享边界的实际语义（与文档承诺一致，非缺陷）。** `ab$x$` 中
`to_markup_byte` 对 `Text("ab")` 端点给 **2**、对 `Math("x")` 起点给 **3**：
因为节点内 caret 偏移**不含定界符**（`Math` 的 offset 0 在开 `$` 之后），
这与仓库既有用例 `math_caret_offsets_exclude_delimiters`
（`[Math("alpha")]` 的 `(0,0) → 1`）一致，是**设计**。
字节 2 是文本节点的真实末端而非共享字节，故 `from_markup_byte(2)` 解析到文本末端是**正确**的；
文档中"共享字节归后来的节点"仅适用于**零宽**节点，规则未被违反。
（我最初的两条断言把"节点起点"误当"内容起点"，是实现无误、**我的期望写错**，已修正后通过。）

**转义往返的实际行为符合文档承诺（不是双射）。**
文档承诺：`to_markup_byte` 精确、全定义、单调；`from_markup_byte` 仅保证
"返回块内合法 caret"。实测**完全符合**：

- `to_markup_byte` 对 15 组含 `$ * _ \` 的结构，全 offset 空间内
  值域 ≤ 投影长度、全程单调不减、且在 stored 字符边界上落在投影字符边界上。
- `from_markup_byte` 对每个投影字节都返回块内合法 caret（`inline ≤ len`、
  `offset ≤ 节点内容长度`），**无越界、无 panic**，符合"有损反函数"的承诺。
- 关键在于 **caret 不驻留偏移处**：编辑请求一律走 `caret → byte`，
  `from_markup_byte` 只服务命中测试，故非双射不影响正确性。
- `NodeId` 分支正确：`Text("$")` `delimiter_start == 0`、投影宽度 **2**（`\$`），
  `projected_len_block == markup().len()` 成立。

一处**澄清**（非缺陷）：`from_markup_byte` 在含转义文本的结构上
（如 `[Text("$") , Text("ab")]`）可能把落在 `\$` **前者**的字节解析到
**前一个**节点。原因是一旦该节点宽度非零即 `break`，
"更靠后的节点优先"只在零宽节点上生效。该行为仍在文档承诺范围内
（返回块内合法 caret），且 `caret → byte` 方向精确，编辑不受影响。

---

## 5. 跨块 `DeleteRange` 原子性（a12 / a18）

**PASS。** 未只看用例通过，而是直读 `scholium-document/src/range_edit.rs` 并自建用例实测：

- `range_edit::plan` 先做 `is_char_boundary` 与顺序校验，非法范围返回
  `InvalidRange`，**不进入** `commit`，因此不存在部分应用。
- 拼接语义为 `head[..start] + text + tail[end..]`，再按 `\n` 切块，
  故首块保留**前缀**、尾块保留**后缀**、中间块整体丢弃。
- 自建用例实测：
  - 跨块删除 `"甲乙"/"丙丁"/"戊己"/"保留尾"`，范围跨第 2、3 块 →
    结果 `["甲乙","丙己","保留尾"]`；**动作数 +1、revision +1**；
    未触及块（首块、末块）的 `NodeId` **逐一不变**。
  - 跨块替换保留首块前缀与尾块后缀 → `"a b"/"c d"/"e f"` 替换 1..1 →
    `["aZ f"]`。
  - 含 `\n` 的替换**不把换行存进 `Inline::Text`**，而是切成块。
  - 4 组非法范围（多字节中间、段内逆序、跨块逆序、越界）全部返回
    `InvalidRange` 且 **revision 不变**。
  - a18 全选替换 `"中文 $alpha$ tail"+"第二段 👍"` → 单动作、起始块身份保留。
- ADR 0031 的负向口径另经独立验证：a05/a06/b10 的语义（删内容不删定界符、
  空公式保留 `$$`、方向对称）及"结果中不得出现含 `$`/`*` 的
  `Inline::Text`"均成立。

> 附注：`scholium-document` 与 `scholium-model` 在本轮 **零改动**
> （`git diff` 为空），跨块接缝是既有实现，S2 未改其语义。

---

## 6. 门禁独立复跑（原始输出见 `.verify/s2b/evidence/`）

| 门禁 | 命令 | 退出码 |
|---|---|---|
| format | `cargo fmt --all -- --check` | **0**（输出为空） |
| size | `python3 scripts/check-rust-size.py` | **0**（`Rust file sizes: OK`） |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings` | **0** |
| rustdoc | `env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | **0** |
| tests | `cargo test --workspace -- --test-threads=2` | **0**（95 passed / 0 failed） |

`cargo test` 汇总（独立加总，非照抄）：
`test result: ok` 行合计 **95 passed, 0 failed**（另有 32 ignored，即 opt-in 契约）。
与实现者声称的"95 passed / 0 failed"一致。

---

## 7. 体验契约：13 → 16，零回归（独立复算）

为避免把"基线"当既定事实，**在 baseline 提交 `59eca0c` 上新建 git worktree 并重跑同一脚本**
（`python3 scripts/audit-editor.py --quick --out <fresh>`），再与 HEAD 结果逐用例比对：

- baseline 重跑：**13 passed / 19 failed**，与 `docs/spikes/evidence/SPK-0046/summary.md`
  的 32 行表格**逐用例完全一致**（脚本未变，可复现）。
- HEAD：**16 passed / 16 failed**。
- 逐用例差集（自算）：

```
FIXED (3): a05_backspace_after_formula_keeps_formula_structure
           a06_delete_before_formula_keeps_formula_structure
           b10_backspace_after_formula_with_geometry_preserves_structure
REGRESSED (0): []
still FAILED (16) / still ok (13)  —  与基线完全对应，无新增失败、无消失用例
```

**因此"16 通过、零回归、净 +3（a05/a06/b10）"成立，且满足计划 S2 的"≥ 13"下限。**

`--quick` 口径提示：脚本默认输出目录用 `tempfile.mkdtemp`，本沙盒 `/tmp` 不可建目录，
必须加 `--out <workspace 内空目录>`（任务已提示，实测确认）。

---

## 8. 原生窗口：4 → 5，零回归

`python3 scripts/audit-editor.py --native --out .verify/native-out`（含软件 GL 修复）：

```
inline-enter: failed      display-enter: failed     formula-backspace: passed
plain-split: passed       undo: failed              views-save-restore: passed
keyboard-crlf-paste: passed                          ribbon-crlf-paste: passed
passed: 5 / 8
```

与 `summary.md` 的 8 场景逐项比对：baseline 4 通过（`plain-split`、
`views-save-restore`、`keyboard-crlf-paste`、`ribbon-crlf-paste`），
本轮 **`formula-backspace` 由 failed 转 passed**，其余 7 项**状态不变**。
"5 通过、零回归"成立。

---

## 9. 应报备的缺口（未发现缺陷，但两处需 Lead 知情）

1. **b13 的"常规（非 opt-in）回归测试"尚未添加。** ADR 0031「验证方法」与
   E1 计划 S4 均要求 b13（同帧点击+键入越界 panic）另加一条**常规**回归测试。
   实测仓库内 b13 仍**只有** opt-in 的
   `audit!(b13_stale_click_and_typing_in_one_frame_never_panics, …)`；
   `page_editor/tests.rs` 的 `#[test]` 数量 baseline 与 HEAD **同为 10**，无新增。
   → 这是 **S4 的范围**，S2 并未声称已完成 b13（b13 至今仍 FAILED），
   **不构成 S2 声明的失实**；但请确认它已被 S4 明确承接，避免静默漏项。
2. **`caret.rs` 文档注释与实现细节的一处措辞。** `from_markup_byte` 的文档称
   "resolves shared offsets to the *later* node"，而实现中该优先级
   仅在**零宽**节点上生效（非零宽节点首次匹配即 `break`）。
   实测行为**不违反**函数承诺（只保证返回块内合法 caret），
   但若日后有人据该措辞推断非零宽共享字节的归属，可能误判。
   建议在 S3/S4 顺手澄清措辞（**不建议**在本轮改实现）。

---

## 10. 复现命令

```bash
# 门禁
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
python3 scripts/check-rust-size.py
env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test --workspace -- --test-threads=2

# 体验契约（/tmp 不可用时必须给 --out）
mkdir -p .verify/audit-out && python3 scripts/audit-editor.py --quick --out .verify/audit-out

# 原生窗口（脚本已内置 LIBGL_ALWAYS_SOFTWARE=1 / __GLX_VENDOR_LIBRARY_NAME=mesa）
mkdir -p .verify/native-out && python3 scripts/audit-editor.py --native --out .verify/native-out

# 基线复现（在 59eca0c 上重跑，应与 summary.md 的 13/32 逐用例一致）
git worktree add .verify/baseline/wt 59eca0c
cd .verify/baseline/wt && mkdir -p out && python3 scripts/audit-editor.py --quick --out out
```

原始日志：`.verify/s2b/evidence/`（`test.log`、`clippy.log`、`rustdoc.log`、`fmt.log`、
`quick.log`、`native.log`、`head-usability.log`、`baseline-usability.log`、
`baseline-summary.md`、`native-results.json`）。

验证者未修改任何实现或测试代码；`caret.rs` 校验和前后一致；
自建对抗用例目录与 baseline worktree **均已删除**，仓库内无遗留文件。
