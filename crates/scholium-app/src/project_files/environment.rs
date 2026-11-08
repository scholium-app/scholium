//! Cached toolchain inspection runs on the detector's worker, including explicit refreshes.
use crate::{i18n, icons::Glyph, theme};
use eframe::egui;
use scholium_latex::toolchain::{DetectionConfig, Message, ToolchainDetector, ToolchainStatus};

#[derive(Default)]
pub(crate) struct Environment {
    detector: Option<ToolchainDetector>,
    initialized: bool,
    refreshed: bool,
}
impl Environment {
    pub(crate) fn take_refresh(&mut self) -> bool {
        std::mem::take(&mut self.refreshed)
    }
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
                let items = summary(&status, locale, cfg!(debug_assertions));
                if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_some() {
                    crate::audit_line::emit(
                        "ENVIRONMENT",
                        &serde_json::json!({"items": items.iter().map(|i| &i.text).collect::<Vec<_>>()}),
                    );
                }
                for (index, item) in items.iter().enumerate() {
                    crate::widgets::row(
                        ui,
                        &crate::widgets::Row {
                            glyph: if item.warning {
                                Glyph::Warning
                            } else {
                                Glyph::Check
                            },
                            tint: if item.warning { t.warning } else { t.text_dim },
                            primary: &item.text,
                            secondary: None,
                            selected: false,
                            secondary_right: false,
                        },
                        crate::widgets::Meta::new(
                            format!("environment.{index}"),
                            "start",
                            &item.text,
                        )
                        .enabled(false),
                    )
                    .on_hover_text(&item.text);
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
                self.refreshed = true;
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
#[derive(Debug)]
struct Item {
    text: String,
    warning: bool,
}
fn item(label: String, action: Option<String>, warning: bool) -> Item {
    Item {
        text: action.map_or_else(|| label.clone(), |action| format!("{label} — {action}")),
        warning,
    }
}
// The detector preserves probe evidence. This projection groups it by user-facing item.
fn summary(status: &ToolchainStatus, locale: i18n::Locale, development: bool) -> Vec<Item> {
    if let Some(issue) = status
        .issues
        .iter()
        .find(|i| i.message.id == "toolchain.detection-failed")
    {
        return vec![item(
            message(locale, &issue.message),
            Some(message(locale, &issue.action)),
            true,
        )];
    }
    let text = |key| i18n::text(locale, key);
    let fonts = item(
        text(if status.font_cache_warm {
            "project.environment.fonts_ready"
        } else {
            "toolchain.font-cache-cold"
        }),
        (!status.font_cache_warm).then(|| text("toolchain.warm-on-preview")),
        !status.font_cache_warm,
    );
    vec![
        distribution_item(status, locale),
        rtex_item(status, locale, development),
        fonts,
    ]
}
fn rtex_item(status: &ToolchainStatus, locale: i18n::Locale, development: bool) -> Item {
    if development {
        let issue = status
            .issues
            .iter()
            .find(|i| i.message.id == "toolchain.override-invalid")
            .or_else(|| {
                status
                    .issues
                    .iter()
                    .find(|i| i.message.id == "toolchain.dev-override")
            });
        if let Some(issue) = issue {
            return item(
                message(locale, &issue.message),
                Some(message(locale, &issue.action)),
                true,
            );
        }
    }
    bundled_rtex_item(status, locale)
}
fn distribution_item(status: &ToolchainStatus, locale: i18n::Locale) -> Item {
    let text = |key| i18n::text(locale, key);
    if !status.tex_live.found {
        return item(
            format!(
                "{}: {}",
                text("project.environment.tex"),
                text("project.environment.missing")
            ),
            Some(text("toolchain.install-tex")),
            true,
        );
    }
    let name = distribution(status.tex_live.version.as_deref().unwrap_or_default());
    let engines = status
        .engines
        .iter()
        .filter(|p| p.version.is_some())
        .map(|p| match p.name {
            "lualatex" => "LuaLaTeX",
            "xelatex" => "XeLaTeX",
            "pdflatex" => "pdfLaTeX",
            name => name,
        })
        .collect::<Vec<_>>()
        .join(&text("project.environment.separator"));
    let label = if engines.is_empty() {
        name
    } else {
        format!("{name} · {engines}")
    };
    let missing = status.engines.iter().any(|p| p.version.is_none());
    item(
        label,
        missing.then(|| text("toolchain.install-tex-component")),
        missing,
    )
}
fn bundled_rtex_item(status: &ToolchainStatus, locale: i18n::Locale) -> Item {
    let text = |key| i18n::text(locale, key);
    let issue = status
        .issues
        .iter()
        .find(|i| i.message.id.starts_with("toolchain.bundle-"));
    let rtex_state = if status.rtex.valid {
        status
            .rtex
            .version
            .clone()
            .unwrap_or_else(|| text("project.environment.bundled"))
    } else if status.rtex.present {
        text("project.environment.invalid")
    } else {
        text("project.environment.missing")
    };
    item(
        format!("{}: {rtex_state}", text("project.environment.rtex")),
        (!status.rtex.valid).then(|| {
            issue.map_or_else(
                || text("toolchain.reinstall"),
                |i| message(locale, &i.action),
            )
        }),
        !status.rtex.valid,
    )
}
fn distribution(banner: &str) -> String {
    for name in ["MacTeX", "TeX Live", "MiKTeX"] {
        if let Some((_, tail)) = banner.rsplit_once(name) {
            let year = tail
                .trim_start()
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .unwrap_or_default();
            return if year.len() == 4 && year.starts_with("20") {
                format!("{name} {year}")
            } else {
                name.into()
            };
        }
    }
    "TeX".into()
}
#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;
