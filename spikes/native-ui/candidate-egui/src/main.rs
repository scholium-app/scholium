//! egui 候选的可执行入口。
//!
//! 应用逻辑在 `scholium_spike_egui` 库里，这样同一段输入处理代码可以在无窗口条件下被测试驱动
//! （见 `tests/input.rs`）。

use eframe::egui;
use scholium_spike_egui::{SpikeApp, load_cjk_font};

/// 启动候选应用。
fn main() -> eframe::Result {
    let font = load_cjk_font();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_title("Scholium spike — egui candidate"),
        ..Default::default()
    };
    eframe::run_native(
        "Scholium spike — egui candidate",
        options,
        Box::new(move |cc| Ok(Box::new(SpikeApp::new(&cc.egui_ctx, font)))),
    )
}
