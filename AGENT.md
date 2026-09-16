# Scholium 开发规范

设计索引见 [docs/README.md](docs/README.md)。废弃原型已经删除；开始正式实现前必须阅读产品、体验、
架构、数据模型以及所修改模块的独立设计文档。

## 硬性约束

1. **不搬运废弃原型、`../mogan` 或 `../mogan-rs` 的实现。** 技术结论可复用，
   源码不复用；TeXmacs 系代码还会把许可证锁死到 GPL-3。
2. **一个项目只能有一个权威模型。** 原生项目以语义文档图为真；源码项目以 `.tex` / `.typ` /
   `.md` 为真。切换权威模型必须是带兼容报告的迁移，不能靠两个可写副本互相覆盖。
3. **CRDT、用户动作、checkpoint DAG 是三层。** 不用快照栈实现协作撤销，
   不把 CRDT 内部更新直接当产品版本史，也不用 Git 做逐键同步。
4. **共享历史只追加。** undo/revert/merge 都生成新变更，不原地回退共享状态；
   默认 undo 只跟踪当前 actor 的 origin。
5. **跨格式转换不可静默丢失。** unsupported 结构必须带源位置进入导出报告。
6. **快速预览不能冒充最终排版。** LaTeX 发布目标可用 Typst 交互预览，但最终结果与兼容性以
   LaTeX 验证构建为准，UI 必须始终显示当前后端和 revision。
7. **阶段出口优先。** 阶段 0 的七项否决性验证——原生结构/源码编辑、Typst 映射、源码 reconcile、CRDT 赛马、
   构建与恢复、团队语言协调、混合构建——全部通过前，不搭生产脚手架，不做 AI、CAS、绘图、幻灯片或插件市场。
   清单、出口条件和证据要求见 `docs/ROADMAP.md` 与 `docs/spikes/README.md`。

8. **团队只能同时编辑一种源码语言。** 同一共享项目分支的 LaTeX/Typst 写入受活动语言与 epoch 门禁约束；同语言可多人编辑，编译可并行，不能仅在本机 UI 实现。
9. **混用是完整产品要求。** 正文、公式、图表、宏、模板、跨片段引用都要有验证路径。Raw 保留不等于支持执行；正式输出不得包含 unresolved 占位。见 `docs/MIXED_SOURCE_EDITING.md`。

10. **原生技术栈。** 应用 UI、核心与服务端优先 Rust；必要时引入 C/C++/Zig。禁止 npm/Node.js、JavaScript/TypeScript 编辑器、WebView/Electron/Tauri 作为应用 UI 或验证依赖。HTML 导出是文件能力，不是 UI 实现。可选浏览器 WASM 允许必要的工具生成加载/绑定胶水，不允许 JS/TS 业务编辑器或 npm/Node.js；见 `docs/WASM.md`。
    C/C++/Zig 依赖（含经由构建脚本编译本地代码的 crate）必须登记在 [docs/NATIVE_DEPENDENCIES.md](docs/NATIVE_DEPENDENCIES.md)，显式声明 ABI、内存所有权、线程约束与销毁顺序。
11. **UI 验证顺序固定。** 按 Iced → GPUI → C++ EUI-NEO → Slint 或 egui 的顺序，用 `docs/NATIVE_UI_VALIDATION.md` 的同一验收集验证，不以演示能运行替代编辑器可行性。**阶段 0 已完成并选定 egui**（[ADR 0006](docs/adr/0006-native-ui-framework.md)、报告 0001–0004）；更换框架必须新立 ADR 并重跑同一验收集。

## 设计文档纪律

- `docs/PLAN.md` 只是总览；产品、体验、架构、数据模型、排版转换、混合源码与团队编辑、协议、历史、格式、
  安全、测试、原生 UI 验证、全栈候选、WASM 和路线图各自维护。
- 修改模块前阅读 `docs/modules/<module>.md`；公共接口、不变量或职责发生变化时，同一提交更新文档。
- 第三方核心选型、长期存储格式、协议破坏性变化和跨模块边界调整必须新增 ADR。
- `docs/archive/` 只用于查证废弃原型，不得作为实现规范引用；确需沿用的实验结论应重新验证。
- 设计中的“待验证”必须通过 spike 关闭，报告落在 `docs/spikes/` 并回链到阶段 0 对应条目；
  测试代码放 `spikes/<name>/`。不能通过先写生产脚手架、以后再验证来关闭风险。

## 分支与提交

格式：`username/<type>/<description>`，type 取 `feat` / `fix` / `chore` / `refactor` / `docs`。

提交信息：`<type>: <简述>`。一个 PR 聚焦一个主题。推送直接用 `git push`，不用 `gh`。

所有提交必须带签署行（`git commit -s`）：

```text
Signed-off-by: 姓名 <邮箱>
```

这是 [DCO 1.1](DCO) 的要求，确认贡献者有权按本项目许可提交该内容。没有签署行的提交不接受。
采用 DCO 而非 CLA，是为了让贡献者保留版权、同时保证许可仍可在需要时整体调整。

