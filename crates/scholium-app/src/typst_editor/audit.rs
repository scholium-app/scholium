//! Opt-in fixture diagnostics for real-window validation; disabled for normal editing.
use super::*;
use serde_json::json;

impl EditorSession {
    pub(super) fn audit(
        &mut self,
        ctx: &egui::Context,
        rect: egui::Rect,
        scale: f32,
        text_shapes: usize,
        ppp: f32,
    ) {
        self.pixels_per_point = ppp;
        self.audit_with(ctx, Some((rect, scale)), text_shapes, false);
    }

    /// Source-workspace frames also report, so window checks can follow the preview.
    pub(super) fn audit_source(&mut self, ctx: &egui::Context) {
        self.audit_with(ctx, None, 0, true);
    }

    fn audit_with(
        &mut self,
        ctx: &egui::Context,
        placement: Option<(egui::Rect, f32)>,
        text_shapes: usize,
        source_mode: bool,
    ) {
        let (rect, scale) = placement.unwrap_or((egui::Rect::ZERO, 1.0));
        if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_none() {
            return;
        }
        let carets: Vec<_> = self.scene().into_iter().flat_map(|s| &s.geometry.carets).map(|c| json!({
            "leaf": format!("{:?}", c.position.leaf), "byte": c.position.byte,
            "affinity": format!("{:?}", c.position.affinity),
            "x": rect.left() + c.top[0] as f32 * scale,
            "y": rect.top() + (c.top[1] + c.bottom[1]) as f32 * 0.5 * scale, "exact": c.exact
        })).collect();
        let selection = self.range().ok().flatten().map(|(a, b)| {
            json!({
                "start_leaf": format!("{:?}", a.leaf), "start_byte": a.byte,
                "end_leaf": format!("{:?}", b.leaf), "end_byte": b.byte,
            })
        });
        let quads = self.selection_quads();
        let body_leaves: Vec<_> = input::leaves(&self.snapshot)
            .iter()
            .filter(|l| !l.math)
            .map(|l| json!({"leaf": format!("{:?}", l.id), "bytes": l.text.len()}))
            .collect();
        let ppp = self.pixels_per_point;
        let raster = self.texture.as_ref().map(|layers| {
            let (base, width_px, height_px, strips) = layers.audit();
            json!({
                "px_per_pt": layers.sharp_px_per_pt(), "width_px": width_px, "height_px": height_px,
                "strips": strips, "base_px_per_pt": base,
                "visible_sharp": layers.covers(self.core.visible_range()),
                "wanted_px_per_pt": self.core.raster_spec().px_per_pt,
                "pixels_per_point": ppp,
                "displayed_width_px": rect.width() * ppp,
                "page_left_px": rect.min.x * ppp, "page_top_px": rect.min.y * ppp,
            })
        });
        let (wanted, shown) = self.core.scene_identities();
        let value = json!({
            "backend": "Typst Content main app", "current": self.current(),
            "revision": self.snapshot.revision.0, "wanted": wanted,
            "shown": shown,
            "cursor_leaf": format!("{:?}", self.cursor.leaf), "cursor_byte": self.cursor.byte,
            "cursor_affinity": format!("{:?}", self.cursor.affinity),
            "saved": !self.core.dirty(),
            "snapshot": self.snapshot.as_ref(), "carets": carets, "page_text_draws": text_shapes,
            "source_reads": self.scene().map(|s| s.stats.source_reads),
            "layout_error": self.layout_error,
            "selection": selection, "selection_quads": quads.as_ref().map(|q| q.len()).ok(),
            "selection_geometry_error": quads.err().map(|e| e.to_string()),
            "body_leaves": body_leaves, "deferred": self.core.deferred_len(),
            "source_mode": source_mode, "preview": self.source_view.audit(),
            "raster": raster, "follow": self.follow_audit(),
            "formula_overflow": self.formula_overflows(),
            "diagnostics": self.audit_problems(),
            "controls": crate::widgets::audit::controls_json(ctx),
            "chrome": crate::widgets::audit::facts_json(ctx),
            "fonts": crate::theme::fonts::chosen().map(|f| json!({
                "ui": f.ui, "latin": f.latin, "mono": f.mono,
            })),
        })
        .to_string();
        if self.last_audit.as_ref() != Some(&value) {
            println!("AUDIT {value}");
            self.last_audit = Some(value);
        }
    }
}
