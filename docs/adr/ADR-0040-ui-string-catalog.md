# ADR 0040：界面文字目录采用 Project Fluent

- 状态：Accepted
- 日期：2026-10-14
- 决策者：Scholium 维护者
- 影响模块：`scholium-app`（命令注册表、`i18n`），构建与 CI 门禁

## 背景

[界面多语言计划](../plan/I18N.md)要求菜单、工具栏、提示、读屏名称等界面文字经目录渲染，key 稳定、zh-Hans 与 en 完整、
运行时切换、缺项可被测试发现。命令注册表（[应用壳](../modules/app.md#命令系统)）是第一批使用者：命令标题、提示、
菜单名与不可用原因改由目录提供。目录格式与库须在此裁决：候选为 Project Fluent（`fluent-bundle`）与自研简单目录。

## 验证方法

判据取自 I18N.md 与任务约束，逐条在仓库里量：

| 判据 | 量法 |
|---|---|
| `cargo deny` 通过 | `cargo deny check licenses bans sources`，以及新增依赖是否带入新的 advisory |
| 编译到 wasm32 | 独立临时 crate 依赖 `fluent-bundle`，`cargo build --target wasm32-unknown-unknown` |
| zh/en CLDR 复数 | `scholium-app` 单测 `fluent_selects_cldr_plural_forms_for_en_and_zh` |
| 编译期嵌入 | `include_str!` 目录文件，运行时不读磁盘、不联网 |
| 运行时切换 | 单测 `the_same_command_title_switches_language_at_runtime` 与整窗帧测试 `the_interface_language_switches_at_runtime_without_touching_the_document` |
| 测试发现缺项 | `i18n/tests.rs`：两种语言 key 集合相等、注册命令的标题与提示都有定义、目录无未使用 key |

## 候选方案

**自研目录**（TSV/TOML 加手写复数表）：零依赖、key 与点号命名天然一致；但 CLDR 复数、参数化句子、选择器都要
自己维护，en/zh 之外的语言（阶段 6）会把复数规则表推到自己身上，翻译贡献者没有现成的工具链和语法。

**Project Fluent（`fluent-bundle` 0.16.0）**：成熟库，规范稳定，复数用 CLDR 规则（`intl_pluralrules`），
支持命名参数、选择器和属性；`.ftl` 文本便于翻译协作。代价是 5 个新 crate，以及 Fluent 标识符不允许点号。

## 量得的结果

- **许可**：新增 5 个 crate——`fluent-bundle` 0.16.0、`fluent-langneg` 0.13.1、`fluent-syntax` 0.12.0、
  `intl-memoizer` 0.5.3、`intl_pluralrules` 7.0.2——均为 `Apache-2.0 OR MIT`（`intl_pluralrules` 标为 `Apache-2.0/MIT`）；
  `unic-langid` 等已在依赖图中。`cargo deny check licenses bans sources` 通过。
  `cargo deny check advisories` 在本任务前后都失败，失败项同为 `bincode`、`paste`、`rustybuzz`、`ttf-parser`、
  `unsafe-libyaml`、`yaml-rust`（unmaintained）和 `quick-xml`（两条漏洞），均经 Typst 依赖而来，与本变更无关，新增的
  Fluent crate 本身不在任何 advisory 中（另有一条 `yoke-derive` 被撤回的警告，经 `unic-langid` 到达，同一路径早已由
  Typst 的 `biblatex` 引入）。
- **wasm32**：独立 crate 依赖 `fluent-bundle` 在 `wasm32-unknown-unknown` 下编译通过（冷构建约 4 s，含 30 个包）。
- **复数**：en 的 `one`/`other`（1 → `1 item`，2 → `2 items`）与 zh 的单一 `other`（`5 项`）单测通过。
- **嵌入与切换**：zh-Hans.ftl 5.9 KB、en.ftl 6.1 KB，`include_str!` 嵌入；每次查找按当前 `Locale` 取已解析的 bundle，
  切换界面语言下一帧生效，文档快照不变。缺 key 时回退英语再回退 key 本身，测试保证不会发布。
- **缺项检测**：测试比较两份目录的 key 集合，并核对注册表引用的每个 key 在两种语言中均有定义，目录中没有无人使用的 key。

没有判据失败，因此按“成熟库优先”选 Fluent。

## 决策

采用 Project Fluent，库为 `fluent-bundle`（并发 bundle，关闭 Unicode 隔离符以免进入控件文字与读屏名称）。

- 目录文件：`crates/scholium-app/i18n/zh-Hans.ftl` 与 `en.ftl`，随程序编译嵌入。
- **key 规则**不变，仍是点号分隔的稳定 ASCII 名：`command.<id>.title`、`command.<id>.tooltip`、`menu.<id>`、
  `reason.<name>`。映射到 Fluent：消息 id 把点号换成连字符（`menu.file` → `menu-file`）；`command.` key 的末段是属性，
  消息为 `command-<id 的点号换连字符>`（`command.format.bold.title` → `command-format-bold` 的 `.title`）。命令 id 只含
  小写字母、数字、下划线与点，所以映射可逆。
- 界面语言偏好 `LocaleSetting`（`auto` / `zh-Hans` / `en`）是 `WorkspaceState` 的字段，默认 `auto`：按 `LC_ALL`、
  `LC_MESSAGES`、`LANG` 的第一个非空值判定，`zh*` 为 zh-Hans，其余与缺省为 en。持久保存与界面控件另行接入。
- T12 范围内只有注册表拥有的文字（命令标题与提示、菜单名、不可用原因）走目录；其余硬编码中文进入
  `scripts/ui-strings-allowlist.txt`，由 `scripts/check-ui-strings.py` 在 CI lint 作业里保证只减不增。

明确不承诺：参数化句子与复数在应用文字中的使用（暂无需要，单测已证明库能力）、RTL、其他语言。

## 后果

- 正面：复数、参数、选择器无需自研；翻译者用标准 `.ftl`；阶段 6 新增语言只加文件。
- 负面：多 5 个依赖；Fluent 标识符不能含点号，需要上述可逆映射；每个目录文件解析一次，常驻内存。
- 风险：`fluent-bundle` 的非并发 bundle 不是 `Sync`，因此用并发 bundle；升级库需重跑上表判据。
- 门禁：key 集合一致、注册命令全覆盖、无未使用 key（单测）；硬编码文字门禁（CI lint）。

## 替换方案

目录访问只通过 `i18n::text(locale, key)` 一个函数，key 是库无关的点号名；换成自研目录只需替换 `i18n.rs` 内部
和 `.ftl` 文件，注册表、界面与测试不变。
