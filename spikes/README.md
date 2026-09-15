# Spike 代码

本目录保存阶段 0 验证的可执行代码，每项验证一个子目录：`spikes/<name>/`。

- 使用 Rust 或必要原生代码构建，遵守 `AGENT.md` 的硬性约束：不使用 npm/Node.js、
  JavaScript/TypeScript 编辑器或 WebView/Electron/Tauri。
- 验证代码不进入正式 workspace 的 crate 依赖图，也不作为生产脚手架。
- 每个子目录必须说明对应的验证项，并在 `docs/spikes/` 留下报告。
- 结论为 `Fail` 或验证结束后的临时代码应删除或明确标注归档，不要留在仓库里腐烂。

报告与规则见 [docs/spikes/README.md](../docs/spikes/README.md)。
