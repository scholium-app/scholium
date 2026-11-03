//! Restore and save the single local authority; no UI or environment policy.
use super::*;
use scholium_storage::project::{NativeProject, ProjectHandle};

#[derive(Debug)]
pub(super) enum Backing {
    Legacy(SessionStore),
    Project(Box<NativeProject>),
}

#[cfg(test)]
impl Backing {
    pub(super) fn legacy(&self) -> &SessionStore {
        match self {
            Self::Legacy(store) => store,
            Self::Project(_) => panic!("legacy fixture required"),
        }
    }
}

/// Failure opening, migrating or persisting a local editing session.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// Formal project open/commit failure.
    #[error(transparent)]
    Project(#[from] scholium_storage::project::StoreError),
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
    /// The derived layout worker could not start.
    #[error(transparent)]
    Worker(#[from] scholium_typst::editor::EditorError),
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
        let has_saved = persisted.is_some();
        let session = match persisted {
            Some(p) => LocalSession::restore_structured(p.snapshot, p.requests)?,
            None => blank()?,
        };
        let saved = has_saved.then(|| session.shared_snapshot());
        Self::from_authority(session, Backing::Legacy(store), saved)
    }

    /// Open a formal project and restore durable Actions after its last checkpoint.
    ///
    /// # Errors
    /// Returns exclusive-lock, schema, journal or document validation errors.
    pub fn open_project(handle: ProjectHandle) -> Result<Self, SessionError> {
        let project = NativeProject::open(handle, blank()?.shared_snapshot().as_ref().clone())?;
        let session = LocalSession::restore_structured(
            project.document().clone(),
            project.requests().to_vec(),
        )?;
        let saved = Some(Arc::new(project.saved_document().clone()));
        Self::from_authority(session, Backing::Project(Box::new(project)), saved)
    }

    fn from_authority(
        session: LocalSession<StructuredDocument>,
        store: Backing,
        saved: Option<Arc<StructuredDocument>>,
    ) -> Result<Self, SessionError> {
        let snapshot = session.shared_snapshot();
        let first = input::leaves(&snapshot)
            .first()
            .map(|l| l.id)
            .ok_or(SessionError::NoInputLeaf)?;
        Ok(Self {
            math_input: Default::default(),
            runtime: None,
            deferred: Default::default(),
            wanted: session.scene_stamp(ProfileGeneration(0), ResourceGeneration(0)),
            scene: None,
            session: ReadAuthority::new(session),
            snapshot,
            store,
            pending_persistence: Default::default(),
            persistence_error: None,
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
            view: ViewRange {
                top_pt: 0.0,
                bottom_pt: 800.0,
            },
            raster: RasterSpec {
                px_per_pt: 1.0,
                max_side: 4096,
            },
            pointer: None,
            geometry: None,
            geometry_stamp: None,
            goal: None,
            follow: FollowState::default(),
            pointer_drag: false,
            #[cfg(test)]
            fail_next_input: false,
        })
    }

    /// Immutable access to accepted action identities and the authority's layout lifetime.
    pub fn session(&self) -> &LocalSession<StructuredDocument> {
        &self.session
    }

    /// Allocate a complete Scene identity for the current authority.
    pub fn scene_stamp(&self) -> SceneStamp {
        self.session
            .scene_stamp(ProfileGeneration(0), ResourceGeneration(0))
    }

    /// Whether the accepted content differs from the last successfully saved head.
    pub fn dirty(&self) -> bool {
        !self.pending_persistence.is_empty()
            || self.persistence_error.is_some()
            || self.saved.as_ref() != Some(&self.snapshot)
    }

    /// Persist accepted content. Called only after command success and outside IME preedit.
    ///
    /// # Errors
    /// Returns the store's transaction failure; the saved baseline changes only after success.
    pub fn save(&mut self) -> Result<(), SessionError> {
        let requests: Vec<_> = self.session.actions().iter().map(|a| a.request).collect();
        self.persist_pending()?;
        match &mut self.store {
            Backing::Legacy(store) => store.save_structured(&self.snapshot, &requests)?,
            Backing::Project(project) => project.save()?,
        }
        self.saved = Some(self.snapshot.clone());
        Ok(())
    }

    pub(super) fn persist_pending(&mut self) -> Result<(), SessionError> {
        let Backing::Project(project) = &mut self.store else {
            self.pending_persistence.clear();
            return Ok(());
        };
        while let Some(entry) = self.pending_persistence.front() {
            project.append_edit(entry.snapshot.0.as_ref().clone(), entry.requests.to_vec())?;
            self.pending_persistence.pop_front();
        }
        self.persistence_error = None;
        Ok(())
    }

    /// Discard private edits by appending restoration of the last saved native state.
    ///
    /// # Errors
    /// Returns validation/commit failures, preserving the accepted input for recovery.
    pub fn discard_project(&mut self) -> Result<(), SessionError> {
        let Backing::Project(project) = &self.store else {
            return Ok(());
        };
        let snapshot = project.saved_document().clone();
        // Saved journal is restored from its revision prefix only when it is available.
        // History restoration can replace the local journal; the checkpoint stores its own IDs.
        let requests = project.saved_requests().to_vec();
        self.restore_document(snapshot, requests)
    }

    /// Create an independent native copy of the accepted state, keeping the original project.
    ///
    /// # Errors
    /// Rejects an existing target or a failed publication/checkpoint. Failed copies are retained.
    pub fn save_as_project(&mut self, target: &Path) -> Result<ProjectHandle, SessionError> {
        let handle = match &self.store {
            Backing::Project(project) => project.handle().copy_native(target)?,
            Backing::Legacy(_) => {
                ProjectHandle::create(target, scholium_storage::project::ProjectLanguage::Typst)?
            }
        };
        let mut project =
            NativeProject::open(handle.clone(), blank()?.shared_snapshot().as_ref().clone())?;
        let requests = self.session.actions().iter().map(|a| a.request).collect();
        project.append_edit(self.snapshot.as_ref().clone(), requests)?;
        project.save()?;
        self.store = Backing::Project(Box::new(project));
        self.pending_persistence.clear();
        self.persistence_error = None;
        self.saved = Some(self.snapshot.clone());
        Ok(handle)
    }

    /// Restore an externally supplied local document through the same validation as open.
    /// This is restoration, not a collaborative undo or an unchecked write handle.
    ///
    /// # Errors
    /// Rejects invalid document structure or action identities without changing the authority.
    pub fn restore_document(
        &mut self,
        snapshot: StructuredDocument,
        requests: impl AsRef<[RequestId]>,
    ) -> Result<(), SessionError> {
        self.session = ReadAuthority::new(LocalSession::restore_structured_shared(
            Arc::new(snapshot),
            requests.as_ref(),
        )?);
        self.snapshot = self.session.shared_snapshot();
        if matches!(self.store, Backing::Project(_)) {
            self.pending_persistence.push_back(self.record());
            self.persist_pending()?;
        }
        Ok(())
    }

    pub(super) fn new_document(&mut self) -> Result<(), CommandError> {
        if matches!(self.store, Backing::Project(_)) {
            return Err(CommandError::Arguments);
        }
        self.math_input.reset();
        self.session = ReadAuthority::new(blank()?);
        self.anchor = None;
        self.placeholder = None;
        self.undo.clear();
        self.run = None;
        self.redo.clear();
        self.saved = None;
        self.layout_error = None;
        self.overflows.clear();
        self.snapshot = self.session.shared_snapshot();
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

#[cfg(test)]
mod project_tests {
    use super::*;
    #[test]
    fn failed_or_panicking_project_commands_never_publish_partial_actions() {
        let root = std::env::temp_dir().join(format!("t39-rollback-{:?}", RequestId::fresh()));
        let handle =
            ProjectHandle::create(&root, scholium_storage::project::ProjectLanguage::Typst)
                .expect("create");
        let mut core = EditorCore::open_project(handle.clone()).expect("open");
        core.run(
            CommandId::Text,
            CommandArgs::Text("accepted".into(), Origin::Paste),
        )
        .expect("accepted");
        let expected = core.snapshot.clone();
        for id in [CommandId::FailAfterMutation, CommandId::PanicAfterMutation] {
            assert!(core.run(id, CommandArgs::None).is_err());
            assert_eq!(core.snapshot, expected);
        }
        drop(core);
        let restored = EditorCore::open_project(handle.clone()).expect("recover");
        assert_eq!(restored.snapshot, expected);
        drop(restored);
        let store = scholium_storage::project::ProjectStore::open(
            handle.root(),
            scholium_storage::project::OpenPolicy::Full,
        )
        .expect("store")
        .store;
        assert_eq!(
            store
                .actions_after(scholium_storage::project::DurableSequence(0))
                .expect("actions")
                .len(),
            2
        );
        drop(store);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
