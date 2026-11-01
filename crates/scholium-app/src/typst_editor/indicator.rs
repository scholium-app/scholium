//! Current-Scene guards and rendering adapters.
use super::*;
#[cfg(test)]
use scholium_session::typst::indicator::Bounds;
pub(super) use scholium_session::typst::indicator::past_column;
impl EditorSession {
    pub(super) fn math_indicator(
        &self,
    ) -> Option<scholium_session::typst::indicator::MathIndicator> {
        if !self.core.document_scene_is_current() {
            return None;
        }
        self.core.math_indicator(self.scene()?)
    }
    pub(super) fn formula_overflows(&self) -> bool {
        self.math_indicator()
            .is_some_and(|indicator| past_column(indicator.formula))
    }
}
#[cfg(test)]
mod tests;
