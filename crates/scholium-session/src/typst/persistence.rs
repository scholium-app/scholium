//! Restore and save the single local authority; no UI or environment policy.
use super::*;
use scholium_storage::project::{ProjectHandle, ProjectLanguage};
/// Failure opening, migrating or persisting a local editing session.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// SDG codec, projection or engine rejected the recovered state.
    #[error(transparent)]
    Sdg(#[from] scholium_sdg::Error),
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
        let target = path.with_extension("sdg-project");
        // Read-only import remains available if a prior attempt died after declaration
        // publication. A nonempty SDG head ignores this seed; an empty target resumes it.
        let initial = if !target.try_exists()? && path.try_exists()? {
            scholium_storage::project::import_session(path)?.map(|p| p.snapshot)
        } else {
            None
        };
        let handle = if target.try_exists()? {
            ProjectHandle::open(&target, None)?
        } else {
            ProjectHandle::create(&target, ProjectLanguage::Typst)?
        };
        let (store, replica) = sdg_project::SdgProject::open_import(
            handle,
            initial.unwrap_or(blank()?.snapshot()),
            Some(path),
        )?;
        Self::from_sdg(store, replica)
    }

    /// Open a formal project; migrate retired native v1 records once, retaining their bytes.
    /// # Errors
    /// Rejects locks, unknown versions, corrupt or incomplete causal history.
    pub fn open_project(handle: ProjectHandle) -> Result<Self, SessionError> {
        Self::open_sdg(handle, blank()?.snapshot())
    }

    fn open_sdg(handle: ProjectHandle, initial: StructuredDocument) -> Result<Self, SessionError> {
        let (store, replica) = sdg_project::SdgProject::open(handle, initial)?;
        Self::from_sdg(store, replica)
    }
    fn from_sdg(
        store: sdg_project::SdgProject,
        replica: scholium_sdg::Replica,
    ) -> Result<Self, SessionError> {
        let session = LocalSession::restore_structured(replica.document()?, replica.action_ids()?)?;
        let saved = Some(Arc::new(store.saved.clone()));
        Self::from_authority(session, Box::new(store), replica, saved)
    }

    pub(super) fn from_authority(
        session: LocalSession<StructuredDocument>,
        store: Box<sdg_project::SdgProject>,
        replica: scholium_sdg::Replica,
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
            replica,
            input_transaction: false,
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

    /// Deterministic structural repair diagnostics from the shared authority.
    /// # Errors
    /// Returns engine/schema/capacity failures while projecting malformed state.
    pub fn shared_repairs(&self) -> Result<Vec<scholium_sdg::Repair>, scholium_sdg::Error> {
        Ok(self.replica.projection()?.repairs)
    }

    /// Whether the accepted content differs from the last successfully saved head.
    pub fn dirty(&self) -> bool {
        !self.pending_persistence.is_empty()
            || self.persistence_error.is_some()
            || self.saved.as_ref().is_none_or(|saved| {
                saved.document != self.snapshot.document || saved.blocks != self.snapshot.blocks
            })
    }

    /// Persist accepted content. Called only after command success and outside IME preedit.
    ///
    /// # Errors
    /// Returns the store's transaction failure; the saved baseline changes only after success.
    pub fn save(&mut self) -> Result<(), SessionError> {
        self.persist_pending()?;
        self.store.save(&self.replica)?;
        self.saved = Some(self.snapshot.clone());
        Ok(())
    }

    pub(super) fn persist_pending(&mut self) -> Result<(), SessionError> {
        while let Some(action) = self.pending_persistence.front() {
            self.store.append(action)?;
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
        let snapshot = self.store.saved.clone();
        let mut planned = snapshot;
        planned.revision = self.snapshot.revision;
        self.restore_document(planned, self.replica.action_ids()?)
    }

    /// Create an independent SDG project containing the accepted state and resource files.
    /// # Errors
    /// Rejects existing targets and failed durable publication; preserves the original.
    pub fn save_as_project(&mut self, target: &Path) -> Result<ProjectHandle, SessionError> {
        let handle = self.store.handle.copy_native(target)?;
        let (store, replica) =
            sdg_project::SdgProject::open(handle.clone(), self.snapshot.as_ref().clone())?;
        *self.store = store;
        self.replica = replica;
        self.adopt_replica()?;
        self.undo.clear();
        self.redo.clear();
        self.run = None;
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
        LocalSession::restore_structured_shared(Arc::new(snapshot.clone()), requests.as_ref())?;
        // External restoration is an explicit append-only replacement, never an undo stack.
        // A changed document identity is only allowed for a private, newly initialized fixture.
        if snapshot.document != self.snapshot.document {
            let empty = self.snapshot.blocks.len() == 1
                && self.snapshot.blocks[0]
                    .content
                    .iter()
                    .all(|i| matches!(&i.body, InlineBody::Text { text, .. } if text.is_empty()));
            if !empty || self.snapshot.revision.0 != 1 || !self.pending_persistence.is_empty() {
                return Err(scholium_storage::project::StoreError::Corrupt(
                    "document replacement requires a fresh empty session",
                )
                .into());
            }
            let (replica, action) =
                scholium_sdg::Replica::initialize(snapshot, super::sdg_project::new_actor())?;
            self.replica = replica;
            self.pending_persistence.push_back(action);
        } else if let Some(action) = self.replica.apply_effects(
            RequestId::fresh(),
            scholium_sdg::Origin::System("restore".into()),
            self.snapshot.revision,
            snapshot,
        )? {
            self.pending_persistence.push_back(action);
        }
        self.adopt_replica()?;
        self.persist_pending()?;
        Ok(())
    }

    pub(super) fn new_document(&mut self) -> Result<(), CommandError> {
        Err(CommandError::Arguments)
    }
}

pub(super) fn blank() -> Result<LocalSession<StructuredDocument>, scholium_document::EditError> {
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
