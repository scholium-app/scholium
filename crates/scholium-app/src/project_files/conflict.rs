//! Read-only disk/draft comparison; saving continues to use storage's exact-byte conflict gate.
use super::*;
use crate::widgets::{self, DialogButton};
use std::time::SystemTime;
use time::{OffsetDateTime, UtcOffset};

impl ProjectFiles {
    pub(super) fn conflict_dialog(
        &mut self,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
    ) {
        let Some(source) = project.as_mut().and_then(ProjectWorkspace::source_mut) else {
            self.conflict = false;
            return;
        };
        let locale = state.locale.resolve();
        let text = |key| i18n::text(locale, key);
        let path = source.handle().root().join(source.handle().entry());
        let when = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|time| timestamp(time, SystemTime::now(), locale))
            .unwrap_or_else(|| text("project.conflict.time_unknown"));
        let title = i18n::text_with(
            locale,
            "project.conflict.title",
            &[
                ("file", source.handle().entry().display().to_string().into()),
                ("when", when.into()),
            ],
        );
        let mut diff = DialogButton::new("diff", text("project.conflict.diff"), false);
        diff.left = true;
        let buttons = [
            diff,
            DialogButton::new("cancel", text("chrome.dialog.cancel"), false),
            DialogButton::new("reload", text("project.reload"), false),
            DialogButton::new("save-as", text("command.file.save_as.title"), true),
        ];
        let action = widgets::dialog(ctx, "project-conflict", &title, locale, &buttons, |ui| {
            ui.label(text("project.conflict.refused"));
            widgets::field_gap(ui);
            ui.label(text("project.conflict.help"));
            if self.diff {
                widgets::field_gap(ui);
                match std::fs::read_to_string(&path) {
                    Ok(disk) => {
                        let diff = unified_diff(&disk, source.text());
                        egui::ScrollArea::both().max_height(240.0).show(ui, |ui| {
                            ui.monospace(diff);
                        });
                    }
                    Err(_) => {
                        ui.label(text("project.conflict.read_error"));
                    }
                }
            }
        });
        match action.as_deref() {
            Some("diff") => self.diff = !self.diff,
            Some("cancel") => {
                self.conflict = false;
                self.diff = false;
            }
            Some("reload") => {
                if let Some(p) = project {
                    match p.discard() {
                        Ok(()) => {
                            state.storage_error = None;
                            self.conflict = false;
                            self.diff = false;
                        }
                        Err(e) => state.storage_error = Some(e),
                    }
                }
            }
            Some("save-as") => {
                self.conflict = false;
                self.diff = false;
                self.dialog = Some(FileDialog::SaveAs);
                self.path.clear();
            }
            _ => {}
        }
    }
}
fn unified_diff(disk: &str, draft: &str) -> String {
    let mut result = format!(
        "--- disk\n+++ draft\n@@ -1,{} +1,{} @@\n",
        disk.lines().count(),
        draft.lines().count()
    );
    for (prefix, text) in [("-", disk), ("+", draft)] {
        for line in text.split_inclusive('\n') {
            result.push_str(prefix);
            result.push_str(line);
            if !line.ends_with('\n') {
                result.push_str("\n\\ No newline at end of file\n");
            }
        }
    }
    result
}
fn timestamp(time: SystemTime, now: SystemTime, locale: i18n::Locale) -> Option<String> {
    fn local(time: SystemTime) -> Option<OffsetDateTime> {
        let utc = OffsetDateTime::from(time);
        Some(utc.to_offset(UtcOffset::local_offset_at(utc).ok()?))
    }
    Some(friendly_time(local(time)?, local(now)?, locale))
}
fn friendly_time(time: OffsetDateTime, now: OffsetDateTime, locale: i18n::Locale) -> String {
    let days = (now.date() - time.date()).whole_days();
    let day = match days {
        0 => i18n::text(locale, "project.conflict.today"),
        1 => i18n::text(locale, "project.conflict.yesterday"),
        _ => format!(
            "{:04}-{:02}-{:02}",
            time.year(),
            u8::from(time.month()),
            time.day()
        ),
    };
    format!("{day} {:02}:{:02}", time.hour(), time.minute())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unified_diff_preserves_unicode_and_marks_missing_newline() {
        assert_eq!(
            unified_diff("旧\n", "新"),
            "--- disk\n+++ draft\n@@ -1,1 +1,1 @@\n-旧\n+新\n\\ No newline at end of file\n"
        );
    }
    #[test]
    fn conflict_time_uses_local_calendar_days_across_offsets_and_midnight() {
        use time::{Date, Month, UtcOffset};
        let date = Date::from_calendar_date(2026, Month::November, 4).expect("date");
        let now = date
            .with_hms(0, 5, 0)
            .expect("clock")
            .assume_offset(UtcOffset::from_hms(-6, 0, 0).expect("offset"));
        let today = now.replace_time(time::Time::from_hms(0, 1, 47).expect("clock"));
        let yesterday = today - time::Duration::minutes(10);
        assert_eq!(
            friendly_time(today, now, i18n::Locale::ZhHans),
            "今天 00:01"
        );
        assert_eq!(
            friendly_time(yesterday, now, i18n::Locale::ZhHans),
            "昨天 23:51"
        );
        assert_eq!(
            friendly_time(yesterday, now, i18n::Locale::En),
            "Yesterday 23:51"
        );
        assert_eq!(
            friendly_time(today - time::Duration::days(2), now, i18n::Locale::En),
            "2026-11-02 00:01"
        );
        // DST can change the offset between the file write and the current time.
        let previous_offset = UtcOffset::from_hms(-5, 0, 0).expect("offset");
        let previous = date
            .with_hms(0, 30, 0)
            .expect("clock")
            .assume_offset(previous_offset);
        assert_eq!(
            friendly_time(previous, now, i18n::Locale::En),
            "Today 00:30"
        );
    }
}
