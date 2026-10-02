//! Candidate authority uses the existing LocalSession, not a second writable editor.

mod edit;
mod range;
#[cfg(test)]
mod tests;

use super::*;
use scholium_model::structured::{MigrationReport, StructureError, StructuredDocument};

/// Failed consuming migration gives the untouched original authority back.
#[derive(Debug)]
pub struct MigrationFailure {
    /// Original legacy session and request history.
    pub session: Box<LocalSession>,
    /// Reason no switch was made.
    pub error: StructureError,
}

/// Stable body-text endpoint, independent of layout and source projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyTextPosition {
    /// Addressed body Text inline; mathematical and Raw endpoints are rejected.
    pub leaf: NodeId,
    /// Leaf-local UTF-8 extended-grapheme boundary.
    pub byte: usize,
}

/// Accepted local outcome; derived geometry is not part of the authority result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralOutcome {
    /// Whether one revision and action were added.
    pub changed: bool,
    /// Surviving body cursor for a range replacement, including newly split leaves.
    /// Other edits return None; no-op body ranges still return a legal cursor.
    pub cursor: Option<BodyTextPosition>,
}

/// One local identified edit, accepted as a single revision and journal entry.
#[derive(Debug, Clone)]
pub enum StructuralEdit {
    /// Replace a forward body range across inline nodes and blocks atomically.
    /// Fully covered math/Raw inlines are removed; their interiors cannot be endpoints.
    /// LF, CRLF and CR create blocks, inheriting the first block's kind.
    ReplaceBodyRange {
        /// Inclusive body-text endpoint in document order.
        start: BodyTextPosition,
        /// Exclusive body-text endpoint in document order.
        end: BodyTextPosition,
        /// Literal text; markup characters are never parsed.
        text: String,
    },
    /// Replace a leaf range at UTF-8 grapheme boundaries (empty range inserts text).
    ReplaceText {
        /// Addressed body text or math Text/Hole leaf.
        leaf: NodeId,
        /// Inclusive byte boundary within the leaf.
        start: usize,
        /// Exclusive byte boundary within the leaf.
        end: usize,
        /// Literal replacement; never evaluated as markup.
        text: String,
    },
    /// Insert an empty structured formula into a block's inline sequence.
    InsertMath {
        /// Block owner.
        block: NodeId,
        /// Inline insertion boundary.
        at: usize,
    },
    /// Insert inline math at a body grapheme boundary in one action.
    /// The left leaf survives; the right leaf and formula receive fresh IDs.
    InsertMathAt {
        /// Body text leaf, never a math/source leaf.
        leaf: NodeId,
        /// UTF-8 grapheme boundary in that leaf.
        at: usize,
    },
    /// Wrap a math subtree as numerator and create an addressed empty denominator.
    WrapFraction {
        /// Surviving numerator subtree identity.
        node: NodeId,
    },
    /// Split at a body leaf boundary; the left block/leaf retain their identities.
    SplitBlock {
        /// Existing block identity.
        block: NodeId,
        /// Body leaf within that block.
        leaf: NodeId,
        /// Leaf-local UTF-8 grapheme boundary.
        at: usize,
    },
    /// Change an addressed block's paragraph/heading kind without changing identity.
    SetKind {
        /// Block identity.
        block: NodeId,
        /// New semantic kind.
        kind: BlockKind,
    },
    /// Set the style of one whole body text leaf; range styling is a later adapter.
    SetTextStyle {
        /// Body text identity.
        leaf: NodeId,
        /// New plain/strong/emphasis style.
        style: scholium_model::structured::TextStyle,
    },
    /// Move following inline nodes into a block; no surviving node is renumbered.
    MergeWithNext {
        /// Block preceding the removed block.
        block: NodeId,
    },
}

/// Revision/identity checked structural request, with the same action journal semantics.
#[derive(Debug, Clone)]
pub struct StructuralRequest {
    /// Unique successful-request identity.
    pub request: RequestId,
    /// Owning document.
    pub document: DocumentId,
    /// Semantic revision against which the intent was produced.
    pub base: Revision,
    /// Pure structural edit.
    pub edit: StructuralEdit,
}

