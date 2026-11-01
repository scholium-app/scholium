//! Opt-in, process-local IME event seam for private-Xvfb window checks.
//! It feeds the ordinary event path; normal editing never reads a command file.
use super::*;

pub(super) fn events(ctx: &egui::Context) -> Vec<egui::Event> {
    if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_none() {
        return Vec::new();
    }
    let Some(path) = std::env::var_os("SCHOLIUM_IME_CHECK_FILE") else {
        return Vec::new();
    };
    ctx.request_repaint_after(std::time::Duration::from_millis(25));
    let Ok(bytes) = std::fs::read(&path) else {
        return Vec::new();
    };
    let _ = std::fs::remove_file(&path);
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Vec::new();
    };
    if let Some(text) = value.get("preedit").and_then(|v| v.as_str()) {
        let cursor = value
            .get("cursor_chars")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        return vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: text.into(),
            active_range_chars: cursor.map(|n| n..n),
        })];
    }
    if let Some(text) = value.get("commit").and_then(|v| v.as_str()) {
        return vec![
            egui::Event::Ime(egui::ImeEvent::Commit(text.into())),
            egui::Event::Text(text.into()),
        ];
    }
    Vec::new()
}
