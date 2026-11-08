//! Opt-in fixture diagnostics for real-window validation; disabled for normal editing.
use super::*;
use serde_json::json;

/// Counts keyboard and text input once per egui pass, including keys the editor ignores.
///
/// Window checks compare it before and after an action, so a delivered no-op key is visible
/// in the audit and a check never mistakes the pre-input state for the settled result.
#[derive(Debug, Default)]
pub(super) struct InputCounter {
    seen: u64,
    pass: Option<u64>,
}

impl InputCounter {
    pub(super) fn observe(&mut self, ctx: &egui::Context) -> u64 {
        let pass = ctx.cumulative_pass_nr();
        if self.pass != Some(pass) {
            self.pass = Some(pass);
            self.seen += ctx.input(|i| i.events.iter().filter(|e| counts(e)).count()) as u64;
        }
        self.seen
    }
}

fn counts(event: &egui::Event) -> bool {
    matches!(
        event,
        egui::Event::Key { pressed: true, .. }
            | egui::Event::PointerButton { pressed: true, .. }
            | egui::Event::Text(_)
            | egui::Event::Paste(_)
            | egui::Event::Ime(egui::ImeEvent::Commit(_))
    )
}

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
        let input_seq = self.audit_input.observe(ctx);
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
        let mut value = json!({
            "backend": "Typst Content main app", "current": self.current(),
            "revision": self.snapshot.revision.0, "wanted": wanted,
            "shown": shown,
            "cursor_leaf": format!("{:?}", self.cursor.leaf), "cursor_byte": self.cursor.byte,
            "cursor_affinity": format!("{:?}", self.cursor.affinity),
            "anchor_leaf": self.anchor.map(|p| format!("{:?}", p.leaf)),
            "anchor_byte": self.anchor.map(|p| p.byte),
            "anchor_affinity": self.anchor.map(|p| format!("{:?}", p.affinity)),
            "undo_depth": self.undo.len(), "redo_depth": self.redo.len(),
            "saved": !self.core.dirty(),
            "snapshot": self.snapshot.as_ref(), "carets": carets, "page_text_draws": text_shapes,
            "semantic": self.core.semantic_projection(),
            "ime_area": ctx.output(|o| o.ime.as_ref().map(|i| [i.rect.left(), i.rect.top(), i.rect.right(), i.rect.bottom()])),
            "ime_caret": ctx.output(|o| o.ime.as_ref().map(|i| [i.cursor_rect.left(), i.cursor_rect.top(), i.cursor_rect.right(), i.cursor_rect.bottom()])),
            "source_reads": self.scene().map(|s| s.stats.source_reads),
            "layout_error": self.layout_error,
            "selection": selection, "selection_quads": quads.as_ref().map(|q| q.len()).ok(),
            "selection_geometry_error": quads.err().map(|e| e.to_string()),
            "body_leaves": body_leaves, "deferred": self.core.deferred_len(),
            "math_input": { "query": self.core.math_input.command(),
                "selected": self.core.math_input.selected(),
                "candidates": self.core.math_input.candidates().iter().map(|c| &c.name).collect::<Vec<_>>(),
                "tab": format!("{:?}", self.core.tab_action()) },
            "source_mode": source_mode, "preview": self.source_view.audit(),
            "raster": raster, "follow": self.follow_audit(),
            "formula_overflow": self.formula_overflows(),
            "diagnostics": self.audit_problems(),
            "controls": crate::widgets::audit::controls_json(ctx),
            "chrome": crate::widgets::audit::facts_json(ctx),
            "fonts": crate::theme::fonts::chosen().map(|f| json!({
                "ui": f.ui, "latin": f.latin, "mono": f.mono,
            })),
        });
        value.as_object_mut().expect("audit is an object").extend(
            json!({
            "layout_stats": self.scene().map(|s| json!({
                "built": s.stats.built, "reused": s.stats.reused,
                "source_reads": s.stats.source_reads, "elapsed_ms": s.stats.elapsed_ms,
            })),
            "document_geometry": self.core.document_scene_is_current(),
            "input_seq": input_seq,
            "preedit_caret": self.scene().and_then(|s| s.preedit.as_ref()).map(|c| json!({
                "leaf": format!("{:?}", c.position.leaf), "byte": c.position.byte,
                "x": rect.left() + c.top[0] as f32 * scale,
                "y": rect.top() + c.top[1] as f32 * scale,
            })),
            })
            .as_object()
            .expect("composition audit is an object")
            .clone(),
        );
        let value = value.to_string();
        if self.last_audit.as_ref() != Some(&value) {
            crate::audit_line::emit("AUDIT", &value);
            self.last_audit = Some(value);
        }
    }
}

#[cfg(test)]
mod input_counter_tests {
    use super::*;

    fn key(pressed: bool) -> egui::Event {
        egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn pass(
        ctx: &egui::Context,
        counter: &mut InputCounter,
        time: f64,
        events: Vec<egui::Event>,
    ) -> u64 {
        let mut seen = 0;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400.0, 300.0),
            )),
            events,
            time: Some(time),
            ..Default::default()
        };
        let mut output = ctx.run_ui(raw, |ui| {
            // A second audit in the same pass must not count the events again.
            counter.observe(ui.ctx());
            seen = counter.observe(ui.ctx());
        });
        output.textures_delta.clear();
        seen
    }

    #[test]
    fn counts_delivered_presses_and_text_once_per_pass_including_ignored_keys() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let mut counter = InputCounter::default();
        let events = vec![key(true), key(false), egui::Event::Text("a".into())];
        assert_eq!(pass(&ctx, &mut counter, 0.1, events), 2);
        assert_eq!(
            pass(&ctx, &mut counter, 0.2, Vec::new()),
            2,
            "idle passes add nothing"
        );
        assert_eq!(pass(&ctx, &mut counter, 0.3, vec![key(true)]), 3);
    }
}