impl LocalSession {
    /// Validate migration, then consume this authority and retain its request identities.
    ///
    /// # Errors
    /// On malformed/capacity input, returns the original session in MigrationFailure.
    pub fn into_structured(
        self,
    ) -> Result<(LocalSession<StructuredDocument>, MigrationReport), MigrationFailure> {
        let result =
            validate_journal(&self).and_then(|()| StructuredDocument::migrate(&self.snapshot));
        let migration = match result {
            Ok(migration) => migration,
            Err(error) => {
                return Err(MigrationFailure {
                    session: Box::new(self),
                    error,
                });
            }
        };
        Ok((
            LocalSession {
                snapshot: migration.document,
                actions: self.actions,
                epoch: LayoutEpoch::fresh(),
            },
            migration.report,
        ))
    }
}

impl LocalSession<StructuredDocument> {
    /// Restore identified authority without reassigning IDs or replaying source.
    ///
    /// # Errors
    /// Rejects malformed structure, journal length mismatch or duplicate request IDs.
    pub fn restore_structured(
        snapshot: StructuredDocument,
        requests: Vec<RequestId>,
    ) -> Result<Self, EditError> {
        snapshot.validate()?;
        if requests.len() as u64 != snapshot.revision.0 || requests.len() > MAX_ACTIONS {
            return Err(EditError::Capacity);
        }
        let unique: std::collections::HashSet<_> = requests.iter().collect();
        if unique.len() != requests.len() {
            return Err(EditError::DuplicateRequest);
        }
        let actions = requests
            .into_iter()
            .enumerate()
            .map(|(i, request)| Action {
                request,
                before: Revision(i as u64),
                after: Revision(i as u64 + 1),
            })
            .collect();
        Ok(Self {
            snapshot,
            actions,
            epoch: LayoutEpoch::fresh(),
        })
    }

    /// Plan on a temporary snapshot, validate, then commit exactly one semantic action.
    ///
    /// # Errors
    /// Rejects wrong/stale/duplicate requests, invalid targets/boundaries or capacity;
    /// rejection never changes content, revision, epoch or the action journal.
    pub fn apply_structural(&mut self, request: StructuralRequest) -> Result<bool, EditError> {
        self.apply_structural_outcome(request)
            .map(|outcome| outcome.changed)
    }

    /// Accept one atomic edit and return the planner's surviving body cursor.
    ///
    /// # Errors
    /// Uses the same identity, revision, boundary and capacity checks as apply_structural.
    /// Failure never changes authority, epoch or journal, and returns no cursor.
    pub fn apply_structural_outcome(
        &mut self,
        request: StructuralRequest,
    ) -> Result<StructuralOutcome, EditError> {
        self.check_request(&request)?;
        let mut planned = self.snapshot.clone();
        let cursor = edit::apply(&mut planned, request.edit)?;
        planned.validate()?;
        if planned == self.snapshot {
            return Ok(StructuralOutcome {
                changed: false,
                cursor,
            });
        }
        let next = self
            .snapshot
            .revision
            .0
            .checked_add(1)
            .ok_or(EditError::Capacity)?;
        planned.revision = Revision(next);
        self.snapshot = planned;
        self.actions.push(Action {
            request: request.request,
            before: request.base,
            after: Revision(next),
        });
        Ok(StructuralOutcome {
            changed: true,
            cursor,
        })
    }

    /// Issue a rendering stamp without changing actions or semantic revision.
    #[must_use]
    pub fn scene_stamp(
        &self,
        profile: ProfileGeneration,
        resources: ResourceGeneration,
    ) -> SceneStamp {
        SceneStamp {
            document: self.snapshot.document,
            epoch: self.epoch,
            request: LayoutRequestId::fresh(),
            revision: self.snapshot.revision,
            profile,
            resources,
        }
    }

    fn check_request(&self, request: &StructuralRequest) -> Result<(), EditError> {
        if request.document != self.snapshot.document {
            return Err(EditError::WrongTarget);
        }
        if self.actions.iter().any(|a| a.request == request.request) {
            return Err(EditError::DuplicateRequest);
        }
        if request.base != self.snapshot.revision {
            return Err(EditError::StaleRevision);
        }
        if self.actions.len() >= MAX_ACTIONS {
            return Err(EditError::Capacity);
        }
        Ok(())
    }
}

fn validate_journal(session: &LocalSession) -> Result<(), StructureError> {
    let unique: std::collections::HashSet<_> = session.actions.iter().map(|a| a.request).collect();
    if session.actions.len() as u64 != session.snapshot.revision.0
        || session.actions.len() > MAX_ACTIONS
        || unique.len() != session.actions.len()
    {
        return Err(StructureError::Journal);
    }
    Ok(())
}
