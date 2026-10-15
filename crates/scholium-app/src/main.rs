//! Scholium 原生排版工作区：窗口外壳（菜单栏、工具栏、焦点工具栏、标签栏、状态栏）与
//! Typst Content 编辑面共用一个结构会话。

mod chrome;
mod command;
mod commands;
mod i18n;
mod icons;
mod state;
mod theme;
mod typst_editor;
mod widgets;

use eframe::egui;
use state::WorkspaceState;
use typst_editor::EditorSession;

/// Narrowest window: at this width the toolbar already moves its last group into "»".
const MIN_WINDOW_WIDTH: f32 = 480.0;

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
        command::shortcuts::shortcuts(ui.ctx(), &mut self.state);
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
            .with_min_inner_size([MIN_WINDOW_WIDTH, 480.0]),
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
