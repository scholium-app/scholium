//! Scholium 原生排版工作区：窗口外壳（菜单栏、工具栏、焦点工具栏、标签栏、状态栏）与
//! Typst Content 编辑面共用一个结构会话。

mod chrome;
mod command;
mod commands;
mod diagnostics;
mod i18n;
mod icons;
mod latex_workspace;
mod project_files;
mod project_workspace;
mod source_editor;
mod state;
mod theme;
mod typst_editor;
mod widgets;

use eframe::egui;
use project_workspace::ProjectWorkspace;
use state::WorkspaceState;

/// Narrowest window: at this width the toolbar already moves its last group into "»".
const MIN_WINDOW_WIDTH: f32 = 480.0;

#[derive(Default)]
struct ScholiumApp {
    state: WorkspaceState,
    project: Option<ProjectWorkspace>,
    files: project_files::ProjectFiles,
    notice: bool,
}

impl eframe::App for ScholiumApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        command::shortcuts::shortcuts(ui.ctx(), &mut self.state);
        if let Some(project) = &self.project {
            project.metadata(&mut self.state);
        }
        chrome::show(ui, &mut self.state);
        self.files
            .show(ui.ctx(), &mut self.state, &mut self.project);
        if let Some(project) = &mut self.project {
            project.show(ui, &mut self.state);
        } else {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Title("Scholium".into()));
            egui::CentralPanel::default().show(ui, |ui| {
                ui.heading(i18n::text(self.state.locale.resolve(), "project.welcome"));
                if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_some() {
                    println!("PROJECT {{\"closed\":true}}");
                }
                if let Some(error) = &self.state.storage_error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            });
        }
        if self.notice {
            egui::Window::new(i18n::text(
                self.state.locale.resolve(),
                "project.migrated.title",
            ))
            .show(ui.ctx(), |ui| {
                ui.label(i18n::text(
                    self.state.locale.resolve(),
                    "project.migrated.body",
                ));
                if ui
                    .button(i18n::text(self.state.locale.resolve(), "project.ok"))
                    .clicked()
                {
                    self.notice = false;
                }
            });
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
            app.files.initialize(&mut app.state);
            match typst_editor::default_project().and_then(|(handle, migrated)| {
                app.notice = migrated;
                ProjectWorkspace::open(handle, &cc.egui_ctx, &mut app.state)
            }) {
                Ok(project) => {
                    app.files.remember(&project, &mut app.state);
                    app.project = Some(project);
                }
                Err(error) => {
                    eprintln!("{error}");
                    app.state.storage_error = Some(error);
                }
            }
            Ok(Box::new(app))
        }),
    )
}
