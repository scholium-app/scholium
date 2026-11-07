//! Flush source/preview geometry and machine-local project preferences.
use eframe::egui;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const HANDLE: f32 = 8.0;
const MIN_SHARE: f32 = 0.2;
const KEY_STEP: f32 = 0.02;

fn bounded(ratio: f32) -> f32 {
    if ratio.is_finite() {
        ratio.clamp(MIN_SHARE, 1.0 - MIN_SHARE)
    } else {
        0.5
    }
}
fn location() -> Option<PathBuf> {
    std::env::var_os("SCHOLIUM_LAYOUT_FILE")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("SCHOLIUM_RECENT_FILE")
                .map(|p| PathBuf::from(p).with_extension("layout.json"))
        })
        .or_else(|| {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
                .map(|p| p.join("scholium/source-layout.json"))
        })
}
fn load(path: &Path) -> BTreeMap<PathBuf, f32> {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn save(path: &Path, project: &Path, ratio: f32) -> std::io::Result<()> {
    use std::io::Write;
    let mut values = load(path);
    values.insert(project.to_path_buf(), bounded(ratio));
    let parent = path.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
    std::fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(&serde_json::to_vec(&values)?)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// Allocate independent, clipped panes. The hit target is wider than its 1px rule.
pub(crate) fn panes(ui: &mut egui::Ui, project: &Path, enabled: bool) -> (egui::Ui, egui::Ui) {
    let rect = ui.available_rect_before_wrap();
    let id = egui::Id::new(("source-splitter", project));
    let mut ratio = ui.ctx().data_mut(|d| {
        *d.get_temp_mut_or_insert_with(id, || {
            location()
                .and_then(|p| load(&p).get(project).copied())
                .map_or(0.5, bounded)
        })
    });
    let old = ratio;
    let x = rect.left() + rect.width() * ratio;
    let hit = egui::Rect::from_min_max(
        egui::pos2(x - HANDLE / 2.0, rect.top()),
        egui::pos2(x + HANDLE / 2.0, rect.bottom()),
    );
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            ui.interact(hit, id, egui::Sense::click_and_drag())
        })
        .inner;
    if response.clicked() || response.drag_started() {
        response.request_focus();
    }
    if response.dragged()
        && let Some(pos) = response.interact_pointer_pos()
    {
        ratio = bounded((pos.x - rect.left()) / rect.width());
    }
    if response.has_focus() && enabled {
        ui.input_mut(|input| {
            for (key, delta) in [
                (egui::Key::ArrowLeft, -KEY_STEP),
                (egui::Key::ArrowRight, KEY_STEP),
            ] {
                if input.consume_key(egui::Modifiers::NONE, key) {
                    ratio = bounded(ratio + delta);
                }
            }
        });
    }
    if response.double_clicked() {
        ratio = 0.5;
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, ratio));
    if ((ratio != old && !response.dragged()) || response.drag_stopped())
        && let Some(path) = location()
        && let Err(error) = save(&path, project, ratio)
    {
        eprintln!("source layout preference: {error}");
    }
    response
        .clone()
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    let x = rect.left() + rect.width() * ratio;
    let t = crate::theme::tokens(ui.ctx());
    ui.painter().rect_filled(rect, 0.0, t.canvas);
    ui.painter().rect_filled(
        egui::Rect::from_min_max(rect.min, egui::pos2(x, rect.bottom())),
        0.0,
        t.field,
    );
    ui.painter().line_segment(
        [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
        egui::Stroke::new(if response.has_focus() { 2.0 } else { 1.0 }, t.border),
    );
    let left = egui::Rect::from_min_max(rect.min, egui::pos2(x - HANDLE / 2.0, rect.bottom()));
    let right = egui::Rect::from_min_max(egui::pos2(x + HANDLE / 2.0, rect.top()), rect.max);
    crate::project_workspace::audit_rect("splitter", hit);
    let pane = |ui: &mut egui::Ui, rect, salt| {
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(salt)
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        child
    };
    (
        pane(ui, left, "source-pane"),
        pane(ui, right, "preview-pane"),
    )
}

/// Logical px per Typst/TeX point, shared by all preview canvases.
pub(crate) fn scale(
    state: &mut crate::state::WorkspaceState,
    width: f32,
    page_width_pt: f32,
) -> f32 {
    let value = if state.fit_width {
        ((width - 32.0).max(1.0) / page_width_pt).clamp(0.1, 8.0)
    } else {
        state.zoom * (96.0 / 72.0)
    };
    state.rendered_zoom = value / (96.0 / 72.0);
    value
}

pub(crate) fn navigation(ui: &mut egui::Ui, page: &mut usize, pages: usize) {
    let previous = ui.add_enabled(*page > 0, egui::Button::new("‹"));
    crate::project_workspace::audit_rect("preview.previous", previous.rect);
    if previous.clicked() {
        *page -= 1;
    }
    let next = ui.add_enabled(*page + 1 < pages, egui::Button::new("›"));
    crate::project_workspace::audit_rect("preview.next", next.rect);
    if next.clicked() {
        *page += 1;
    }
    ui.weak(format!("{}/{}", *page + 1, pages));
}

/// Paint a page centred in a scrollable canvas; returned coordinates are logical window px.
pub(crate) fn page(
    ui: &mut egui::Ui,
    texture: egui::TextureId,
    size: egui::Vec2,
    salt: &str,
) -> egui::Rect {
    let available = ui.available_width();
    let mut shown = egui::Rect::NOTHING;
    egui::ScrollArea::both()
        .id_salt(salt)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(16.0);
            let width = available.max(size.x + 32.0);
            let (flow, _) =
                ui.allocate_exact_size(egui::vec2(width, size.y + 16.0), egui::Sense::hover());
            let ppp = ui.ctx().pixels_per_point();
            let origin = egui::pos2(
                ((flow.left() + (width - size.x) / 2.0) * ppp).round() / ppp,
                (flow.top() * ppp).round() / ppp,
            );
            shown = egui::Rect::from_min_size(origin, size);
            ui.painter().image(
                texture,
                shown,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        });
    shown
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ratio_is_bounded_and_persisted_for_each_project() {
        let dir = tempfile::tempdir().expect("preferences");
        let path = dir.path().join("layout.json");
        save(&path, Path::new("/paper"), 0.64).expect("save");
        save(&path, Path::new("/book"), 0.34).expect("save");
        let values = load(&path);
        assert_eq!(values[Path::new("/paper")], 0.64);
        assert_eq!(values[Path::new("/book")], 0.34);
        assert_eq!(bounded(f32::NAN), 0.5);
        assert_eq!(bounded(-1.0), MIN_SHARE);
        assert_eq!(bounded(2.0), 1.0 - MIN_SHARE);
    }
    #[test]
    fn fit_width_and_actual_size_publish_the_rendered_zoom() {
        let mut state = crate::state::WorkspaceState::default();
        assert_eq!(scale(&mut state, 632.0, 600.0), 1.0);
        assert_eq!(state.rendered_zoom, 0.75);
        state.fit_width = false;
        state.zoom = 1.5;
        assert_eq!(scale(&mut state, 632.0, 600.0), 2.0);
    }
}
