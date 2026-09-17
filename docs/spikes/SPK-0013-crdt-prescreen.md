# Spike 0013：CRDT 引擎预筛（Loro / Yrs / Automerge）

> **有效性（2026-09-17 登记）**：预筛结论（三者许可均合规），不含选型；选型由 [ADR 0009](../adr/ADR-0009-crdt-engine.md) 依 [报告 0014](SPK-0014-crdt-engines.md) 固定。阶段 0 当前判定见 [报告 0012](SPK-0012-exit-criteria.md)。

- 结论：**预筛通过，三者许可均合规**；引擎选型仍需一次夹具对比（见文末），故不在此下定论
- 对应验证项：ADR 表「CRDT engine（Loro → Yrs → Automerge）」
- 日期：2026-09-16
- 执行者：ation_ciger
- 关联：[ADR 0001 团队单一源码语言与混合项目](../adr/ADR-0001-mixed-source-team-editing.md)、报告 [0007](SPK-0007-crdt-race.md)

## 问题与判据

[ADR 0001](../adr/ADR-0001-mixed-source-team-editing.md) 要求"共享树 + 共享文本"两层协同编辑，
并配本地撤销、快照与 10 万次动作的规模验证。报告 0007 用一个**自写的最小 CRDT** 验证了收敛与撤销语义，
但它不是引擎选型。本预筛回答四件事：

| # | 判据 |
|---|---|
| 1 | 许可证是否落入 `AGENT.md` 允许清单 |
| 2 | 是否能解析并锁定到明确版本，依赖树规模是否可接受 |
| 3 | 是否含需自行登记 ABI 的 C/C++/FFI 依赖 |
| 4 | 是否覆盖本项目需要的四个能力：共享文本、共享树、本地撤销（保留远端输入）、快照 |

## 环境

`cargo add --no-default-features` 解析 + `cargo fetch`，读取解压后的 `Cargo.toml` 与 `Cargo.lock`：

| 引擎 | 版本 | 许可 | 三者合计依赖包数 | `-sys` crate |
|---|---|---|---|---|
| **Loro** | 1.16.0 | **MIT** | 192（三者合计，均为 no-default-features） | 只有 `js-sys` / `windows-sys`（非 Linux 目标，不编译 C） |
| **Yrs** | 0.27.4 | **MIT** | 同上 | 同上 |
| **Automerge** | 0.11.0 | **MIT** | 同上 | 同上 |

三个都是纯 Rust，没有需要登记 ABI/生命周期的本地代码依赖。

## 能力对照（来自各自文档，未在本项目夹具上实测）

| 能力 | Loro 1.16 | Yrs 0.27 | Automerge 0.11 |
|---|---|---|---|
| 共享文本 | `LoroText`（含富文本样式） | `YText`（含格式化属性） | `Text` / 富文本 mark |
| 共享树 / 结构 | **`LoroTree`（可移动树，含 move 操作）** | `YXmlElement`（XML 树，无任意 move） | 无树类型，需用嵌套 map 自行建模 |
| 本地撤销保留远端输入 | `UndoManager` + 作用域/来源过滤 | `UndoManager` + `origin` 过滤 | **无内置 undo manager**，需自行实现 |
| 快照 / 持久化 | `export`/`import`（含快照模式） | `encode_state_as_update`/`apply_update` | `save`/`load` |
| WASM | 一等支持 | 一等支持 | 有 wasm 支持 |
| 生态成熟度 | 较新，自研实现 | Yjs 的 Rust 移植，生态最大 | Automerge 官方 Rust 实现，文档最完整 |

**对本项目最相关的一点**：语义图是**可移动的树**（包裹、解除、重排都要表达为 move），
只有 Loro 提供专门的 movable tree 类型。这不等于它一定胜出——树结构也可以用"节点 id 稳定 + 父指针作为普通字段"
在文本/映射 CRDT 上建模（报告 0007 的最小实现就是这么做的），但代价是移动语义要自己保证收敛。

## 结论与仍需验证的部分

**预筛结论**：三者在许可证、版本锁定、无 C 依赖三项上**全部通过**，都可以进入候选。
**尚不能定选**：上表是文档级对照，没有在本项目的夹具上跑过。按本项目的纪律，
引擎选型必须由一次**夹具对比**关闭（见 ADR 0009 的验证方法）：

1. 同一份夹具（文本 + 可移动树）两副本离线编辑后收敛，比较字节相等与耗时；
2. 本地撤销保留远端输入（三者的 undo manager 语义不同，必须实测）；
3. 快照往返 + 快照后继续合并仍收敛；
4. 10 万次动作的耗时与内存；
5. wasm 目标能否编译（阶段 3 的 WASM 兼容要求）。

未做：以上五项对比；引擎的压缩、GC、增量编码效率；与三层历史（CRDT / action / checkpoint）的对接方式。

## 复现步骤

```bash
cd /tmp && rm -rf crdt-probe && mkdir crdt-probe && cd crdt-probe
cat > Cargo.toml <<'EOF'
[package]
name = "crdt-probe"
version = "0.0.0"
edition = "2024"
[workspace]
EOF
mkdir -p src && echo 'fn main(){}' > src/main.rs
export CARGO_HOME=/home/ation_ciger/Projects/Mogan/scholium/spikes/native-ui/.cargo-home
cargo add loro --no-default-features
cargo add yrs --no-default-features
cargo add automerge --no-default-features
cargo fetch
grep -c '^\[\[package\]\]' Cargo.lock
```
