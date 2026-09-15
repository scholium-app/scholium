//! Iced 候选的最小工程。
//!
//! 目标（`docs/NATIVE_UI_VALIDATION.md` §4）：原生窗口 + 正文/源码/预览三个区域 + 可替换 UI 适配层，
//! 复用 `scholium-spike-core` 的同一组验收脚本。
//!
//! 当前状态：骨架。核心已在 `../core` 通过模型层验收；Iced 适配层待实现。

fn main() {
    eprintln!(
        "candidate-iced: 骨架就绪，尚未接入 Iced 适配层。\
         先运行 `cargo test --manifest-path ../core/Cargo.toml` 复核核心验收。"
    );
    std::process::exit(2);
}
