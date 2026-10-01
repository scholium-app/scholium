//! Isolated native editor validation: actual Typst pixels, no committed-text echo.

mod geometry;
// The CLI also uses save_frame; this executable uses the in-memory page directly.
#[allow(dead_code)]
mod kernel;
mod native;
mod session;

fn main() -> eframe::Result {
    eframe::run_native(
        "Typst edit kernel",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_title("Typst edit kernel")
                .with_inner_size([1040.0, 500.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(native::EditorWindow::new(cc.egui_ctx.clone())))),
    )
}
