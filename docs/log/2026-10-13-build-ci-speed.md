# 构建、测试与 CI 提速开发日志（2026-10-13）

## 问题
- main 最近一次 CI：`check` 252 s，`typst-editor-window` 464 s；无任何构建缓存。
- `on: [push, pull_request]` 使每次 PR 推送跑两遍。
- `rust.yml` 有 11 个近乎相同的 `xvfb-run` 步骤，每个新窗口检查都要改它，是并行 PR 的冲突热点。
- 测试在未优化的 Typst 上跑：`cargo test --workspace` 本地暖缓存约 95 s。
- 并行任务同时跑窗口检查产生资源争用失败。

## 改动
- `Cargo.toml`：`[profile.dev.package."*"]` 与 `[profile.test.package."*"]` 设 `opt-level = 2`。
- `rust.yml`：`push` 仅 `main`；concurrency 组（仅 PR 取消旧运行）；两个 job 接入 `Swatinem/rust-cache@v2`
  （窗口 job 同时缓存 `target/stock-reference`）；所有逐检查步骤替换为一步 `scripts/window-checks.py`。
- 新增 `scripts/window-checks.py`（自动发现、顺序、不重试、机器级 flock、剥离 DISPLAY/WAYLAND_DISPLAY）。
- `spikes/typst-main-editor/README.md`、`docs/TESTING.md` 记录用法。

## 验证
- 本地 `cargo test --workspace` 暖缓存（第二次）：改前 95.6 s（首次 88.5 s）。改后 19.3 s（首次 26.7 s），全部测试通过。
- `scripts/window-checks.py` 12/12 通过，含从未进过 CI 的 `a4-surface`（6.3 s，通过，保持被发现）。
  耗时：sharp-raster 62 s、caret-follow 36 s、native 17 s，其余 5–11 s。

## CI 计时
见 PR 描述（冷缓存与暖缓存两次运行）。
