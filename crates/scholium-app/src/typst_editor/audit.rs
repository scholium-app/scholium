//! Opt-in fixture diagnostics for real-window validation; disabled for normal editing.
use super::*;
use serde_json::json;

impl CandidateSession {
    pub(super) fn audit(&mut self, rect: egui::Rect, scale: f32, text_shapes: usize) {
        if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_none() {
            return;
        }
        let carets: Vec<_> = self.scene.as_ref().into_iter().flat_map(|s| &s.geometry.carets).map(|c| json!({
            "leaf": format!("{:?}", c.position.leaf), "byte": c.position.byte,
            "x": rect.left() + c.top[0] as f32 * scale,
            "y": rect.top() + (c.top[1] + c.bottom[1]) as f32 * 0.5 * scale, "exact": c.exact
        })).collect();
        let value = json!({
            "backend": "Typst Content main app", "current": self.current(),
            "revision": self.snapshot.revision.0, "wanted": format!("{:?}", self.wanted),
            "shown": self.scene.as_ref().map(|s| format!("{:?}", s.stamp)),
            "cursor_leaf": format!("{:?}", self.cursor.leaf), "cursor_byte": self.cursor.byte,
            "saved": self.saved.as_ref() == Some(&self.snapshot),
            "snapshot": self.snapshot.as_ref(), "carets": carets, "page_text_draws": text_shapes,
            "source_reads": self.scene.as_ref().map(|s| s.stats.source_reads),
            "layout_error": self.layout_error,
        })
        .to_string();
        if self.last_audit.as_ref() != Some(&value) {
            println!("AUDIT {value}");
            self.last_audit = Some(value);
        }
    }
}
