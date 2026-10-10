//! Scholium 原生排版工作区：Ribbon 与 Typst Content 编辑面共用一个结构会话。

mod chrome;
mod commands;
mod icons;
mod ribbon;
mod state;
mod theme;
mod typst_editor;

use eframe::egui;
use state::WorkspaceState;
use typst_editor::EditorSession;

#[derive(Default)]
struct ScholiumApp {
    state: WorkspaceState,
    editor: Option<EditorSession>,
    /// The document could not be opened; shown in the window, no editor runs.
    startup_error: Option<String>,
}

impl eframe::App for ScholiumApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Some(error) = &self.startup_error {
            typst_editor::show_startup_error(ui, error);
            return;
        }
        commands::shortcuts(ui.ctx(), &mut self.state);
        chrome::show(ui, &mut self.state);
        if let Some(editor) = &mut self.editor {
            editor.show(ui, &mut self.state);
        }
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Scholium")
            .with_inner_size([1280.0, 900.0])
            .with_min_inner_size([640.0, 480.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Scholium",
        options,
        Box::new(|cc| {
            let mut app = ScholiumApp::default();
            theme::install(&cc.egui_ctx);
            match EditorSession::open(&cc.egui_ctx, &mut app.state) {
                Ok(editor) => app.editor = Some(editor),
                Err(error) => {
                    eprintln!("{error}");
                    app.startup_error = Some(error.to_string());
                }
            }
            Ok(Box::new(app))
        }),
    )
}
