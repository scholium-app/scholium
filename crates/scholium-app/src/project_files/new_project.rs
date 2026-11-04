//! New-project form and validation; engine choices belong to project settings.
use super::*;
use crate::widgets::{self, DialogButton};
use std::path::{Component, Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Validation {
    Name,
    Exists,
    Location,
}
impl Validation {
    fn key(self) -> &'static str {
        match self {
            Self::Name => "project.new.name_error",
            Self::Exists => "project.new.exists",
            Self::Location => "project.new.location_error",
        }
    }
}
pub(super) struct NewProject {
    pub name: String,
    pub location: String,
    pub latex: bool,
    pub chinese: bool,
    pub(super) error: Option<Validation>,
}
impl Default for NewProject {
    fn default() -> Self {
        Self {
            name: String::new(),
            location: std::env::current_dir()
                .unwrap_or_default()
                .display()
                .to_string(),
            latex: true,
            chinese: false,
            error: None,
        }
    }
}
impl NewProject {
    pub fn target(&self) -> PathBuf {
        Path::new(self.location.trim()).join(self.name.trim())
    }
    pub fn validate(&self) -> Result<(), Validation> {
        let name = self.name.trim();
        let mut components = Path::new(name).components();
        if name.is_empty()
            || name.contains(['/', '\\', ':'])
            || !matches!(components.next(), Some(Component::Normal(_)))
            || components.next().is_some()
        {
            return Err(Validation::Name);
        }
        if self.target().try_exists().unwrap_or(true) {
            return Err(Validation::Exists);
        }
        let parent = Path::new(self.location.trim());
        if !parent.is_dir()
            || parent
                .metadata()
                .map_or(true, |m| m.permissions().readonly())
            || tempfile::NamedTempFile::new_in(parent).is_err()
        {
            return Err(Validation::Location);
        }
        Ok(())
    }
    fn language_template(&mut self, ui: &mut egui::Ui, locale: i18n::Locale) {
        let text = |key| i18n::text(locale, key);
        ui.label(text("project.language"));
        let latex = ui.radio_value(&mut self.latex, true, text("project.new.latex"));
        audit_rect("latex", latex.rect);
        let typst = ui.radio_value(&mut self.latex, false, text("project.new.typst"));
        audit_rect("typst", typst.rect);
        widgets::field_gap(ui);
        ui.label(text("project.new.template"));
        let value = text(if !self.latex {
            "project.new.note"
        } else if self.chinese {
            "project.new.ctex"
        } else {
            "project.new.article"
        });
        let response = widgets::field_dropdown(
            ui,
            widgets::Dropdown {
                label: String::new(),
                value,
                max_width: ui.available_width(),
                meta: widgets::Meta::new(
                    "project.template",
                    "dialog",
                    text("project.new.template"),
                ),
            },
        );
        egui::Popup::menu(&response)
            .frame(widgets::popover_frame(ui.ctx()))
            .show(|ui| {
                for (chinese, key) in if self.latex {
                    vec![(false, "project.new.article"), (true, "project.new.ctex")]
                } else {
                    vec![(false, "project.new.note")]
                } {
                    if ui
                        .selectable_label(self.chinese == chinese, text(key))
                        .clicked()
                    {
                        self.chinese = chinese;
                        ui.close();
                    }
                }
            });
    }
    fn error_text(&self, locale: i18n::Locale, name: bool) -> Option<String> {
        self.error
            .filter(|e| matches!(e, Validation::Name | Validation::Exists) == name)
            .map(|e| i18n::text(locale, e.key()))
    }
}
impl ProjectFiles {
    pub(super) fn new_dialog(
        &mut self,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
    ) {
        let locale = state.locale.resolve();
        let text = |key| i18n::text(locale, key);
        let buttons = [
            DialogButton::new("cancel", text("chrome.dialog.cancel"), false),
            DialogButton::new("submit", text("project.new.create"), true),
        ];
        let mut browse = false;
        let action = widgets::dialog(
            ctx,
            "project-chooser",
            &text("project.new.title"),
            locale,
            &buttons,
            |ui| {
                self.new.language_template(ui, locale);
                widgets::field_gap(ui);
                ui.label(text("project.new.name"));
                let error = self.new.error_text(locale, true);
                let name = widgets::text_input(ui, &mut self.new.name, error.as_deref());
                audit_rect("name", name.rect);
                widgets::field_gap(ui);
                ui.label(text("project.new.location"));
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 100.0).max(80.0);
                    ui.allocate_ui(egui::vec2(width, 28.0), |ui| {
                        let path = widgets::text_input(ui, &mut self.new.location, None);
                        audit_rect("path", path.rect);
                        if path.changed() {
                            self.new.error = None;
                        }
                    });
                    let b = widgets::button(ui, &text("project.browse"), false, true);
                    audit_rect("browse", b.rect);
                    browse = b.clicked();
                });
                if let Some(error) = self.new.error_text(locale, false) {
                    ui.colored_label(crate::theme::tokens(ctx).error, error);
                }
                if name.changed() {
                    self.new.error = self.new.validate().err();
                }
                widgets::field_gap(ui);
                ui.label(text("project.new.preview"));
                let entry = if self.new.latex {
                    "main.tex"
                } else {
                    "main.typ"
                };
                ui.label(self.new.target().join(entry).display().to_string());
            },
        );
        if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_some() {
            println!(
                "PROJECT_FORM {}",
                serde_json::json!({"latex":self.new.latex,"name":self.new.name,"location":self.new.location,"error":self.new.error.map(Validation::key)})
            );
        }
        if browse {
            self.picker = Some((
                picker::Purpose::Location,
                Box::new(picker::FallbackPicker::folder(&self.new.location)),
            ));
        }
        match action.as_deref() {
            Some("cancel") => self.dialog = None,
            Some("submit") => match self.new.validate() {
                Err(e) => self.new.error = Some(e),
                Ok(()) => {
                    self.dialog = None;
                    let language = if self.new.latex {
                        ProjectLanguage::Latex
                    } else {
                        ProjectLanguage::Typst
                    };
                    self.request(
                        Operation::Create(
                            self.new.target(),
                            language,
                            self.new.chinese && self.new.latex,
                        ),
                        ctx,
                        state,
                        project,
                    );
                }
            },
            _ => {}
        }
    }
}
