//! The window audit's view of the chrome: every control drawn this frame, keyed by command
//! id, so real-window checks click by command instead of by coordinate. Rectangles are in
//! window pixels (logical points times the pixel ratio), as xdotool wants them.
use super::Meta;
use eframe::egui::{self, Rect};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Entry {
    pub(crate) id: String,
    pub(crate) surface: &'static str,
    pub(crate) rect: Rect,
    pub(crate) enabled: bool,
    pub(crate) checked: Option<bool>,
}

#[derive(Clone, Default)]
struct Frame {
    controls: Arc<Mutex<Vec<Entry>>>,
    facts: Arc<Mutex<Value>>,
}

fn frame(ctx: &egui::Context) -> Frame {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(egui::Id::new("scholium-audit-frame"), Frame::default)
            .clone()
    })
}

/// Forget last frame's controls; call before any chrome is drawn.
pub(crate) fn begin_frame(ctx: &egui::Context) {
    if let Ok(mut controls) = frame(ctx).controls.lock() {
        controls.clear();
    }
}

pub(crate) fn record(ctx: &egui::Context, meta: &Meta, rect: Rect) {
    if let Ok(mut controls) = frame(ctx).controls.lock() {
        controls.push(Entry {
            id: meta.id.clone(),
            surface: meta.surface,
            rect,
            enabled: meta.enabled,
            checked: meta.checked,
        });
    }
}

/// Chrome-wide facts (locale, theme, which toolbar groups overflowed) for the audit.
pub(crate) fn set_facts(ctx: &egui::Context, facts: Value) {
    if let Ok(mut slot) = frame(ctx).facts.lock() {
        *slot = facts;
    }
}

/// The controls drawn this frame so far.
pub(crate) fn entries(ctx: &egui::Context) -> Vec<Entry> {
    frame(ctx)
        .controls
        .lock()
        .map(|c| c.clone())
        .unwrap_or_default()
}

/// The `controls` map: command id to every visible control that runs it.
pub(crate) fn controls_json(ctx: &egui::Context) -> Value {
    let ppp = ctx.pixels_per_point();
    let mut map = serde_json::Map::new();
    for entry in entries(ctx) {
        let px = |v: f32| (v * ppp).round() as i64;
        let item = json!({
            "surface": entry.surface,
            "rect": [px(entry.rect.min.x), px(entry.rect.min.y), px(entry.rect.max.x), px(entry.rect.max.y)],
            "enabled": entry.enabled,
            "checked": entry.checked,
        });
        if let Value::Array(list) = map.entry(entry.id).or_insert_with(|| json!([])) {
            list.push(item);
        }
    }
    Value::Object(map)
}

/// Facts set by the chrome, plus the pixel ratio the rectangles were scaled by.
pub(crate) fn facts_json(ctx: &egui::Context) -> Value {
    let facts = frame(ctx)
        .facts
        .lock()
        .map(|f| f.clone())
        .unwrap_or(Value::Null);
    json!({"facts": facts, "pixels_per_point": ctx.pixels_per_point()})
}
