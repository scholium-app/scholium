//! Cached toolchain inspection runs on the detector's worker, including explicit refreshes.
use crate::{i18n, icons::Glyph, theme};
use eframe::egui;
use scholium_latex::toolchain::{DetectionConfig, Message, ToolchainDetector};

#[derive(Default)]
pub(super) struct Environment {
    detector: Option<ToolchainDetector>,
    initialized: bool,
}
impl Environment {
    pub fn show(&mut self, ui: &mut egui::Ui, locale: i18n::Locale) {
        let text = |key| i18n::text(locale, key);
        if !self.initialized {
            self.detector = DetectionConfig::from_environment()
                .ok()
                .map(ToolchainDetector::new);
            self.initialized = true;
        }
        ui.label(text("project.start.environment"));
        let t = theme::tokens(ui.ctx());
        if let Some(detector) = &self.detector {
            if let Some(status) = detector.cached() {
                ui.label(format!(
                    "{}: {}",
                    text("project.environment.tex"),
                    status
                        .tex_live
                        .version
                        .as_deref()
                        .unwrap_or(&text("project.environment.missing"))
                ));
                let engines = status
                    .engines
                    .iter()
                    .map(|p| format!("{} {}", p.name, if p.path.is_some() { "✓" } else { "—" }))
                    .collect::<Vec<_>>()
                    .join(" · ");
                ui.label(engines);
                let rtex = if status.rtex.valid {
                    "project.environment.bundled"
                } else {
                    "project.environment.missing"
                };
                ui.label(format!(
                    "rtex {}: {}",
                    status.rtex.version.as_deref().unwrap_or("—"),
                    text(rtex)
                ));
                for issue in &status.issues {
                    ui.colored_label(
                        t.text_dim,
                        format!(
                            "{} — {}",
                            message(locale, &issue.message),
                            message(locale, &issue.action)
                        ),
                    );
                }
            } else {
                ui.label(text("project.environment.checking"));
            }
            if super::start_page::list_row(
                ui,
                "environment.refresh",
                &text("project.environment.refresh"),
                None,
                Glyph::Undo,
            )
            .clicked()
            {
                detector.refresh();
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(150));
        } else {
            ui.label(text("toolchain.executable-unavailable"));
        }
    }
}
fn message(locale: i18n::Locale, message: &Message) -> String {
    let arguments: Vec<_> = message
        .arguments
        .iter()
        .map(|(key, value)| (key.as_str(), value.clone().into()))
        .collect();
    i18n::text_with(locale, message.id, &arguments)
}
#[cfg(test)]
mod tests {
    #[test]
    fn all_toolchain_messages_are_localized() {
        for locale in crate::i18n::Locale::ALL {
            for key in super::super::EXTRA_CATALOG_KEYS
                .iter()
                .filter(|k| k.starts_with("toolchain."))
            {
                assert!(crate::i18n::defines(locale, key), "{key}");
            }
        }
    }
}
