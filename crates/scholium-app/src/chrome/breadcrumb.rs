//! Source navigation keeps the file and current element visible before ancestors.
use crate::{
    state::WorkspaceState,
    theme::{self, Tokens},
    widgets::{self, Meta},
};
use eframe::egui::{self, Ui};

pub(super) fn show(ui: &mut Ui, state: &mut WorkspaceState, room: f32) {
    let items = &state.source_navigation.items;
    if items.is_empty() || room < 30.0 {
        return;
    }
    let t = theme::tokens(ui.ctx());
    let font = theme::font(Tokens::SMALL_TEXT);
    let widths: Vec<_> = items
        .iter()
        .map(|c| widgets::text_width(ui, &c.label, font.clone()) + 12.0)
        .collect();
    let indices = visible(&widths, room);
    let start = ui.cursor().left();
    for (i, &index) in indices.iter().enumerate() {
        if i > 0 {
            let sep = if indices[i - 1] + 1 == index {
                "›"
            } else {
                "… ›"
            };
            ui.label(
                egui::RichText::new(sep)
                    .size(Tokens::SMALL_TEXT.0)
                    .color(t.text_dim),
            );
        }
        let remaining = (room
            - (ui.cursor().left() - start)
            - if i + 1 < indices.len() {
                widths[*indices.last().expect("nonempty indices")].min(room * 0.6) + 26.0
            } else {
                0.0
            })
        .max(16.0);
        if level(
            ui,
            &items[index],
            index,
            index + 1 == items.len(),
            remaining,
            widths[index],
        ) {
            state.source_navigation.jump = Some(items[index].byte);
        }
    }
    if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_some() {
        println!(
            "BREADCRUMB {}",
            serde_json::json!({"items":items.iter().map(|c| serde_json::json!({"label":c.label,"byte":c.byte})).collect::<Vec<_>>(),"visible":indices})
        );
    }
}
fn level(
    ui: &mut Ui,
    crumb: &crate::source_editor::navigation::Crumb,
    index: usize,
    current: bool,
    remaining: f32,
    width: f32,
) -> bool {
    let t = theme::tokens(ui.ctx());
    let galley = widgets::single_line(
        ui,
        &crumb.label,
        theme::font(Tokens::SMALL_TEXT),
        if current { t.text } else { t.text_dim },
        (remaining - 12.0).max(4.0),
    );
    let meta = Meta::new(format!("ui.breadcrumb.{index}"), "focus", &crumb.label);
    let response = meta.place(ui, egui::vec2(galley.size().x + 12.0, Tokens::FIELD));
    if response.hovered() {
        ui.painter().rect_filled(response.rect, 0.0, t.hover);
    }
    ui.painter().galley(
        widgets::text_pos(response.rect, response.rect.left() + 6.0, &galley),
        galley,
        t.text,
    );
    widgets::focus_ring(ui, &response, Tokens::RADIUS_CONTROL);
    if response.clicked() {
        return true;
    }
    if width > response.rect.width() {
        response.on_hover_text(&crumb.label);
    }
    false
}
fn visible(widths: &[f32], room: f32) -> Vec<usize> {
    if widths.is_empty() {
        return Vec::new();
    }
    let last = widths.len() - 1;
    let mut result = vec![0];
    let mut used = widths[0] + widths[last] + 26.0;
    if last > 0 {
        let mut middle = Vec::new();
        for index in (1..last).rev() {
            if used + widths[index] + 16.0 > room {
                break;
            }
            used += widths[index] + 16.0;
            middle.push(index);
        }
        result.extend(middle.into_iter().rev());
        result.push(last);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn narrow_breadcrumb_keeps_file_and_current_element() {
        assert_eq!(visible(&[70.0, 100.0, 100.0, 80.0], 200.0), [0, 3]);
        assert_eq!(visible(&[70.0, 100.0, 100.0, 80.0], 500.0), [0, 1, 2, 3]);
    }
}
