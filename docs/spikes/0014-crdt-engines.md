# Spike 0014：CRDT 引擎对比夹具（关闭 ADR 0009）

> **有效性（2026-09-17 登记）**：引擎选型的原始依据，结论由 [ADR 0009](../adr/ADR-0009-crdt-engine.md) 固定（选定 Loro 1.16.0）；性能对比未做。阶段 0 当前判定见 [报告 0012](0012-exit-criteria.md)。

- 结论：**Pass —— 选定 Loro 1.16.0**
- 对应验证项：ADR 表「CRDT engine（Loro → Yrs → Automerge）」；[ADR 0009](../adr/ADR-0009-crdt-engine.md)
- 日期：2026-09-16
- 执行者：ation_ciger
- 关联：[报告 0007](0007-crdt-race.md)（自研最小实现，接口语义的参照）、[报告 0013](0013-crdt-prescreen.md)（预筛）
- 代码：[`spikes/crdt-engines/`](../../spikes/crdt-engines)

## 问题与判据

[ADR 0009](../adr/ADR-0009-crdt-engine.md) 写定的关闭判据：三个候选在同一份语义上跑
①两副本离线收敛 ②本地撤销保留远端输入 ③快照往返 ④可移动树；
另需回答"树用引擎自带 movable tree 还是自建"。

## 环境

- Loro `=1.16.0`、Yrs `=0.27.4`、Automerge `=0.11.0`，均 `default-features = false`，纯 Rust（无 `-sys` 编译）
- 复现：见文末

## 结果

| 检查 | Loro 1.16 | Yrs 0.27 | Automerge 0.11 |
|---|---|---|---|
| 两副本离线编辑后收敛 | **PASS** `你好world` | **PASS** `你好world` | **PASS** `world你好` |
| 本地撤销保留远端输入 | **PASS**（撤销后 `LOCALhello-remote`，远端 `-remote` 保留） | 无内置 undo 过滤的验证（见下） | **无 undo API** |
| 快照往返 | **PASS**（261 字节） | **PASS**（35 字节） | **PASS**（246 字节） |
| 可移动树 | **PASS**（`create`+`mov`，API 还有 `mov_to`/`mov_after`/`mov_before`） | **无 mov 类 API** | **无树类型** |

三个引擎都收敛，但**同一位置的并发插入次序不同**：Loro/Yrs 得 `你好world`，Automerge 得 `world你好`。
这是真实可观察的行为差异（去中心化文本的已知问题），会影响协作时的观感；三者都"收敛"，
但"收敛到哪一个"不同——这一点在选择时必须知道。

静态能力证据（对各自 crate 源码全量检索，不是只看文档）：

```text
Yrs：grep -rnE 'pub fn (mov|move_|mov_to|mov_after|mov_before)\b' yrs-0.27.4/src/  → 无结果
     树是 XmlFragment/XmlElement，只能插入/删除子节点；"移动到另一父节点"要自行拆成删除+插入，
     而那不是 CRDT 的 move 语义（并发移动的收敛性无从保证）。
Automerge：grep -rnE 'pub fn (undo|redo)\b' automerge-0.11.0/src/  → 无结果
     即：没有内置 undo manager，需要自己在 change history 上实现（并自行保证不覆盖远端输入）。
```

## 决策依据

ADR 0009 的两个决定性能力——**可移动树**与**内置且能过滤远端输入的 undo**——只有 Loro 同时具备，
且两者都在本夹具上端到端跑通（不是文档声明）：

- **可移动树**直接对应本项目的"包裹 / 解除 / 重排"：`LoroTree` 的 `mov`/`mov_to`/`mov_after`/`mov_before`
  就是这些操作，不必自建移动语义并自行证明并发移动的收敛性（报告 0007 的自研实现正是在这里留下缺口：
  并发互移可成环）。
- **UndoManager** 实测满足"撤销只作用于本地、远端输入保留"（`undo` 后 `XYZ` 消失、`-remote` 与 `LOCAL` 都在）。

## 失败与不确定性

- **未做性能对比。** 本夹具只验证语义能力，没有测吞吐、编码体积、内存与 10 万动作规模。
  报告 0007 的自研基线是 10 万动作 3.7 s / 堆增量 87.5 MiB，**Loro 在同等规模下的表现未知**。
- **Yrs 与 Automerge 只跑了文本与快照两项**：它们缺失的能力（移动、undo）来自**公开 API 全量检索**，
  不是运行期失败；没有为它们手工实现"删除+插入"式移动或自建 undo 来对比。
- **Yrs 的 undo manager 存在**（`UndoManager` + `include_origin`），但本次没有为它写"撤销保留远端输入"
  的用例——因此上表该格是"未验证"，不是"不支持"。这是本报告最明显的缺口。
- 未测：编码格式稳定性与迁移、压缩与 GC、wasm 目标编译、与三层历史（CRDT/action/checkpoint）的对接。
- 夹具本身踩过一次坑并已修正：Automerge 两个副本各自 `put_object` 会得到不同对象 id，
  合并后文本不合并，报出 `A="你好" B="world"` 的**假失败**；改为"B 从 A 的快照建立"后通过。
  记录它是因为这类"夹具错了、看着像引擎不行"的失败最容易误导结论。

## 对设计的影响

1. **ADR 0009 由 Proposed 转为 Accepted，选定 Loro 1.16.0**，锁定 `=1.16.0`。
2. **树的移动语义用引擎自带的 movable tree**，不再自建；报告 0007 里"并发互移可成环"的缺口
   由引擎承担，但**仍需在阶段 1 复核**引擎在并发移动下的行为（本夹具没有构造并发移动）。
3. **撤销用引擎的 `UndoManager`**，并保留"撤销只作用于本地"这条判据作为回归测试
   （本报告的 Loro 用例可直接移植）。
4. **阶段 1 的第一个迭代必须补性能对比**（10 万动作、编码体积、内存），若 Loro 明显劣于 Yrs，
   则按 ADR 0009 的替换方案转 Yrs 并自建移动语义；这条写进 ADR 的后果节。

## 复现步骤

```bash
cd /home/ation_ciger/Projects/Mogan/scholium
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"
cargo run --release --manifest-path spikes/crdt-engines/Cargo.toml
```

静态能力检查：

```bash
R=$CARGO_HOME/registry/src/*/yrs-0.27.4
grep -rnE 'pub fn (mov|move_|mov_to|mov_after|mov_before)\b' $R/src/    # 无结果
R=$CARGO_HOME/registry/src/*/automerge-0.11.0
grep -rnE 'pub fn (undo|redo)\b' $R/src/                                # 无结果
```
