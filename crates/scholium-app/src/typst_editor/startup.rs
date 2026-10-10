//! Default-launch document location; a legacy head is migrated once, never rewritten.
use super::*;
use std::path::Path;

/// Prepare `target` before the editor opens it.
///
/// Migrates only when `target` is absent and `legacy` holds a saved head. Both the
/// probe and the migration open the legacy file read-only; it stays the user's copy.
pub(super) fn prepare(
    target: &Path,
    legacy: &Path,
) -> Result<Option<scholium_storage::FileMigrationReport>, SessionError> {
    if target.try_exists()? || !legacy.try_exists()? {
        return Ok(None);
    }
    let migrated = scholium_storage::legacy_has_head(legacy).and_then(|head| {
        head.then(|| scholium_storage::migrate_legacy_file(legacy, target))
            .transpose()
    });
    match migrated {
        Ok(report) => Ok(report),
        // Another instance published the target first; its migration is authoritative.
        Err(_) if target.try_exists()? => Ok(None),
        Err(error) => Err(SessionError::Startup(format!(
            "旧版会话 {} 无法迁移：{error}。旧文件未改动",
            legacy.display()
        ))),
    }
}

/// Shown instead of any editor when the default document cannot be opened.
///
/// Never starts an editor on a guessed document: that could write a second authority.
pub(crate) fn show_error(ui: &mut egui::Ui, error: &str) {
    egui::CentralPanel::default().show(ui, |ui| {
        ui.heading("无法打开默认文档");
        ui.colored_label(ui.visuals().error_fg_color, error);
        ui.label("文件没有被修改。移走或改名上面的文件后重新启动，以空白文档开始；");
        ui.label("旧版会话仍可用 0.1 系列早于 ADR 0037 的版本打开。");
    });
}

/// Default-launch document, with a notice when it was just migrated.
pub(super) fn default_document() -> Result<(PathBuf, Option<String>), SessionError> {
    let target = scholium_storage::default_document_path().map_err(SessionError::Startup)?;
    let legacy = scholium_storage::default_session_path().map_err(SessionError::Startup)?;
    let notice = prepare(&target, &legacy)?.map(|report| {
        let raw = report.content.raw_formulas.len();
        let formulas = if raw == 0 {
            String::new()
        } else {
            format!("；{raw} 个旧公式保留为原串，转换前不可排版或编辑")
        };
        format!(
            "已从旧版文档迁移，原文件保留在 {}{formulas}。",
            legacy.display()
        )
    });
    Ok((target, notice))
}

#[cfg(test)]
mod tests;
