//! Flat, keyboard-accessible start page; recent entries are inspected without opening a store.
use super::*;
use crate::{
    command::ui as command_ui,
    icons::Glyph,
    theme::{self, Tokens},
    widgets::{self, Meta},
};

impl ProjectFiles {
    pub(crate) fn start_page(
        &mut self,
        parent: &mut egui::Ui,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
    ) {
        let ctx = parent.ctx().clone();
        ctx.send_viewport_cmd(egui::ViewportCommand::Title("Scholium".into()));
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::tokens(&ctx).canvas)
                    .inner_margin(Tokens::PAGE_TOP as i8),
            )
            .show(parent, |ui| {
                let width = ui.available_width().min(560.0);
                let left = ((ui.available_width() - width) / 2.0).max(0.0);
                let mut rect = ui.available_rect_before_wrap();
                rect.min.x += left;
                rect.max.x = rect.min.x + width;
                rect.min.y += Tokens::PAGE_TOP;
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.add_enabled_ui(!state.file_modal, |ui| {
                                self.start_contents(ui, state, project)
                            });
                        });
                    },
                );
            });
        if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_some() {
            println!(
                "PROJECT {}",
                serde_json::json!({"closed":true,"controls":widgets::audit::controls_json(&ctx),"chrome":widgets::audit::facts_json(&ctx),"locked":self.locked,"recent":self.recent})
            );
        }
    }
    fn start_contents(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
    ) {
        let locale = state.locale.resolve();
        let text = |key| i18n::text(locale, key);
        let mut ids = vec![
            "start.new".to_owned(),
            "start.open".into(),
            "start.import".into(),
        ];
        for index in 0..self.recent.len() {
            ids.push(format!("recent.{index}"));
            ids.push(format!("remove.{index}"));
        }
        if self.locked.is_some() {
            ids.push("start.retry".into());
        }
        ids.push("environment.refresh".into());
        navigate(ui.ctx(), &ids);
        if let Some(path) = self.locked.clone() {
            ui.label(text("project.locked.title"));
            ui.label(path.display().to_string());
            if list_row(ui, "start.retry", &text("project.retry"), None, Glyph::Undo).clicked() {
                self.request(
                    Operation::Open(path, None),
                    &ui.ctx().clone(),
                    state,
                    project,
                );
            }
            ui.add_space(Tokens::PAGE_TOP);
        } else if let Some(error) = &state.storage_error {
            ui.label(error);
        }
        ui.heading("Scholium");
        ui.label(text("project.start.description"));
        ui.add_space(Tokens::PAGE_TOP);
        ui.label(text("project.start.begin"));
        for (id, command, glyph) in [
            ("start.new", "file.new", Glyph::Plus),
            ("start.open", "file.open", Glyph::Open),
            ("start.import", "file.import", Glyph::Page),
        ] {
            if let Some(resolved) = command_ui::resolve(state, command) {
                let response = list_row(
                    ui,
                    id,
                    &resolved.title(state),
                    resolved.shortcut().as_deref(),
                    glyph,
                );
                if response.clicked() {
                    widgets::consume_activation(ui.ctx());
                    command_ui::invoke(&ui.ctx().clone(), state, command);
                }
            }
        }
        ui.add_space(Tokens::PAGE_TOP);
        ui.label(text("project.start.recent"));
        if self.recent.is_empty() {
            ui.label(text("project.recent.empty"));
        }
        self.recent_rows(ui, state, project);
        ui.add_space(Tokens::PAGE_TOP);
        self.environment.show(ui, locale);
    }
    fn recent_rows(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
    ) {
        let locale = state.locale.resolve();
        let text = |key| i18n::text(locale, key);
        for (index, path) in self.recent.clone().into_iter().enumerate() {
            ui.horizontal(|ui| {
                let width =
                    (ui.available_width() - Tokens::ICON_BUTTON - ui.spacing().item_spacing.x)
                        .max(1.0);
                ui.allocate_ui(egui::vec2(width, Tokens::STATUS_BAR), |ui| {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    let primary = format!("{name}   {}", recent::display(&path));
                    let language = recent_language(&path);
                    if list_row(
                        ui,
                        &format!("recent.{index}"),
                        &primary,
                        Some(language),
                        Glyph::Page,
                    )
                    .clicked()
                    {
                        widgets::consume_activation(ui.ctx());
                        self.request(
                            Operation::Open(path.clone(), None),
                            &ui.ctx().clone(),
                            state,
                            project,
                        );
                    }
                });
                let name = format!(
                    "{} {}",
                    text("project.recent.remove"),
                    recent::display(&path)
                );
                let response = widgets::icon_button(
                    ui,
                    widgets::IconButton {
                        glyph: Glyph::Close,
                        size: Tokens::ICON_STATUS,
                        active: false,
                        meta: Meta::new(format!("remove.{index}"), "start", name),
                    },
                );
                audit_rect(&format!("remove.{index}"), response.rect);
                if response.clicked()
                    && let Some(file) = recent::location()
                    && let Err(e) = recent::remove(&mut self.recent, &path, &file)
                {
                    state.storage_error = Some(e);
                }
            });
        }
    }
    pub(super) fn lock_dialog(
        &mut self,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
    ) {
        let Some(path) = self.locked.clone().filter(|_| project.is_some()) else {
            return;
        };
        let locale = state.locale.resolve();
        let text = |key| i18n::text(locale, key);
        let buttons = [
            widgets::DialogButton::new("cancel", text("chrome.dialog.cancel"), false),
            widgets::DialogButton::new("retry", text("project.retry"), true),
        ];
        let action = widgets::dialog(
            ctx,
            "project-locked",
            &text("project.locked.title"),
            locale,
            &buttons,
            |ui| {
                ui.label(path.display().to_string());
            },
        );
        match action.as_deref() {
            Some("cancel") => self.locked = None,
            Some("retry") => self.request(Operation::Open(path, None), ctx, state, project),
            _ => {}
        }
        state.file_modal |= self.locked.is_some();
    }
    pub(crate) fn startup_error(&mut self, error: &str) {
        if error.contains("locked")
            || error == i18n::text(i18n::LocaleSetting::Auto.resolve(), "project.locked")
        {
            self.locked = std::env::var_os("SCHOLIUM_PROJECT_PATH")
                .map(PathBuf::from)
                .or_else(|| {
                    scholium_storage::default_document_path()
                        .ok()
                        .map(|p| p.with_file_name("default-project"))
                });
        }
    }
}
fn recent_language(path: &std::path::Path) -> &'static str {
    let declaration = std::fs::read_to_string(path.join(".scholium/project.toml"))
        .ok()
        .and_then(|s| s.parse::<toml::Value>().ok());
    match declaration
        .as_ref()
        .and_then(|v| v.get("language"))
        .and_then(toml::Value::as_str)
    {
        Some("Latex") => "LaTeX",
        Some("Typst") => "Typst",
        _ => "—",
    }
}
pub(super) fn list_row(
    ui: &mut egui::Ui,
    id: &str,
    primary: &str,
    secondary: Option<&str>,
    glyph: Glyph,
) -> egui::Response {
    let t = theme::tokens(ui.ctx());
    let name = format!("{primary} {}", secondary.unwrap_or_default());
    let r = widgets::row(
        ui,
        &widgets::Row {
            glyph,
            tint: t.text_dim,
            primary,
            secondary,
            selected: false,
            secondary_right: true,
        },
        Meta::new(id, "start", name),
    );
    audit_rect(id, r.rect);
    r
}
fn navigate(ctx: &egui::Context, ids: &[String]) {
    let focused = ctx.memory(|m| m.focused());
    let current = ids
        .iter()
        .position(|id| Some(Meta::new(id, "start", id).id_for()) == focused);
    let directions = ctx.input_mut(|input| {
        let mut directions = Vec::new();
        input.events.retain(|event| {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            else {
                return true;
            };
            if *modifiers != egui::Modifiers::NONE {
                return true;
            }
            let delta = match key {
                egui::Key::ArrowDown => 1,
                egui::Key::ArrowUp => -1,
                _ => return true,
            };
            directions.push(delta);
            false
        });
        directions
    });
    let mut next = current;
    for delta in &directions {
        next = Some(next.map_or(0, |n| {
            (n as isize + delta).rem_euclid(ids.len() as isize) as usize
        }));
    }
    if !directions.is_empty()
        && let Some(index) = next
    {
        let id = Meta::new(&ids[index], "start", &ids[index]).id_for();
        ctx.memory_mut(|m| {
            // egui queues spatial arrow navigation before we consume the events.
            m.move_focus(egui::FocusDirection::None);
            m.request_focus(id);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_navigation_keeps_every_arrow_press_in_one_frame() {
        let ctx = egui::Context::default();
        let ids: Vec<String> = [
            "start.new",
            "start.open",
            "start.import",
            "recent.0",
            "remove.0",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let frame = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    navigate(&ctx, &ids);
                    for id in &ids {
                        Meta::new(id, "start", id).place(ui, egui::vec2(200.0, 28.0));
                    }
                },
            );
            output.textures_delta.clear();
        };
        frame(vec![]);
        frame(vec![]);
        let down = egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame(vec![down.clone(); 4]);
        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(Meta::new("recent.0", "start", "recent.0").id_for())
        );
        frame(vec![down]);
        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(Meta::new("remove.0", "start", "remove.0").id_for())
        );
    }
}
