//! The editor's contribution to the diagnostics list: what is wrong right now and where,
//! and the jump that moves the caret to a diagnostic's location.
use super::*;
use crate::{
    diagnostics::{self, Diagnostic, Severity, Target},
    state::ViewMode,
};
use scholium_model::NodeId;

impl EditorSession {
    fn excerpt_near(&self, leaf: NodeId) -> Option<String> {
        diagnostics::excerpt(&self.core.paragraph_text(leaf)?)
    }

    /// Every current problem, most serious first. Locations are leaf identities, so a
    /// diagnostic keeps pointing at its text while the picture is re-laid out.
    pub(super) fn problems(&self, state: &WorkspaceState) -> Vec<Diagnostic> {
        let mut list = Vec::new();
        let errors = [
            ("diag.edit", "detail", &state.edit_error),
            ("diag.save", "error", &state.storage_error),
            ("diag.layout_failed", "error", &self.layout_error),
        ];
        for (key, name, error) in errors {
            if let Some(error) = error {
                list.push(Diagnostic::new(Severity::Error, key).arg(name, error.clone()));
            }
        }
        if self.rejected_input.is_some() {
            list.push(Diagnostic::new(Severity::Warning, "diag.rejected"));
        }
        if let Some(notice) = &self.notice {
            list.push(Diagnostic::new(Severity::Info, "diag.notice").arg("detail", notice.clone()));
        }
        if self.current() && state.mode != ViewMode::Source {
            self.page_problems(&mut list);
        }
        diagnostics::ordered(list)
    }

    /// Problems that need the current Scene: selection geometry, formulas, the caret.
    fn page_problems(&self, list: &mut Vec<Diagnostic>) {
        if self.selected() {
            match self.selection_quads() {
                Err(_) => list.push(Diagnostic::new(
                    Severity::Warning,
                    "diag.selection.unsupported",
                )),
                Ok(quads) if quads.iter().any(|q| !q.exact) => {
                    list.push(Diagnostic::new(
                        Severity::Warning,
                        "diag.selection.estimated",
                    ));
                }
                _ => {}
            }
        }
        for target in &self.overflows {
            let found = Diagnostic::new(Severity::Warning, "diag.formula_overflow");
            let at = Target {
                node: target.node,
                byte: target.byte,
            };
            list.push(found.at(at, self.excerpt_near(target.node)));
        }
        let caret = self
            .scene
            .as_ref()
            .and_then(|s| s.geometry.caret(self.cursor));
        if caret.is_none() {
            let at = Target {
                node: self.cursor.leaf,
                byte: self.cursor.byte,
            };
            let found = Diagnostic::new(Severity::Warning, "diag.caret_missing");
            list.push(found.at(at, self.excerpt_near(at.node)));
        }
    }

    /// Move the caret to a diagnostic's location and let caret-follow bring it into view.
    /// A location that no longer exists (the text was edited away) is ignored.
    pub(super) fn jump(&mut self, target: Target) -> bool {
        if !self.core.jump_to(scholium_session::typst::Target {
            node: target.node,
            byte: target.byte,
        }) {
            return false;
        }
        self.drag = None;
        self.run = None;
        self.pending_style = None;
        self.focus = true;
        self.follow.sync_from_core(&self.core.follow);
        true
    }

    /// Diagnostics for the window audit, with their locations.
    pub(super) fn audit_problems(&self) -> Vec<serde_json::Value> {
        self.shown_problems
            .iter()
            .map(|d| {
                serde_json::json!({
                    "severity": format!("{:?}", d.severity),
                    "key": d.key,
                    "target": d.target.map(|t| serde_json::json!({
                        "leaf": format!("{:?}", t.node), "byte": t.byte,
                    })),
                    "excerpt": d.excerpt,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
