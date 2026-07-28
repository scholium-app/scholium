# Scholium 开发规范

实施方案见 [docs/PLAN.md](docs/PLAN.md)。动手前先读 §2（排版管线）和 §6（AI 接口）。

## 硬性约束

1. **不移植 `../mogan` 或 `../mogan-rs` 的任何代码。** 纯重写。
   移植 TeXmacs 系代码会把许可证锁死到 GPL-3，也会带回旧架构的问题。
2. **Typst 是排版库，不是导出器。** 不要引入「生成 Typst 源码 → 渲染 SVG → 塞进控件」
   的思路，那是旧方案失败的根因。
3. **所有编辑走 `EditOp` / `Transaction` 通道。** 键盘、菜单、AI 一视同仁。
   不要给任何模块 AST 的裸可变引用。
4. **P3 结束前不碰格式转换、文献管理、AI provider 实现。**
   前两份计划都是被范围拖死的。

## 分支与提交

格式：`username/<type>/<description>`，type 取 `feat` / `fix` / `chore` / `refactor` / `docs`。

提交信息：`<type>: <简述>`。一个 PR 聚焦一个主题。推送直接用 `git push`，不用 `gh`。

## Rust 规范

- Edition 2024，MSRV 1.92（typst 0.15 要求）
- `pub` 类型须实现 `Debug`
- 有 `new()` 须同步实现 `Default`
- 错误用 `thiserror`，不用裸 `String`
- 异步用 `tokio`

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
| 安全 | `cargo audit` | 已知漏洞 |
| 许可证 | `cargo deny` | 不兼容许可证 |
| 覆盖率 | `cargo llvm-cov` | 见下 |
| 文档 | `cargo doc` | 任何 warning |

`scholium-doc`、`scholium-serialize`、`scholium-agent` 是纯逻辑无 IO，
覆盖率要求高于其他 crate——没有"依赖外部环境"的借口。
