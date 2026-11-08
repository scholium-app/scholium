//! Durable SDG authority; ProjectStore handles opaque bytes and never depends on the engine.
use super::*;
use scholium_sdg::{Action, Replica};
use scholium_storage::project::{
    ActionRecord, Authority, DurableSequence, ProjectHandle, ProjectStore, StoreError,
    import_native_v1,
};

#[derive(Debug)]
pub(super) struct SdgProject {
    pub handle: ProjectHandle,
    pub store: ProjectStore,
    pub sequence: DurableSequence,
    pub saved: StructuredDocument,
}
impl SdgProject {
    pub fn open(
        handle: ProjectHandle,
        initial: StructuredDocument,
    ) -> Result<(Self, Replica), SessionError> {
        Self::open_import(handle, initial, None)
    }
    pub(super) fn open_import(
        handle: ProjectHandle,
        initial: StructuredDocument,
        legacy: Option<&Path>,
    ) -> Result<(Self, Replica), SessionError> {
        if handle.authority() != Authority::Structured {
            return Err(StoreError::InvalidResource.into());
        }
        let report = ProjectStore::open(handle.root(), Default::default())?;
        if !report.recovery.steps.is_empty() {
            return Err(StoreError::RecoveryRequired.into());
        }
        let initial = if let Some(path) = legacy
            && path.try_exists()?
            && report.store.latest_snapshot()?.is_none()
            && report.store.actions_after(Default::default())?.is_empty()
        {
            scholium_storage::project::import_session(path)?.map_or(initial, |p| p.snapshot)
        } else {
            initial
        };
        Self::recover(handle, initial, report.store)
    }
    pub(super) fn recover(
        handle: ProjectHandle,
        initial: StructuredDocument,
        mut store: ProjectStore,
    ) -> Result<(Self, Replica), SessionError> {
        let checkpoint = store.latest_snapshot()?;
        let mut sequence = checkpoint.as_ref().map_or_default(|s| s.sequence);
        let actor = new_actor();
        let mut old = None;
        let mut replica = if let Some(snapshot) = &checkpoint {
            if snapshot.bytes.starts_with(b"SCSDGS01") {
                Some(Replica::restore(&snapshot.bytes, actor)?)
            } else {
                old = Some(import_native_v1(&snapshot.bytes)?);
                None
            }
        } else {
            None
        };
        let mut saved = replica
            .as_ref()
            .map(Replica::document)
            .transpose()?
            .or_else(|| old.clone());
        for stored in store.actions_after(sequence)? {
            sequence = stored.sequence;
            if stored.record.payload.starts_with(b"SCSDGA01") {
                let action = Action::decode(&stored.record.payload)?;
                if action.origin() == &scholium_sdg::Origin::System("initialize-v1".into()) {
                    replica = Some(Replica::from_initial_action(&action, actor)?);
                    // Initialization is a saved seed even if killed before its first snapshot.
                    // Later replayed Actions must still differ from this baseline until save.
                    saved = replica.as_ref().map(Replica::document).transpose()?;
                } else if let Some(current) = &mut replica {
                    current.receive(&action)?;
                } else {
                    replica = Some(Replica::from_initial_action(&action, actor)?);
                }
            } else if replica.is_none() {
                old = Some(import_native_v1(&stored.record.payload)?);
            } else {
                return Err(
                    StoreError::Corrupt("retired native record after SDG migration").into(),
                );
            }
        }
        let replica = match replica {
            Some(replica) => replica,
            None => {
                let (replica, action) = Replica::initialize(old.unwrap_or(initial), actor)?;
                sequence = store.append_action(&ActionRecord::new(action.encode()?))?;
                // If killed between these commits, the initialization Action contains the seed.
                store.save_snapshot(sequence, &replica.snapshot()?)?;
                saved = Some(replica.document()?);
                replica
            }
        };
        if replica.pending_actions() != 0 {
            return Err(StoreError::Corrupt("incomplete SDG causal history").into());
        }
        let saved = saved.unwrap_or(replica.document()?);
        Ok((
            Self {
                handle,
                store,
                sequence,
                saved,
            },
            replica,
        ))
    }
    pub fn append(&mut self, action: &Action) -> Result<(), SessionError> {
        self.sequence = self
            .store
            .append_action(&ActionRecord::new(action.encode()?))?;
        Ok(())
    }
    pub fn save(&mut self, replica: &Replica) -> Result<(), SessionError> {
        if self
            .store
            .latest_snapshot()?
            .is_none_or(|s| s.sequence != self.sequence)
        {
            self.store
                .save_snapshot(self.sequence, &replica.snapshot()?)?;
        }
        self.saved = replica.document()?;
        Ok(())
    }
}
pub(super) fn new_actor() -> u64 {
    // UUID entropy gives each device session a separate writer, including after a crash/restart.
    let bytes = scholium_model::NodeId::fresh().as_bytes();
    u64::from_le_bytes(bytes[..8].try_into().expect("eight-byte UUID prefix")).max(1)
}

impl EditorCore {
    pub(super) fn adopt_replica_if_changed(&mut self) -> Result<(), scholium_sdg::Error> {
        if self.replica.document()? != *self.snapshot {
            self.adopt_replica()?;
        }
        debug_assert_eq!(self.replica.document()?, *self.session.shared_snapshot());
        Ok(())
    }
    pub(super) fn adopt_replica(&mut self) -> Result<(), scholium_sdg::Error> {
        let projection = self.replica.projection()?;
        let session =
            LocalSession::restore_structured(projection.document, self.replica.action_ids()?)?;
        self.session = ReadAuthority::new(session);
        self.snapshot = self.session.shared_snapshot();
        debug_assert_eq!(self.replica.document()?, *self.snapshot);
        Ok(())
    }
    pub(super) fn lower_command(&mut self, state: &mut EditState) -> Result<(), CommandError> {
        let current = self.replica.document()?;
        if current.blocks == self.snapshot.blocks && current.document == self.snapshot.document {
            return Ok(());
        }
        let after = self.snapshot.as_ref().clone();
        let origin = scholium_sdg::Origin::User("editor-command".into());
        // All intent planning and injected failures have completed. apply_effects validates the
        // complete lowering on a fork before changing the live device undo scope.
        if state.new_group {
            self.replica.start_undo_group()?;
        }
        if let Some(action) =
            self.replica
                .apply_effects(RequestId::fresh(), origin, current.revision, after)?
        {
            state.actions.push(action);
        }
        self.adopt_replica()?;
        Ok(())
    }
}
