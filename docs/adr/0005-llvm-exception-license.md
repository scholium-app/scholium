# ADR 0005：允许 Apache-2.0 WITH LLVM-exception 依赖

- 状态：Accepted
- 日期：2026-09-16
- 决策者：ation_ciger
- 影响模块：依赖许可门禁（`deny.toml`）、构建与工具链选型
- 关联：[ADR 0004 项目许可证](0004-project-license.md)

## 背景

[ADR 0004](0004-project-license.md) 与 `AGENT.md` 的许可证政策规定：新增依赖必须落在允许类别内，
由 `cargo deny` 强制；**引入新许可类别必须走新 ADR**。

阶段 0 的 spike 首次实际拉取了完整依赖树（`typst` 298 包、`recovery` 307 包等）。
按政策的严格允许清单运行时，`cargo deny check licenses` 失败：

```text
error[rejected]: failed to satisfy license requirements
   ┌─ .../ar_archive_writer-0.5.3/Cargo.toml:29:12
29 │ license = "Apache-2.0 WITH LLVM-exception"
   │            rejected: license is not explicitly allowed
```

同一许可还出现在 `rustix`、`linux-raw-sys`（两者都是底层系统调用的基础依赖，
会进入几乎所有 Linux 原生依赖树）。它们不在政策的允许清单内，因此门禁按设计拦住了。

## 验证方法

- 对每个 spike workspace 运行 `cargo deny --offline check licenses`，记录被拒依赖与其许可。
- 复现命令见本 ADR 末尾与 `docs/spikes/0011-exit-criteria.md`。

## 候选方案

| 方案 | 说明 | 结论 |
|---|---|---|
| A. 不加该许可，换掉相关依赖 | `rustix`/`linux-raw-sys` 是 Linux 原生栈的基础依赖（`std` 之外的文件/进程/时间接口），换掉等于放弃整条原生依赖链 | 不可行 |
| B. 逐个依赖写 `exceptions` | 为每个包开例外；新包出现就要再开一次，门禁会退化成"打地鼠" | 不可取 |
| C. 把该许可加入允许清单并记录理由 | 一次性澄清类别边界 | **采用** |

## 决策

**允许 `Apache-2.0 WITH LLVM-exception`。**

理由：它是 Apache-2.0 附加一项**放弃性**例外（LLVM 例外只放弃 Apache-2.0 第 2 节中关于
专利/衍生作品的部分要求），不增加任何限制，权限**严格宽于**已经允许的 Apache-2.0。
因此这不是放松政策，而是把"Apache-2.0 的附带例外"明确归入同一类别。

适用边界：

- 仅限该 SPDX 表达式本身；其它 `Apache-2.0 WITH <例外>` 需要单独评估。
- 该判断只覆盖许可本身，不改变"以独立子进程调用的 GPL 工具链"等既有规则。

同时把 `deny.toml` 的 `licenses.allow` 补上该项，并在 `AGENT.md` 的允许清单里显式列出（指向本 ADR）。

## 后果

正面：

- 门禁不再因基础依赖的附带例外而失败，同时**仍然拦得住真正的 copyleft**（见验证：`slint`/`wry`/`tauri`
  在 `bans.deny` 中，GPL/AGPL 不在允许清单内）。
- 政策与实际依赖树的边界被实际运行确认过一次，而不是停留在纸面。

负面与风险：

- 允许清单多一项，需要维护；若将来出现 `Apache-2.0 WITH` 的其它例外，仍需单独 ADR。
- `deny.toml` 目前只在 spike workspace 上验证过；正式 crate 建立后必须把它接入 CI，
  并在根 workspace 有成员后运行（当前根 workspace `members = []`，`cargo deny` 无法在根目录运行）。

需要新增的门禁：

- CI 中对每个 workspace 运行 `cargo deny check licenses bans advisories`，失败即阻断。

## 替换方案

`Apache-2.0 WITH LLVM-exception` 的依赖只是运行期/构建期的基础库，替换路径与 Apache-2.0 依赖一致：
换用许可更简的实现或移除该依赖。本决定不涉及任何数据格式或公共接口，因此不需要兼容层。
