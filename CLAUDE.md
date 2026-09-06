# Scholium 开发规范

实施方案见 [docs/PLAN.md](docs/PLAN.md)。动手前先读 §2（排版管线）和 §6（AI 接口）。

## 硬性约束

1. **不移植 `../mogan` 或 `../mogan-rs` 的任何代码。** 纯重写。
   移植 TeXmacs 系代码会把许可证锁死到 GPL-3，也会带回旧架构的问题。
2. **Typst 是排版库，不是导出器。** 不要引入「生成 Typst 源码 → 渲染 SVG → 塞进控件」
   的思路，那是旧方案失败的根因。
3. **所有编辑走 `EditOp` / `Transaction` 通道。** 键盘、菜单、AI 一视同仁。
   不要给任何模块 AST 的裸可变引用。
4. **P4 结束前不碰格式转换、AI provider 实现；文献只做 P6 的最小版**
   （`.bib` + `\cite` + GB/T 7714）。对照 Liii STEM 补出的 P3 文档结构阶段
   同样要克制：只做 AST + 序列化 + 基本交互。前两份计划都是被范围拖死的。

## 分支与提交

格式：`username/<type>/<description>`，type 取 `feat` / `fix` / `chore` / `refactor` / `docs`。

提交信息：`<type>: <简述>`。一个 PR 聚焦一个主题。推送直接用 `git push`，不用 `gh`。

## Rust 规范

- Edition 2024，MSRV 1.92（typst 0.15 要求）
- `pub` 类型须实现 `Debug`
- 有 `new()` 须同步实现 `Default`
- 错误用 `thiserror`，不用裸 `String`
- 异步用 `tokio`

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
- **依赖方向单向**，见 PLAN.md §4。任何反向 `use` 都是设计问题，不是导入问题。
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
   Typst 的 `Abs`/`Em`、vello 的物理像素、winit 的逻辑像素、
   frame 局部坐标 vs 页面坐标，混用一次就要 debug 半天。
   凡是几何量，注释里写清楚参照系和单位。

```rust
// 好：说明了为什么和参照系
/// 命中测试。`pos` 是页面坐标（左上原点，Abs 单位），
/// 不是窗口坐标——调用方负责先减去滚动偏移。
///
/// 逐字形线性扫描而非空间索引：单页字形量级在 1e3，
/// 实测比构建 R-tree 更快，且省掉了增量维护索引的复杂度。
pub fn hit_test(frame: &Frame, pos: Point) -> Option<NodeId> { ... }

// 坏：复述代码
/// 遍历 frame 的 items，返回 NodeId
pub fn hit_test(frame: &Frame, pos: Point) -> Option<NodeId> { ... }
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
// TODO(P2): 支持嵌套矩阵。当前 hit_test 只递归一层 Group。
// FIXME: ligature 内偏移语义待确认，见 PLAN.md §2 未确认项。
```

必须带阶段标记或说明卡在什么上。裸 `// TODO: 优化` 不接受——没有信息量，
也不知道该由谁在什么时候处理。

## 测试

- 结构用 AAA（Arrange-Act-Assert）。
- 命名描述行为，不是函数名：
  `fn cursor_enters_denominator_on_down_arrow()` 而非 `fn test_cursor()`。
- 排版和渲染的正确性用快照测试，改动时 diff 可读。
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

`scholium-doc`、`scholium-serialize`、`scholium-agent` 是纯逻辑无 IO，
覆盖率要求高于其他 crate——没有"依赖外部环境"的借口。

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
