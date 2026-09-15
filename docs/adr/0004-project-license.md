# ADR 0004：项目许可证

- 状态：Accepted
- 日期：2026-09-16
- 决策者：用户
- 影响模块：全仓库、构建与 CI、依赖选型、文档

## 背景

workspace 最初只在 `Cargo.toml` 与 `README.md` 中声明 Apache-2.0，既没有许可证正文，也没有依赖许可
政策。项目已明确不搬运 Mogan/TeXmacs 源码（`AGENT.md` 约束 1），而 Mogan/TeXmacs 是 GPL-3，因此
GPL 对本项目不是合规必需，而是偏好选择。

同时，候选技术栈里存在非宽松许可：**Slint** 提供 GPLv3、商业许可或 Slint Royalty-free 许可，均不是
MIT/Apache；**EUI-NEO** 的许可证尚未核实。不先定项目许可，就无法判断这两个候选是否可用。许可证一旦
接受过外部贡献就很难更改，必须在实现开始前决定。

## 候选方案

1. **Apache-2.0 单一许可**：带明确专利授权与报复条款，企业友好；但比 MIT 严格，且与 GPL-2.0 不兼容
   （与 GPL-3 兼容）。
2. **MIT 单一许可**：最短、最宽松；但没有显式专利授权，企业采用时可能需要额外评估。
3. **MIT OR Apache-2.0 双许可**：Rust 生态惯例，下游可任选其一，兼容面最大；代价是多维护一个许可文件。
4. **GPL-3.0 整体**：强制开源回馈；但排除商业嵌入，且一旦接受外部 GPL-3 贡献即不可逆。仅在计划复用
   Mogan/TeXmacs 代码（与本项目约束冲突）或有明确 copyleft 诉求时成立。
5. **核心宽松 + sync-server AGPL-3.0**：适合提供官方托管并避免他人直接托管白嫖；但托管模式仍是未决项
   （`PRODUCT.md` §9），当前不预先绑定。

## 决策

采用**候选方案 3：MIT OR Apache-2.0 双许可**。正文见仓库根 `LICENSE-MIT` 与 `LICENSE-APACHE`，
`Cargo.toml` 的 SPDX 表达式为 `MIT OR Apache-2.0`。

依赖许可政策与禁止类别见 `AGENT.md` 的许可证政策一节，由 `cargo deny` 强制。所有提交使用 DCO 1.1
（`git commit -s`），保证许可在必要时仍可整体调整。

`sync-server` 暂与核心同许可，不单独采用 AGPL-3.0。项目当前没有法律实体，MIT 许可的版权行使用
`Scholium contributors`，与 DCO 下贡献者各自保留版权的安排一致。

## 验证方法

- `cargo deny check licenses` 在 CI 通过，且 `deny.toml` 的允许清单与 `AGENT.md` 政策逐条一致。
- 阶段 0 每个候选的报告必须记录该框架及其必需传递依赖的许可证，Slint 与 EUI-NEO 两项必须给出明确结论。
- 发布产物生成 SBOM 并复核许可类别，与 `SECURITY.md` 的供应链要求一致。

## 后果

- 下游可在 MIT 与 Apache-2.0 之间选择，最大化学术与商业采用；选 Apache-2.0 路径时自带专利授权。
- 不能在双许可下链接 GPLv3 的 Slint，因此 Slint 候选实际被降级为"需先改许可或购买商业许可"，
  这会影响原生 UI 验证顺序的推进方式。
- 选择宽松许可意味着放弃强制回馈；社区贡献依靠 DCO 与评审，而不是许可约束。
- 需要维护两个许可文件，并明确源文件头部版权声明的策略。

## 待定项

**当前决定：`sync-server` 与核心同为 `MIT OR Apache-2.0`，暂不采用 AGPL-3.0。** 托管模式尚未确定
（`PRODUCT.md` §9）；若只自托管，AGPL 只会给企业自托管者增加合规阻力而不带来收益，而宽松优先保留
选项，DCO 记录也使该 crate 将来仍可单独重新许可。若确定提供官方托管服务，另立 ADR 评估改为
AGPL-3.0，并在此前避免接受大量外部贡献到该 crate。

## 替换方案

若将来需要复用 GPL-3 代码或改为强 copyleft，可整体重新许可为 GPL-3.0：MIT 与 Apache-2.0 都可并入
GPL-3 组合作品，方向可逆；反向（GPL-3 → 宽松）不可行。因此许可调整必须在新贡献大量进入前完成，
或依赖 DCO 记录取得贡献者授权。改变许可类别需新 ADR，并同步 `deny.toml` 与 `AGENT.md`。
