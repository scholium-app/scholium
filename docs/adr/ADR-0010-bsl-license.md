# ADR 0010：允许 BSL-1.0 依赖

- 状态：Accepted
- 日期：2026-09-16
- 决策者：ation_ciger
- 影响模块：依赖许可门禁（`deny.toml`）、CRDT 与工具链依赖
- 关联：[ADR 0004 项目许可证](ADR-0004-project-license.md)、[ADR 0005 Apache-2.0 WITH LLVM-exception](ADR-0005-llvm-exception-license.md)、[ADR 0009 CRDT 引擎选型](ADR-0009-crdt-engine.md)、报告 [0014](../spikes/SPK-0014-crdt-engines.md)（本项由该 spike 的许可门禁发现）

## 背景

按 `AGENT.md` 的许可证政策，新增依赖必须落在允许类别内，由 `cargo deny` 强制；
**引入新许可类别必须走新 ADR**。

[ADR 0009](ADR-0009-crdt-engine.md) 选定 Loro 作为 CRDT 引擎后，其依赖树引入了
**Boost Software License 1.0**（`xxhash-rust`，Loro 与 Yrs 都用它）。门禁按设计拦住了：

```text
error[rejected]: failed to satisfy license requirements
   ┌─ .../xxhash-rust-0.8.18/Cargo.toml:42:12
42 │ license = "BSL-1.0"
   │            rejected: license is not explicitly allowed
```

## 验证方法

对 `spikes/crdt-engines/` 运行 `cargo deny --offline check licenses`，记录被拒的许可与来源 crate。

## 候选方案

| 方案 | 说明 | 结论 |
|---|---|---|
| A. 不加，换掉 `xxhash-rust` | 它是 Loro/Yrs 的传递依赖，换掉等于放弃已选定的 CRDT 引擎 | 不可行 |
| B. 为该 crate 写 `exceptions` | 例外会随依赖变化反复出现 | 不可取 |
| C. 把 BSL-1.0 加入允许类别 | 一次性澄清 | **采用** |

## 决策

**允许 BSL-1.0（Boost Software License 1.0）。**

理由：BSL-1.0 是 OSI 认可的**宽松**许可，与 MIT 同属"保留版权声明、不要求衍生作品同许可"的类别，
没有任何 copyleft 或商业限制；它唯一的实质要求是"分发源码时保留许可声明"，
比已允许的 MPL-2.0（文件级 copyleft）限制更少。因此这不是放松政策，而是补齐一个同类宽松许可。

适用边界：

- 仅限 BSL-1.0 本身；其它 Boost 变体或"BSL"字样需单独确认（`BSL-1.1` 是另一回事）。
- 该判断只覆盖许可本身，不改变既有规则（GPL/AGPL/SSPL/BUSL 仍禁止；Slint 仍禁止）。

同时把 `deny.toml` 的 `licenses.allow` 与 `AGENT.md` 的允许清单补上该项（指向本 ADR）。

## 后果

正面：

- 门禁不再因常见宽松许可而误伤，同时仍拦得住真正的 copyleft。
- 这是政策第二次被实际依赖树检验（第一次是 Apache-2.0 WITH LLVM-exception），边界更清楚。

负面与风险：

- 允许清单增至 13 项，需要维护；若再出现新的宽松许可，仍应走 ADR 而不是随手加。

需要新增的门禁：

- CI 中对每个 workspace 运行 `cargo deny check licenses bans advisories`（当前只能逐 crate 跑，
  因为根 workspace 没有成员）。

## 替换方案

`xxhash-rust` 只是哈希实现，替换路径与其他宽松依赖一致：换用许可更简的实现或移除该依赖。
本决定不涉及数据格式或公共接口，不需要兼容层。
