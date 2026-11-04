//! Project settings edit declaration fields through the locked storage API.
use super::*;
use crate::widgets::{self, DialogButton};
use scholium_storage::project::{BibliographyTool, LatexEngine};

impl ProjectFiles {
    pub(super) fn settings_dialog(
        &mut self,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
    ) {
        let Some(project) = project else {
            self.dialog = None;
            return;
        };
        let locale = state.locale.resolve();
        let text = |key| i18n::text(locale, key);
        let title = format!(
            "{} · {}",
            text("command.document.project_settings.title"),
            project_workspace::name(project.handle())
        );
        let Some(settings) = &mut self.settings else {
            self.dialog = None;
            return;
        };
        let buttons = [
            DialogButton::new("cancel", text("chrome.dialog.cancel"), false),
            DialogButton::new("settings-save", text("command.file.save.title"), true),
        ];
        let root = project.handle().root().to_path_buf();
        let latex = project.handle().language() == ProjectLanguage::Latex;
        let action = widgets::dialog(ctx, "project-settings", &title, locale, &buttons, |ui| {
            ui.label(text("project.settings.entry"));
            let entry = settings.entry.display().to_string();
            choose(ui, "settings-entry", &entry, |ui| {
                for entry in picker::entries(&root) {
                    if entry.extension() == project.handle().entry().extension() {
                        let label = entry.display().to_string();
                        if ui
                            .selectable_value(&mut settings.entry, entry, &label)
                            .clicked()
                        {
                            ui.close();
                        }
                    }
                }
            });
            if latex {
                latex_fields(ui, settings, locale);
            }
            ui.label(text("project.settings.language"));
            ui.label(if latex { "LaTeX" } else { "Typst" });
            if let Some(error) = &self.settings_error {
                ui.colored_label(crate::theme::tokens(ctx).error, error);
            }
        });
        match action.as_deref() {
            Some("cancel") => {
                self.dialog = None;
                self.settings_error = None;
            }
            Some("settings-save") => match project.update_settings(settings.clone()) {
                Ok(()) => {
                    self.dialog = None;
                    self.settings_error = None;
                    project.metadata(state);
                }
                Err(error) => {
                    self.settings_error = Some(text(
                        if matches!(
                            error,
                            scholium_storage::project::StoreError::RecoveryRequired
                        ) {
                            "project.settings.save_first"
                        } else {
                            "project.settings.failed"
                        },
                    ))
                }
            },
            _ => {}
        }
    }
}
fn latex_fields(
    ui: &mut egui::Ui,
    settings: &mut scholium_storage::project::ProjectSettings,
    locale: i18n::Locale,
) {
    let text = |key| i18n::text(locale, key);
    ui.label(text("project.settings.engine"));
    let value = engine_label(settings.engine, locale);
    choose(ui, "engine", &value, |ui| {
        for engine in [
            None,
            Some(LatexEngine::LuaLatex),
            Some(LatexEngine::XeLatex),
            Some(LatexEngine::PdfLatex),
        ] {
            let r = ui.selectable_value(&mut settings.engine, engine, engine_label(engine, locale));
            audit_rect(&format!("engine.{}", engine_label(engine, locale)), r.rect);
            if r.clicked() {
                ui.close();
            }
        }
    });
    ui.label(text("project.settings.engine_help"));
    ui.label(text("project.settings.bibliography"));
    choose(
        ui,
        "bibliography",
        &bibliography_label(settings.bibliography, locale),
        |ui| {
            for tool in [
                BibliographyTool::Auto,
                BibliographyTool::Biber,
                BibliographyTool::Bibtex,
                BibliographyTool::None,
            ] {
                if ui
                    .selectable_value(
                        &mut settings.bibliography,
                        tool,
                        bibliography_label(tool, locale),
                    )
                    .clicked()
                {
                    ui.close();
                }
            }
        },
    );
}

fn choose(ui: &mut egui::Ui, id: &str, value: &str, content: impl FnOnce(&mut egui::Ui)) {
    let response = widgets::field_dropdown(
        ui,
        widgets::Dropdown {
            label: String::new(),
            value: value.into(),
            max_width: ui.available_width(),
            meta: widgets::Meta::new(id, "dialog", value),
        },
    );
    audit_rect(id, response.rect);
    egui::Popup::menu(&response)
        .frame(widgets::popover_frame(ui.ctx()))
        .show(content);
}
fn engine_label(engine: Option<LatexEngine>, locale: i18n::Locale) -> String {
    match engine {
        None => i18n::text(locale, "project.settings.auto"),
        Some(LatexEngine::LuaLatex) => "LuaLaTeX".into(),
        Some(LatexEngine::XeLatex) => "XeLaTeX".into(),
        Some(LatexEngine::PdfLatex) => "pdfLaTeX".into(),
    }
}
fn bibliography_label(tool: BibliographyTool, locale: i18n::Locale) -> String {
    match tool {
        BibliographyTool::Auto => i18n::text(locale, "project.settings.auto"),
        BibliographyTool::Biber => "biber".into(),
        BibliographyTool::Bibtex => "BibTeX".into(),
        BibliographyTool::None => i18n::text(locale, "project.settings.none"),
    }
}
