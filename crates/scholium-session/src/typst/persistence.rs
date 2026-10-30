//! Restore and save the single local authority; no UI or environment policy.
use super::*;

/// Failure opening, migrating or persisting a local editing session.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// The document contains no supported editable leaf.
    #[error("no supported text or math input leaf")]
    NoInputLeaf,
    /// A stored head or schema could not be opened or saved.
    #[error(transparent)]
    Storage(#[from] scholium_storage::StructuredStoreError),
    /// The restored structured document is invalid.
    #[error(transparent)]
    Edit(#[from] scholium_document::EditError),
    /// The filesystem could not be inspected.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl EditorCore {
    /// Open or create a structured store and restore its saved head.
    ///
    /// # Errors
    /// Returns schema, filesystem or document validation failures without replacing the file.
    pub fn open(path: &Path) -> Result<Self, SessionError> {
        let store = if path.try_exists()? {
            SessionStore::open_structured(path)?
        } else {
            SessionStore::create_structured(path)?
        };
        let persisted = store.load_structured()?;
        let saved = persisted.as_ref().map(|p| p.snapshot.clone());
        let session = match persisted {
            Some(p) => LocalSession::restore_structured(p.snapshot, p.requests)?,
            None => blank()?,
        };
        let snapshot = Arc::new(session.snapshot());
        let first = input::leaves(&snapshot)
            .first()
            .map(|l| l.id)
            .ok_or(SessionError::NoInputLeaf)?;
        Ok(Self {
            session: ReadAuthority::new(session),
            snapshot,
            store,
            saved,
            cursor: Position {
                leaf: first,
                byte: 0,
                affinity: Affinity::Downstream,
            },
            anchor: None,
            undo: Vec::new(),
            redo: Vec::new(),
            run: None,
            clock: history::system_clock(),
            placeholder: None,
            pending_style: None,
            layout_error: None,
            rejected_input: None,
            overflows: Vec::new(),
        })
    }

    /// Immutable access to accepted action identities and the authority's layout lifetime.
    pub fn session(&self) -> &LocalSession<StructuredDocument> {
        &self.session
    }

    /// Allocate a Scene identity for the current authority; scenes are owned by app.
    pub fn scene_stamp(&self) -> SceneStamp {
        self.session
            .scene_stamp(ProfileGeneration(0), ResourceGeneration(0))
    }

    /// Whether the accepted content differs from the last successfully saved head.
    pub fn dirty(&self) -> bool {
        self.saved.as_ref() != Some(&self.snapshot)
    }

    /// Persist accepted content. Called only after command success and outside IME preedit.
    ///
    /// # Errors
    /// Returns the store's transaction failure; the saved baseline changes only after success.
    pub fn save(&mut self) -> Result<(), SessionError> {
        let requests: Vec<_> = self.session.actions().iter().map(|a| a.request).collect();
        self.store.save_structured(&self.snapshot, &requests)?;
        self.saved = Some((*self.snapshot).clone());
        Ok(())
    }

    /// Restore an externally supplied local document through the same validation as open.
    /// This is restoration, not a collaborative undo or an unchecked write handle.
    ///
    /// # Errors
    /// Rejects invalid document structure or action identities without changing the authority.
    pub fn restore_document(
        &mut self,
        snapshot: StructuredDocument,
        requests: Vec<RequestId>,
    ) -> Result<(), SessionError> {
        self.session = ReadAuthority::new(LocalSession::restore_structured(snapshot, requests)?);
        self.snapshot = Arc::new(self.session.snapshot());
        Ok(())
    }

    pub(super) fn new_document(&mut self) -> Result<(), CommandError> {
        self.session = ReadAuthority::new(blank()?);
        self.anchor = None;
        self.placeholder = None;
        self.undo.clear();
        self.run = None;
        self.redo.clear();
        self.saved = None;
        self.layout_error = None;
        self.overflows.clear();
        self.snapshot = Arc::new(self.session.snapshot());
        if let Some(leaf) = input::leaves(&self.snapshot).first() {
            self.cursor = Position {
                leaf: leaf.id,
                byte: 0,
                affinity: Affinity::Downstream,
            };
        }
        Ok(())
    }
}

fn blank() -> Result<LocalSession<StructuredDocument>, scholium_document::EditError> {
    LocalSession::default()
        .into_structured()
        .map(|(session, _)| session)
        .map_err(|e| e.error.into())
}

/// Migrate a saved legacy head only if the structured target does not exist.
/// Both probe and migration preserve the original file; a competing published target wins.
///
/// # Errors
/// Returns filesystem or migration failures, leaving both authorities untouched on rejection.
pub fn prepare(
    target: &Path,
    legacy: &Path,
) -> Result<Option<scholium_storage::FileMigrationReport>, SessionError> {
    if target.try_exists()? || !legacy.try_exists()? {
        return Ok(None);
    }
    let migrated = scholium_storage::legacy_has_head(legacy).and_then(|head| {
        head.then(|| scholium_storage::migrate_legacy_file(legacy, target))
            .transpose()
    });
    match migrated {
        Ok(report) => Ok(report),
        Err(_) if target.try_exists()? => Ok(None),
        Err(error) => Err(error.into()),
    }
}
