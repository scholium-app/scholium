//! Read-only disk/draft comparison; saving continues to use storage's exact-byte conflict gate.
use super::*;
use crate::widgets::{self, DialogButton};
use std::time::{SystemTime, UNIX_EPOCH};

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
            .map(timestamp)
            .unwrap_or_else(|_| text("project.conflict.time_unknown"));
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
            ui.label(text("project.conflict.help"));
            if self.diff {
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
// UTC civil date conversion, independent of machine locale and daylight-saving state.
fn timestamp(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (seconds / 86400) as i64 + 719468;
    let era = days / 146097;
    let day = days - era * 146097;
    let year = (day - day / 1460 + day / 36524 - day / 146096) / 365;
    let doy = day - (365 * year + year / 4 - year / 100);
    let month = (5 * doy + 2) / 153;
    let date = doy - (153 * month + 2) / 5 + 1;
    let month = month + if month < 10 { 3 } else { -9 };
    let year = year + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{date:02} {:02}:{:02}:{:02} UTC",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
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
    fn conflict_time_is_an_unambiguous_utc_date() {
        assert_eq!(timestamp(UNIX_EPOCH), "1970-01-01 00:00:00 UTC");
        assert_eq!(
            timestamp(UNIX_EPOCH + std::time::Duration::from_secs(1735689600)),
            "2025-01-01 00:00:00 UTC"
        );
    }
}
