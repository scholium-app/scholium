//! Typst local editing authority and atomic commands (ADR 0042, phase 1).
mod attach;
mod command;
mod diagnose;
mod edit;
mod events;
pub mod history;
pub mod indicator;
pub mod input;
mod math_enter;
mod persistence;
mod selection;
pub mod style;
#[cfg(test)]
mod tests;

pub use command::{
    CommandArgs, CommandError, CommandId, FollowIntent, Outcome, OverlayData, Rejection,
};
pub use events::{Input, Key, Modifiers, PointerKind, PointerState};
pub use history::Origin;
use history::{HistoryEntry, RunKind};
pub use persistence::{SessionError, prepare};
use scholium_document::{
    BodyTextPosition, LocalSession, StructuralEdit, StructuralOutcome, StructuralRequest,
};
use scholium_model::{RequestId, layout_identity::*, structured::*};
use scholium_storage::SessionStore;
use scholium_typst::editor::{Affinity, EditorScene, Position, RasterSpec, ViewRange};
#[cfg(test)]
use std::path::PathBuf;
use std::{path::Path, sync::Arc};

/// Stable semantic location; UTF-8 byte offset within a text/math leaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    /// Leaf identity.
    pub node: scholium_model::NodeId,
    /// Leaf-local UTF-8 byte offset.
    pub byte: usize,
}

/// Local editing core. Public interaction fields are the phase-1 app adapter contract;
/// document mutation and persistence go through commands and `save` respectively.
pub struct EditorCore {
    /// Read-only authority access; mutable document methods are private to this crate.
    pub session: ReadAuthority,
    /// Read-only accepted document projection; never an independent authority.
    pub snapshot: Arc<StructuredDocument>,
    store: SessionStore,
    saved: Option<Arc<StructuredDocument>>,
    /// Caret in leaf-local UTF-8 bytes; navigation remains in app during phase 1.
    pub cursor: Position,
    /// Semantic selection anchor.
    pub anchor: Option<Position>,
    /// Local undo entries; exposed for the transitional app test harness.
    pub undo: Vec<HistoryEntry>,
    /// Local redo entries.
    pub redo: Vec<HistoryEntry>,
    /// Open typing/deletion run.
    pub run: Option<history::Run>,
    /// Injectable monotonic time source.
    pub clock: history::Clock,
    /// Empty trailing Attach slot, owned until the caret leaves it.
    pub placeholder: Option<attach::Placeholder>,
    /// Typing style armed at one position.
    pub pending_style: Option<style::Pending>,
    /// Layout diagnostic source; presentation belongs to app.
    pub layout_error: Option<String>,
    /// Rejected input retained as a copyable draft.
    pub rejected_input: Option<String>,
    /// Formula overflow targets from the current Scene.
    pub overflows: Vec<Target>,
    /// Last viewport supplied by the host, in page points.
    pub visible: ViewRange,
    /// Raster request supplied by the host.
    pub raster: RasterSpec,
    /// Last abstract pointer event, for drag continuity.
    pub pointer: Option<PointerState>,
    #[cfg(test)]
    #[allow(missing_docs)]
    pub fail_next_input: bool,
}

impl std::fmt::Debug for EditorCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorCore")
            .field("snapshot", &self.snapshot)
            .field("cursor", &self.cursor)
            .field("anchor", &self.anchor)
            .field("undo", &self.undo.len())
            .field("redo", &self.redo.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
struct EditState {
    edit_error: Option<CommandError>,
    cleared_error: bool,
    reset_drag: bool,
}

fn math_contains(node: &MathNode, leaf: scholium_model::NodeId) -> bool {
    node.node == leaf
        || input::math_children(node)
            .into_iter()
            .any(|c| math_contains(c, leaf))
}

/// Read-only authority view for phase-1 app adapters. It deliberately has no `DerefMut`;
/// callers can inspect action identities but cannot apply edits outside `run`.
#[derive(Debug)]
pub struct ReadAuthority {
    inner: LocalSession<StructuredDocument>,
}
impl std::ops::Deref for ReadAuthority {
    type Target = LocalSession<StructuredDocument>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl ReadAuthority {
    fn new(inner: LocalSession<StructuredDocument>) -> Self {
        Self { inner }
    }
}
