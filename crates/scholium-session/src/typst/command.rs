//! Commands are the only edit entry; UI effects are returned after the transaction succeeds.
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Editing commands; the first six variants map directly to the app's registered ids.
#[derive(Debug, Clone, Copy)]
pub enum CommandId {
    /// `insert.formula` / `insert.exit_formula`.
    Math,
    /// `insert.fraction`.
    Fraction,
    /// `insert.superscript` / `insert.subscript`.
    Attach(scholium_document::AttachSlot),
    /// `format.bold`, `format.italic`, `format.clear`.
    Style(TextStyle),
    /// `format.paragraph.body`, `format.paragraph.heading1`, `format.paragraph.heading2`.
    Kind(scholium_model::BlockKind),
    /// `edit.undo`.
    Undo,
    /// `edit.redo`.
    Redo,
    /// Select the entire body.
    SelectAll,
    /// Delete the selection or the explicit leaf-local range.
    Delete,
    /// Typed, pasted or committed IME text; never preedit.
    Text,
    /// Replace the semantic selection (including paragraph splits).
    ReplaceRange,
    /// Enter in body or math.
    Split,
    /// Leave an Attach slot, creating its temporary trailing Hole when needed.
    LeaveSlot,
    /// Remove an empty math slot.
    RemoveSlot,
    /// Settle a placeholder after app navigation.
    SettlePlaceholder,
    /// Apply a validated structural edit, used by non-UI callers.
    Structural,
    /// Start an unnamed blank document after app confirmation.
    New,
    /// Test-only error after mutating all checkpoint state.
    #[cfg(test)]
    FailAfterMutation,
    /// Test-only panic after mutating all checkpoint state.
    #[cfg(test)]
    PanicAfterMutation,
}

/// Typed command arguments; mismatched variants fail before mutation.
#[derive(Debug, Default)]
pub enum CommandArgs {
    /// Command has no payload.
    #[default]
    None,
    /// Literal input and its grouping origin.
    Text(String, Origin),
    /// UTF-8 byte range within the caret leaf.
    Range(usize, usize),
    /// Whether the caret is in math and the leaf's UTF-8 byte length.
    Slot(bool, usize),
    /// Validated by the authoritative document layer.
    Edit(StructuralEdit),
}

/// Plain diagnostic source, translated for presentation by app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// Clipboard selection has invalid endpoints.
    CopyEndpoints,
    /// Plain text cannot represent the selected formula/RawMath.
    CopyNonText,
    /// Selection cannot receive a structural formula/block command.
    StructuralSelection,
    /// A formula must be entered first.
    NotMath,
    /// Math cannot receive a text style.
    MathStyle,
    /// Style selection endpoints no longer exist.
    StyleEndpoints,
    /// Replacement contains unsupported RawMath.
    RawSelection,
    /// Formula has no following body leaf for a split.
    MissingBody,
}

/// A failed command is fully rolled back and never writes storage.
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// Rejected editor intent, with no presentation dependency.
    #[error("rejected editor intent: {0:?}")]
    Rejected(Rejection),
    /// Command id and argument shape do not match.
    #[error("invalid command arguments")]
    Arguments,
    /// Structural validation rejected the edit.
    #[error(transparent)]
    Edit(#[from] scholium_document::EditError),
    /// The command panicked; its payload is not published (it may contain user data).
    #[error("editing command panicked and was rolled back")]
    Panicked,
    /// Test-only failure injection.
    #[cfg(test)]
    #[error("injected command failure")]
    Injected,
}

/// Accepted command result; persistence remains an explicit successful `save` operation.
#[derive(Debug, Default)]
pub struct Outcome {
    /// Authority changed or was restored (including a new layout lifetime).
    pub changed: bool,
    /// Whether a conditional operation applied or moved the caret.
    pub applied: bool,
    /// A structural edit accepted, so app clears its previous edit diagnostic.
    pub cleared_error: bool,
    /// Accepted editing invalidated an in-progress app drag, even for a no-op.
    pub reset_drag: bool,
    /// History/New restored a different document lifetime; app discards its old Scene.
    pub invalidate_scene: bool,
}

