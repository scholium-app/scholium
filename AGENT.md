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
7. **阶段出口优先。** 阶段 0 的结构编辑、源码 reconcile、CRDT、恢复和安全构建验证通过前，不搭生产脚手架，
   不做 AI、CAS、绘图、幻灯片或插件市场。

8. **团队只能同时编辑一种源码语言。** 同一共享项目分支的 LaTeX/Typst 写入受活动语言与 epoch 门禁约束；同语言可多人编辑，编译可并行，不能仅在本机 UI 实现。
9. **混用是完整产品要求。** 正文、公式、图表、宏、模板、跨片段引用都要有验证路径。Raw 保留不等于支持执行；正式输出不得包含 unresolved 占位。见 `docs/MIXED_SOURCE_EDITING.md`。

10. **原生技术栈。** 应用 UI、核心与服务端优先 Rust；必要时引入 C/C++/Zig。禁止 npm/Node.js、JavaScript/TypeScript 编辑器、WebView/Electron/Tauri 作为应用 UI 或验证依赖。HTML 导出是文件能力，不是 UI 实现。可选浏览器 WASM 允许必要的工具生成加载/绑定胶水，不允许 JS/TS 业务编辑器或 npm/Node.js；见 `docs/WASM.md`。
11. **UI 验证顺序固定。** Iced → GPUI → C++ EUI-NEO → Slint 或 egui；按 `docs/NATIVE_UI_VALIDATION.md` 的同一验收集验证，不以演示能运行替代编辑器可行性，也不提前锁定最终框架。

## 设计文档纪律

- `docs/PLAN.md` 只是总览；产品、体验、架构、数据模型、排版转换、协议、历史、格式、安全、测试和路线图各自维护。
- 修改模块前阅读 `docs/modules/<module>.md`；公共接口、不变量或职责发生变化时，同一提交更新文档。
- 第三方核心选型、长期存储格式、协议破坏性变化和跨模块边界调整必须新增 ADR。
- `docs/archive/` 只用于查证废弃原型，不得作为实现规范引用；确需沿用的实验结论应重新验证。
- 设计中的“待验证”必须通过 spike 关闭。不能通过先写生产脚手架、以后再验证来关闭风险。

## 分支与提交

格式：`username/<type>/<description>`，type 取 `feat` / `fix` / `chore` / `refactor` / `docs`。

提交信息：`<type>: <简述>`。一个 PR 聚焦一个主题。推送直接用 `git push`，不用 `gh`。

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

有了这些门禁，评审时就能只谈设计和正确性，不用再纠缠风格。
