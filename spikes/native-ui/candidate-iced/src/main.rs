//! Iced 候选的可执行入口。
//!
//! 应用逻辑在 `scholium_spike_iced` 库里，这样同一段消息处理代码可以被测试直接驱动
//! （见 `tests/input.rs`）。

use iced::Font;
use scholium_spike_iced::{App, CJK_FAMILY, load_cjk_font};

/// 启动候选应用。
pub fn main() -> iced::Result {
    let font_bytes = load_cjk_font();
    let font_loaded = font_bytes.is_some();

    let mut application = iced::application(move || App::boot(font_loaded), App::update, App::view)
        .title("Scholium spike — Iced candidate")
        .window_size((1280.0, 820.0))
        .subscription(App::subscription);

    if let Some(bytes) = font_bytes {
        application = application
            .font(bytes)
            .default_font(Font::with_name(CJK_FAMILY));
    }

    application.run()
}