impl EditorCore {
    /// Execute one command atomically, including every constituent structural edit.
    /// No store access occurs inside the unwind boundary; only `save` writes a head.
    ///
    /// # Errors
    /// Rejects mismatched arguments, unsupported intents, document validation failures and
    /// caught panics. Every checkpoint field is restored before the error is returned.
    pub fn run(&mut self, id: CommandId, args: CommandArgs) -> Result<Outcome, CommandError> {
        #[cfg(test)]
        let checkpoint_start = std::time::Instant::now();
        let checkpoint = self.checkpoint()?;
        #[cfg(test)]
        super::tests::checkpoint_bench::sample(checkpoint_start.elapsed());
        let draft = match (&id, &args) {
            (CommandId::Text | CommandId::ReplaceRange, CommandArgs::Text(text, _)) => {
                Some(text.clone())
            }
            _ => None,
        };
        let mut state = EditState::default();
        // Only owned in-memory editing state is captured, with no UI objects or store writes.
        // The injected clock must only read time; even its panic is followed by full restore.
        let result = catch_unwind(AssertUnwindSafe(|| self.execute(id, args, &mut state)));
        let result = match result {
            Ok(Ok(applied)) => match state.edit_error {
                Some(error) => Err(error),
                None => Ok(applied),
            },
            Ok(Err(error)) => Err(error),
            Err(_) => Err(CommandError::Panicked),
        };
        match result {
            Ok(applied) => Ok(Outcome {
                changed: !Arc::ptr_eq(&checkpoint.snapshot, &self.snapshot),
                applied,
                cleared_error: state.cleared_error,
                reset_drag: state.reset_drag,
                invalidate_scene: matches!(id, CommandId::Undo | CommandId::Redo | CommandId::New),
            }),
            Err(error) => {
                self.rollback(checkpoint);
                if let Some(text) = draft {
                    self.rejected_input = Some(text);
                }
                Err(error)
            }
        }
    }

    fn execute(
        &mut self,
        id: CommandId,
        args: CommandArgs,
        state: &mut EditState,
    ) -> Result<bool, CommandError> {
        use CommandArgs as A;
        use CommandId as C;
        let applied = match (id, args) {
            (C::Math | C::Fraction | C::Attach(_) | C::Style(_) | C::Kind(_), A::None) => {
                self.command(id, state);
                true
            }
            (C::Undo | C::Redo, A::None) => {
                self.history(matches!(id, C::Redo), state);
                true
            }
            (C::Text, A::Text(text, origin)) => {
                self.insert(text, origin, state);
                true
            }
            (C::ReplaceRange, A::Text(text, _)) => self.replace_range(text, state),
            (C::Delete, A::Range(start, end)) => {
                self.erase(start, end, state);
                true
            }
            (C::Delete, A::None) => self.replace_range(String::new(), state),
            (C::Split, A::None) => {
                self.split(state);
                true
            }
            (C::LeaveSlot, A::Slot(math, len)) => self.leave_slot(math, len, state),
            (C::RemoveSlot, A::Slot(empty, _)) => self.remove_slot(empty, state),
            (C::SettlePlaceholder, A::None) => {
                self.settle_placeholder(state);
                true
            }
            (C::Structural, A::Edit(edit)) => self.apply(edit, state),
            (C::SelectAll, A::None) => {
                self.select_all();
                true
            }
            (C::New, A::None) => {
                self.new_document()?;
                state.cleared_error = true;
                state.reset_drag = true;
                true
            }
            #[cfg(test)]
            (C::FailAfterMutation | C::PanicAfterMutation, A::None) => {
                self.inject_failure(id, state)?
            }
            _ => return Err(CommandError::Arguments),
        };
        Ok(applied)
    }

    fn select_all(&mut self) {
        let body: Vec<_> = input::leaves(&self.snapshot)
            .into_iter()
            .filter(|l| !l.math)
            .collect();
        if let (Some(first), Some(last)) = (body.first(), body.last()) {
            self.anchor = Some(Position {
                leaf: first.id,
                byte: 0,
                affinity: Affinity::Downstream,
            });
            self.cursor = Position {
                leaf: last.id,
                byte: last.text.len(),
                affinity: Affinity::Upstream,
            };
        }
        self.run = None;
        self.pending_style = None;
    }
}

mod checkpoint;
