//! Typst editing authority, abstract input and derived Scene runtime (ADR 0042).
mod attach;
mod command;
mod deferred;
mod diagnose;
mod edit;
mod events;
mod follow;
pub mod history;
pub mod indicator;
pub mod input;
mod key;
mod math_enter;
mod persistence;
pub mod pointer;
pub mod raster;
mod runtime;
#[cfg(test)]
mod runtime_tests;
mod selection;
pub mod style;
#[cfg(test)]
mod tests;
pub mod vertical;

pub use command::{
    CommandArgs, CommandError, CommandId, FollowIntent, Outcome, OverlayData, Rejection,
};
pub use deferred::{Admission, DEFERRED_LIMIT, DeferredEvent, DeferredStep};
pub use events::{FollowEdge, FollowState, Input, Key, Modifiers, PointerKind, PointerState};
pub use history::Origin;
use history::{HistoryEntry, RunKind};
pub use persistence::{SessionError, prepare};
use runtime::Runtime;
use scholium_document::{
    BodyTextPosition, LocalSession, StructuralEdit, StructuralOutcome, StructuralRequest,
};
use scholium_model::{RequestId, layout_identity::*, structured::*};
use scholium_storage::SessionStore;
use scholium_typst::editor::{
    Affinity, EditGeometry, EditorScene, Position, RasterSpec, ViewRange,
};
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
    runtime: Option<Runtime>,
    deferred: std::collections::VecDeque<DeferredEvent>,
    /// Wanted complete layout identity, including the session lifetime.
    pub wanted: SceneStamp,
    /// Last adopted derived Scene, retained while newer layout is pending.
    pub scene: Option<EditorScene>,
    /// Read-only authority access; mutable document methods are private to this crate.
    pub session: ReadAuthority,
    /// Read-only accepted document projection; never an independent authority.
    pub snapshot: Arc<StructuredDocument>,
    store: SessionStore,
    saved: Option<Arc<StructuredDocument>>,
    /// Caret in leaf-local UTF-8 bytes.
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
    pub view: ViewRange,
    /// Raster request supplied by the host.
    pub raster: RasterSpec,
    /// Last abstract pointer event, for drag continuity.
    pub pointer: Option<PointerState>,
    /// Immutable geometry of the adopted current Scene for semantic input.
    pub geometry: Option<EditGeometry>,
    /// Scene identity associated with `geometry`.
    pub geometry_stamp: Option<SceneStamp>,
    /// Preferred visual column for consecutive vertical navigation.
    pub(crate) goal: Option<vertical::Goal>,
    /// Pending caret-follow intent owned by the editing core.
    pub follow: FollowState,
    /// Whether a pointer drag has an active semantic anchor.
    pub pointer_drag: bool,
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

impl EditorCore {
    /// Install immutable geometry while the session adopts or invalidates a Scene.
    pub fn set_geometry(&mut self, geometry: Option<EditGeometry>, stamp: Option<SceneStamp>) {
        self.geometry = geometry;
        self.geometry_stamp = stamp;
    }

    /// Move the semantic caret and optionally extend the selection.
    pub fn move_to(&mut self, target: Position, extend: bool) {
        self.goal = None;
        if extend {
            let cursor = self.cursor;
            self.anchor.get_or_insert(cursor);
        } else {
            self.anchor = None;
        }
        self.cursor = target;
    }

    /// Move to a validated diagnostic location and request caret follow.
    pub fn jump_to(&mut self, target: Target) -> bool {
        let Some(leaf) = input::leaves(&self.snapshot)
            .into_iter()
            .find(|leaf| leaf.id == target.node)
        else {
            return false;
        };
        self.cursor = Position {
            leaf: target.node,
            byte: if target.byte <= leaf.text.len() && leaf.text.is_char_boundary(target.byte) {
                target.byte
            } else {
                0
            },
            affinity: Affinity::Downstream,
        };
        self.anchor = None;
        self.goal = None;
        self.run = None;
        self.pending_style = None;
        self.follow.pending = true;
        self.follow.edge = None;
        self.pointer_drag = false;
        true
    }
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
