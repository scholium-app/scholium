//! Command candidates are UI; committed document text remains exclusively Typst-rendered.
use super::*;
use scholium_math_input::{Event, TabAction};

#[cfg(test)]
pub(crate) const CATALOG_KEYS: &[&str] = &[
    "math.input.command",
    "math.input.category.letter",
    "math.input.category.relation",
    "math.input.category.arrow",
    "math.input.category.constant",
    "math.input.category.punctuation",
    "math.input.category.structure",
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
        let Some(caret) = self.scene().and_then(|s| s.geometry.caret(self.cursor)) else {
            return;
        };
        let point = |p: [f64; 2]| page.min + egui::vec2(p[0] as f32 * scale, p[1] as f32 * scale);
        let caret = egui::Rect::from_two_pos(point(caret.top), point(caret.bottom));
        let candidates = self.core.math_input.candidates();
        let chosen = crate::widgets::command_box(
            ctx,
            crate::widgets::CommandBox {
                id: "math-inline-command",
                caret,
                query: &query,
                candidates: &candidates,
                selected: self.core.math_input.selected(),
                locale: state.locale.resolve(),
            },
        );
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
