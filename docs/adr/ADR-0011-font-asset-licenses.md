# ADR 0011：允许字体资产许可（OFL-1.1 / Ubuntu-font-1.0）

- 状态：Accepted
- 日期：2026-09-16
- 决策者：ation_ciger
- 影响模块：依赖许可门禁、字体与内容资产登记、安装包体积
- 关联：[ADR 0004 项目许可证](ADR-0004-project-license.md)、[ADR 0006 原生桌面框架](ADR-0006-native-ui-framework.md)、报告 [0012](../spikes/SPK-0012-exit-criteria.md)

## 背景

`AGENT.md` 的许可证政策把**内容类资产**单列：『字体、图标和夹具样本各自声明许可证』。
但允许清单（`licenses.allow`）只覆盖代码许可，没有覆盖字体许可。

[ADR 0006](ADR-0006-native-ui-framework.md) 选定 egui 后，其默认字体包 `epaint_default_fonts 0.36.2`
进入依赖树，声明为：

```text
license = "(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0"
```

`cargo deny check licenses` 因此失败。这是**第一次**因为内容资产而不是代码许可触发门禁——
说明门禁确实按设计工作，同时也暴露了政策里"字体许可"这一类目没有被落到清单上。

## 验证方法

对 `spikes/native-ui/candidate-egui/` 运行 `cargo deny --offline check licenses`，读取被拒 crate 与许可；
再核对被拒 crate 是代码还是资产（`epaint_default_fonts` 的 crate 内容是 `.ttf` 字体文件）。

## 候选方案

| 方案 | 说明 | 结论 |
|---|---|---|
| A. 不使用 egui 默认字体 | 需要自带并登记一套中文字体；默认字体是 egui 开箱可用的前提，去掉会改变渲染基线 | 暂不采用（可作为后续瘦身选项） |
| B. 为主 crate 写 `exceptions` | 例外会随依赖变化反复出现 | 不可取 |
| C. 把 OFL-1.1 与 Ubuntu-font-1.0 明确列为**字体资产**允许类别 | 一次性澄清，并保留登记义务 | **采用** |

## 决策

**允许 OFL-1.1（SIL Open Font License 1.1）与 Ubuntu-font-1.0，仅限未修改的字体资产。**

理由与边界：

- 两者都是**面向字体的宽松许可**：允许随软件一起分发与嵌入，不要求作品整体同许可，无 copyleft。
- **限制一（保留名称）**：若修改字体，不得继续使用原字体名（OFL 的 Reserved Font Name；Ubuntu-font-1.0 的
  Ubuntu 名称条款）。**本项目不分发修改版字体**，因此不触发。
- **限制二（不得单独售卖字体）**：两者都禁止把字体本身作为商品出售；随应用分发不受影响。
- **登记义务**：按 `AGENT.md` 的内容资产规则，使用这些字体必须登记来源（crate 与版本）、
  确切字体名与许可，并在分发物中保留许可文本。
- 该决定**不**扩展到其它字体许可（例如仅限非商业使用的字体），它们仍需单独 ADR。

同时修正两处门禁配置：

1. `deny.toml` 的 `licenses.allow` 增加 `OFL-1.1`、`Ubuntu-font-1.0`。
2. `bans.wildcards = "deny"` 会误伤**路径依赖**（`scholium-spike-core = { path = "../core" }` 没有版本号），
   改为 `allow-wildcard-paths = true`：仍然禁止 registry 依赖使用通配版本，但允许本地路径依赖。

## 后果

正面：

- 门禁第一次覆盖到**内容资产**这一类，并把"字体许可"从政策文本落到可执行的清单上。
- 暴露了一次我自己的验证漏洞：先前的门禁检查只跑了部分 workspace 与单个子检查，
  且 `bans` 的 wildcard 规则被路径依赖误伤——两者都已修正（见报告 0012）。

负面与风险：

- 允许清单增至 15 项，且现在混有两类语义（代码许可、字体资产许可），
  后续应在 `deny.toml` 注释里保持区分。
- 采用 egui 默认字体会把 Ubuntu-Light 与 emoji 字体打进安装包，增加体积；
  中文显示仍需自带字体（当前用系统 Source Han Serif）。打包体积与字体子集化留待阶段 5。

## 替换方案

字体资产与代码依赖解耦：若要缩减体积或规避 Ubuntu 字体名条款，可在构建时禁用
`epaint_default_fonts` 并登记自带的替代字体，不涉及任何数据格式或公共接口。