## 许可证政策

本项目以 **MIT OR Apache-2.0** 双许可发布（见 [LICENSE-MIT](LICENSE-MIT)、[LICENSE-APACHE](LICENSE-APACHE)），
决策依据见 [ADR 0004](docs/adr/0004-project-license.md)。新增依赖必须落在下列允许类别内，由 `cargo deny` 强制。

允许，无需额外审批：

- MIT、Apache-2.0、**Apache-2.0 WITH LLVM-exception**（依据 [ADR 0005](docs/adr/0005-llvm-exception-license.md)）、
  **BSL-1.0**（Boost Software License，依据 [ADR 0010](docs/adr/0010-bsl-license.md)）、
  BSD-2-Clause、BSD-3-Clause、ISC、Zlib、0BSD、Unicode-DFS、Unicode-3.0、CC0-1.0、Unlicense。
- MPL-2.0：仅作为文件级 copyleft 依赖；修改其文件时按 MPL 公开该文件。
- 以**独立子进程**调用的外部工具链（例如 TeX Live 的 GPL 系组件）不构成链接，不改变本项目许可；
  但若在安装包中分发其二进制，必须单独履行对应 GPL 义务并登记。
- LGPL：仅限动态链接且用户可替换该库；静态链接按禁止处理。

需 ADR 批准，默认禁止：

- GPL-2.0、GPL-3.0、AGPL-3.0、SSPL、BUSL，以及任何"非商业""仅限评估""禁止竞品"条款。
- 源码可见但受限的自定义许可，例如 **Slint**（GPLv3、商业许可或 Slint Royalty-free 许可，均非宽松）。
  在项目保持双许可的前提下不得链接 Slint；选择它必须先把项目改为对应 copyleft 或取得商业许可。
- 许可未明确的依赖或代码。
- EUI-NEO：许可证已核实为 **Apache-2.0**（见[报告 0003](docs/spikes/0003-native-ui-prescreen.md)），
  但**无任何无障碍支持**，加上 C++/CMake + FFI 的成本，预筛即不投入（[ADR 0006](docs/adr/0006-native-ui-framework.md)）。

其他规则：

- 内容类资产单独登记：CSL 样式文件为 CC-BY-SA，打包时保留署名并说明同类共享要求；字体、图标和
  夹具样本各自声明许可证。**字体许可**另列一类：`OFL-1.1` 与 `Ubuntu-font-1.0` 允许用于
  **未修改**的字体资产（须登记来源、字体名与许可，并随分发保留许可文本），依据见
  [ADR 0011](docs/adr/0011-font-asset-licenses.md)；其它字体许可仍需单独审批。
- `deny.toml` 随首个 crate 提交，按上表配置 `licenses.allow`；CI 的 `cargo deny` 失败即阻断。
- 引入新许可类别、需要例外，或调整本政策，都走新 ADR。

## Rust 规范

- Edition 2024；MSRV 在技术验证后确定并由 CI 固定
- `pub` 类型须实现 `Debug`
- 有 `new()` 须同步实现 `Default`
- 错误用 `thiserror`，不用裸 `String`
- 原生/服务端异步用 `tokio`；WASM 核心不绑定运行时，浏览器调度由宿主适配。

## 规模与复杂度

阈值不是审美偏好，是可维护性的下限。超了就拆，不要辩解。

| 对象 | 建议 | 上限 | 超了怎么办 |
|---|---|---|---|
| 单文件 | 200-400 行 | **600 行** | 按职责拆模块，不要按"上半部分/下半部分"切 |
| 单函数 | < 30 行 | **60 行** | 抽出命名良好的私有函数 |
| 嵌套深度 | ≤ 3 层 | **4 层** | 早返回、`let else`、`?`，把守卫条件提到顶部 |
| 函数参数 | ≤ 4 个 | **6 个** | 聚成参数 struct（尤其是几何/布局参数） |
| `match` 分支体 | ≤ 10 行 | — | 分支体超了就抽成函数，保持 match 可一眼扫完 |
| 泛型参数 | ≤ 2 个 | **3 个** | 考虑 trait object 或具体类型 |

`600 行`这条对本项目尤其重要：旧项目里 671 个 C++ 文件 20.7 万行，
平均 300 行看着还行，但真正难改的都是那些超千行的核心文件。

拆分要沿职责边界，不沿行数边界。`layout.rs` 拆成 `layout/frame.rs` +
`layout/glyph.rs` + `layout/hit_test.rs` 是对的；拆成 `layout_1.rs` + `layout_2.rs` 是错的。

## 可维护性

- **抽象要等到第三次重复**。前两次复制粘贴可以接受，第三次再抽。
  过早抽象比重复更难拆。
- **删代码，不要注释掉代码**。git 记得住，注释掉的代码只会腐烂。
- **魔法数字一律具名常量**，尤其是排版里的尺寸、阈值、debounce 时长。
  `16` 到处出现却不知道是 ms 还是 px，是这类项目最典型的腐化方式。
