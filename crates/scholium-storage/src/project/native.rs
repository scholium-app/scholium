//! Temporary versioned native journal, replaced by the shared SDG adapter Action format.
use super::{
    ActionRecord, Authority, DurableSequence, OpenPolicy, ProjectHandle, ProjectStore, StoreError,
};
use scholium_model::{RequestId, structured::StructuredDocument};

/// Exclusive native project journal. Semantic validation stays independent of rendering.
#[derive(Debug)]
pub struct NativeProject {
    handle: ProjectHandle,
    store: ProjectStore,
    state: NativeState,
    saved: NativeState,
}

/// Temporary absolute state payload: local undo also appends, never deletes shared history.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeState {
    format: String,
    version: u32,
    event: RequestId,
    document: StructuredDocument,
    requests: Vec<RequestId>,
}
impl NativeState {
    fn new(document: StructuredDocument, requests: Vec<RequestId>) -> Result<Self, StoreError> {
        let state = Self {
            format: "scholium.temporary.native".into(),
            version: 1,
            event: RequestId::fresh(),
            document,
            requests,
        };
        state.validate()?;
        Ok(state)
    }
    fn validate(&self) -> Result<(), StoreError> {
        if self.format != "scholium.temporary.native" || self.version != 1 {
            return Err(StoreError::Corrupt("native envelope version"));
        }
        self.document
            .validate()
            .map_err(|_| StoreError::Corrupt("native document"))?;
        crate::structured::validate_requests(self.document.revision.0, &self.requests)
            .map_err(|_| StoreError::Corrupt("native request journal"))
    }
    fn decode(bytes: &[u8]) -> Result<Self, StoreError> {
        let state: Self =
            serde_json::from_slice(bytes).map_err(|_| StoreError::Corrupt("native envelope"))?;
        state.validate()?;
        Ok(state)
    }
    fn encode(&self) -> Result<Vec<u8>, StoreError> {
        serde_json::to_vec(self).map_err(|_| StoreError::Corrupt("native encoding"))
    }
}
impl NativeProject {
    /// Open and validate snapshot plus subsequent Actions, or durably initialize the supplied blank.
    ///
    /// # Errors
    /// Rejects wrong authority, lock conflicts, corruption and failed durable initialization.
    pub fn open(handle: ProjectHandle, initial: StructuredDocument) -> Result<Self, StoreError> {
        if handle.authority() != Authority::Structured {
            return Err(StoreError::InvalidResource);
        }
        let report = ProjectStore::open(handle.root(), OpenPolicy::Full)?;
        if !report.recovery.steps.is_empty() {
            return Err(StoreError::RecoveryRequired);
        }
        let mut store = report.store;
        let snapshot = store.latest_snapshot()?;
        let sequence = snapshot.as_ref().map_or(DurableSequence(0), |s| s.sequence);
        let mut state = snapshot
            .as_ref()
            .map(|s| NativeState::decode(&s.bytes))
            .transpose()?;
        let saved = state.as_ref().cloned();
        for action in store.actions_after(sequence)? {
            if action.record.version != 1 {
                return Err(StoreError::Corrupt("native action version"));
            }
            state = Some(NativeState::decode(&action.record.payload)?);
        }
        let state = match state {
            Some(state) => state,
            None => {
                let state = NativeState::new(initial, Vec::new())?;
                let bytes = state.encode()?;
                let sequence = store.append_action(&ActionRecord::new(bytes.clone()))?;
                store.save_snapshot(sequence, &bytes)?;
                state
            }
        };
        let saved = saved.unwrap_or_else(|| state.clone());
        Ok(Self {
            handle,
            store,
            state,
            saved,
        })
    }
    /// Project descriptor.
    pub fn handle(&self) -> &ProjectHandle {
        &self.handle
    }
    /// Current accepted document.
    pub fn document(&self) -> &StructuredDocument {
        &self.state.document
    }
    /// Accepted request identities in the current local state.
    pub fn requests(&self) -> &[RequestId] {
        &self.state.requests
    }
    /// Last explicit checkpoint, before replayed unsaved Actions.
    pub fn saved_document(&self) -> &StructuredDocument {
        &self.saved.document
    }
    /// Local request IDs at the last explicit checkpoint.
    pub fn saved_requests(&self) -> &[RequestId] {
        &self.saved.requests
    }
    /// Append one accepted request (or explicit local history restoration) before acknowledgement.
    ///
    /// # Errors
    /// Rejects invalid state or failed durable commit; the prior state remains available.
    pub fn append_edit(
        &mut self,
        document: StructuredDocument,
        requests: Vec<RequestId>,
    ) -> Result<(), StoreError> {
        let state = NativeState::new(document, requests)?;
        self.store
            .append_action(&ActionRecord::new(state.encode()?))?;
        self.state = state;
        Ok(())
    }
    /// Save a versioned structured snapshot at the latest durable Action boundary.
    ///
    /// # Errors
    /// Returns failed validation or durable checkpoint errors.
    pub fn save(&mut self) -> Result<(), StoreError> {
        let sequence = self.store.latest_action_sequence()?;
        // The same boundary is immutable; a repeated save is a no-op.
        if self
            .store
            .latest_snapshot()?
            .is_none_or(|s| s.sequence != sequence)
        {
            self.store.save_snapshot(sequence, &self.state.encode()?)?;
        }
        self.saved = self.state.clone();
        Ok(())
    }
}
