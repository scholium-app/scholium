//! Command candidates are UI; committed document text remains exclusively Typst-rendered.
use super::*;
use scholium_math_input::{Event, TabAction};

#[cfg(test)]
pub(crate) const CATALOG_KEYS: &[&str] = &[
    "math.input.command",
    "math.input.no_candidates",
    "math.input.accept_hint",
    "math.input.tab.complete",
    "math.input.tab.cycle",
    "math.input.tab.slot",
];

pub(crate) fn tab_hint(state: &WorkspaceState) -> String {
    let action = state
        .editor
        .as_ref()
        .map(|v| v.tab_action)
        .unwrap_or_default();
    crate::i18n::text(
        state.locale.resolve(),
        match action {
            TabAction::Complete => "math.input.tab.complete",
            TabAction::Cycle => "math.input.tab.cycle",
            TabAction::NextSlot => "math.input.tab.slot",
        },
    )
}
impl EditorSession {
    pub(super) fn math_command_box(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
        let Some(query) = self.core.math_input.command().map(str::to_owned) else {
            return;
        };
        let Some((page, scale)) = self.placement else {
            return;
        };
        let caret = self
            .scene()
            .and_then(|s| s.geometry.caret(self.cursor))
            .map(|c| egui::pos2(c.bottom[0] as f32 * scale, c.bottom[1] as f32 * scale));
        let at = page.min + caret.unwrap_or_default().to_vec2();
        let locale = state.locale.resolve();
        let candidates = self.core.math_input.candidates();
        let selected = self.core.math_input.selected();
        let mut chosen = None;
        egui::Area::new(egui::Id::new("math-inline-command"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .constrain(true)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(320.0);
                    ui.label(crate::i18n::text(locale, "math.input.command"));
                    ui.monospace(format!("\\{query}"));
                    if candidates.is_empty() {
                        ui.label(crate::i18n::text(locale, "math.input.no_candidates"));
                    }
                    let start = selected.saturating_sub(7);
                    for (index, candidate) in candidates.iter().enumerate().skip(start).take(8) {
                        let label = match &candidate.symbol {
                            Some(symbol) => format!("{}  {}", candidate.name, symbol),
                            None => candidate.name.clone(),
                        };
                        if ui.selectable_label(index == selected, label).clicked() {
                            chosen = Some(index);
                        }
                    }
                    ui.weak(crate::i18n::text(locale, "math.input.accept_hint"));
                });
            });
        if let Some(index) = chosen {
            self.submit(
                scholium_session::typst::Input::Command(
                    Command::MathInput,
                    CommandArgs::Math(Event::Choose(index)),
                ),
                state,
            );
            self.focus = true;
        }
    }
}
