//! Ephemeral layout identities. None of these fields belong in saved actions.

use crate::{DocumentId, Revision};

/// One lifetime of a layout session, including revision jumps after undo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayoutEpoch(uuid::Uuid);

impl LayoutEpoch {
    /// Allocate a new lifetime on restore, migration or worker replacement.
    #[must_use]
    pub fn fresh() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

/// Identity of one layout submission, independent of its semantic revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayoutRequestId(uuid::Uuid);

impl LayoutRequestId {
    /// Allocate a layout request without creating a document action.
    #[must_use]
    pub fn fresh() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

/// Version of paper size, styles or the selected layout profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProfileGeneration(pub u64);

/// Version of fonts and resources visible to a layout session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResourceGeneration(pub u64);

/// Complete current-scene identity; comparison of revision alone is insufficient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneStamp {
    /// Owning semantic document.
    pub document: DocumentId,
    /// Layout-session lifetime.
    pub epoch: LayoutEpoch,
    /// Accepted layout submission.
    pub request: LayoutRequestId,
    /// Semantic state used for this layout.
    pub revision: Revision,
    /// Layout-profile version.
    pub profile: ProfileGeneration,
    /// Font/resource version.
    pub resources: ResourceGeneration,
}