- **返回值优先不可变**。渲染/布局热路径上确需就地累积时，
  用 builder 或局部 `&mut`，但把可变范围限制在函数内部，不要泄漏到 API。
- **依赖方向单向**，见 `docs/ARCHITECTURE.md` 与 `docs/modules/README.md`。
  任何反向 `use` 都是设计问题，不是导入问题。
- **一个 `pub` 项只有一个职责**。`fn update_and_render_and_export()` 这种名字
  自己就在报告问题。
- **新增 `pub` API 要问一句"这个必须公开吗"**。默认私有，按需放开。

## 注释

注释解释**为什么**，不解释**是什么**。代码已经说了是什么。

必须写注释的三种情况：

1. **非显然的取舍**——为什么选了这个方案而不是更直白的那个。
2. **不变量**（invariant）——尤其是 `unwrap()` / `expect()` 旁边必须说明
   为什么此处不可能 panic。
3. **坐标系与单位**——本项目最容易出错的地方。
   源码 byte offset、UTF-16 offset、预览坐标、窗口逻辑像素和物理像素都必须标清；
   混用一次就会破坏光标或协作锚点。凡是位置量，注释里写清楚参照系和单位。

```rust
// 好：说明了为什么和 offset 单位
/// 将源码位置转换为协作锚点。`byte_offset` 是 UTF-8 byte offset，
/// 不是编辑器协议使用的 UTF-16 code-unit offset。
///
/// 使用相对位置而非绝对下标，使锚点在其他参与者插入文本后仍能重定位。
pub fn anchor_at(text: &SharedText, byte_offset: usize) -> Result<Anchor, AnchorError> { ... }

// 坏：复述代码
/// 返回 offset 对应的锚点
pub fn anchor_at(text: &SharedText, offset: usize) -> Result<Anchor, AnchorError> { ... }
```

不要写的注释：

- 复述代码的（`// 加一` 配 `i += 1`）
- 分隔装饰（`// ===== 辅助函数 =====`）——那说明文件该拆了
- 变更日志和署名（`// 2026-07-28 张三修改`）——git 的职责
- 注释掉的旧实现

文档注释（`///`）要求：

- 所有 `pub` 项必须有，CI 的 `cargo doc` 门禁会查。
- `Err` 的条件要写明——调用方需要知道什么情况下会失败。
- panic 条件必须写明（`# Panics`）。
- 复杂 API 给一个 `# Examples`，doctest 会跟着跑，顺带当测试用。

`TODO` / `FIXME` 规范：

```rust
// TODO(R3): 支持嵌套矩阵。当前 LaTeX 投影只覆盖一层环境。
// FIXME(R0): CRDT 相对位置在删除后的语义待由赛马测试确认。
```

必须带阶段标记或说明卡在什么上。裸 `// TODO: 优化` 不接受——没有信息量，
也不知道该由谁在什么时候处理。

## 测试

- 结构用 AAA（Arrange-Act-Assert）。
- 命名描述行为，不是函数名：
  `fn local_undo_preserves_remote_insert()` 而非 `fn test_undo()`。
- 格式 round-trip、导出和解析诊断使用 golden/快照测试，改动时 diff 可读。
- 修 bug 先写复现该 bug 的失败测试，再改实现。

## 检查

推送前：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI 门禁：

| 检查 | 工具 | 失败条件 |
|---|---|---|
| 格式 | `cargo fmt` | 不一致 |
| Lint | `cargo clippy` | 任何 warning |
| 复杂度 | `cargo crap` | CRAP > 30 |
| 文件规模 | 脚本 | 任何 `.rs` 超 600 行 |
| 安全 | `cargo audit` | 已知漏洞 |
| 许可证 | `cargo deny` | 不兼容许可证 |
| 覆盖率 | `cargo llvm-cov` | 见下 |
| 文档 | `cargo doc` | 任何 warning |

`scholium-model`、`scholium-history` 和格式解析/投影层是纯逻辑无 IO，
覆盖率要求高于其他 crate——没有“依赖外部环境”的借口。

规模和复杂度阈值由 CI 判定，不靠自觉。lint 配置放 workspace 根 `Cargo.toml`：

```toml
[workspace.lints.clippy]
too_many_lines = "warn"          # 函数过长
cognitive_complexity = "warn"    # 认知复杂度
too_many_arguments = "warn"
unwrap_used = "warn"             # 逼出「为什么不会 panic」的注释
missing_panics_doc = "warn"
missing_errors_doc = "warn"
undocumented_unsafe_blocks = "deny"

[workspace.lints.rust]
missing_docs = "warn"
```

`rustfmt.toml`、`deny.toml`、`rust-toolchain.toml`（固定 MSRV）和 600 行文件规模脚本随首个 crate 一并提交；
当前 workspace 还没有成员，CI 平台与这些配置文件的位置在阶段 0 建立最小核心时确定并更新本节。

有了这些门禁，评审时就能只谈设计和正确性，不用再纠缠风格。
