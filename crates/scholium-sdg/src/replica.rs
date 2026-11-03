use crate::{
    Action, Error, Origin, Projection,
    codec::{self, ActionHeader, SnapshotHeader, key},
    graph::{self, Graph},
    lower, project,
};
use loro::{ExportMode, LoroDoc, UndoManager};
use scholium_document::{LocalSession, StructuralEdit, StructuralRequest};
use scholium_model::{DocumentId, NodeId, RequestId, Revision, structured::StructuredDocument};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const MAX_ACTIONS: usize = 100_000;
const MAX_PENDING_BYTES: usize = 1024 * 1024;
const MAX_UNDO_STEPS: usize = 1000;

/// Opaque stable tree identity, comparable across snapshots without exposing Loro types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeIdentity {
    /// Creating device actor.
    pub actor: u64,
    /// Creating operation counter.
    pub counter: i32,
}
/// Sole mutable structured authority for a device actor; session/store integration is external.
#[derive(Debug)]
pub struct Replica {
    doc: LoroDoc,
    undo: UndoManager,
    document: DocumentId,
    actor: u64,
    base_revision: u64,
    receipts: BTreeMap<String, [u8; 32]>,
    pending: BTreeMap<String, Vec<u8>>,
}
impl Replica {
    /// Bootstrap once from a valid identified document. Other writers restore this snapshot.
    /// # Errors
    /// Rejects invalid structure, zero actors, identity collisions or engine/capacity errors.
    pub fn new(initial: StructuredDocument, actor: u64) -> Result<Self, Error> {
        initial.validate()?;
        let doc = engine(actor)?;
        lower::bootstrap(&doc, &initial)?;
        let result = Self::attach(
            doc,
            actor,
            SnapshotHeader {
                document: initial.document,
                base_revision: initial.revision.0,
                receipts: BTreeMap::new(),
                pending: BTreeMap::new(),
            },
        );
        result.projection()?;
        Ok(result)
    }
    /// Restore a version 1 snapshot, keeping all node/operation identities and pending updates.
    /// Undo stacks start empty; a caller must use a unique actor for each concurrent writer.
    /// # Errors
    /// Rejects unsupported/malformed/capacity snapshots and mismatched document identities.
    pub fn restore(bytes: &[u8], actor: u64) -> Result<Self, Error> {
        let (header, payload) = codec::unpack::<SnapshotHeader>(codec::SNAPSHOT_MAGIC, bytes)?;
        validate_header(&header)?;
        let doc = engine(actor)?;
        doc.import(payload)?;
        for (id, bytes) in &header.pending {
            let action = Action::decode(bytes)?;
            if key(&action.id()) != *id
                || action.header.document != header.document
                || header.receipts.get(id) != Some(&fingerprint(bytes))
            {
                return Err(Error::Identity);
            }
            doc.import(&action.update)?;
        }
        let result = Self::attach(doc, actor, header);
        result.check_document(&result.doc)?;
        result.projection()?;
        Ok(result)
    }
    fn attach(doc: LoroDoc, actor: u64, header: SnapshotHeader) -> Self {
        let mut undo = UndoManager::new(&doc);
        undo.add_exclude_origin_prefix("system/");
        undo.add_exclude_origin_prefix("remote/");
        undo.set_merge_interval(0);
        undo.set_max_undo_steps(MAX_UNDO_STEPS);
        Self {
            doc,
            undo,
            actor,
            document: header.document,
            base_revision: header.base_revision,
            receipts: header.receipts,
            pending: header.pending,
        }
    }
    /// Valid structured projection with explicit deterministic repair diagnostics.
    /// # Errors
    /// Returns schema/identity/capacity errors if internal data is malformed.
    pub fn projection(&self) -> Result<Projection, Error> {
        self.project(&self.doc, self.receipts.len())
    }
    /// Read the document projection; use projection() when displaying conflict repairs.
    /// # Errors
    /// Uses the same validation as projection().
    pub fn document(&self) -> Result<StructuredDocument, Error> {
        Ok(self.projection()?.document)
    }
    /// Number of accepted Actions still awaiting causal predecessors.
    /// Fetch/replay dependencies before treating the replica as fully synchronized.
    pub fn pending_actions(&self) -> usize {
        self.pending.len()
    }
    /// Stable one-to-one identity, including retained tombstones and repair reserves.
    /// # Errors
    /// Rejects unknown nodes or malformed internal mappings.
    pub fn tree_identity(&self, node: NodeId) -> Result<TreeIdentity, Error> {
        let graph = Graph::read(&self.doc)?;
        let id = graph.get(node)?.tree;
        Ok(TreeIdentity {
            actor: id.peer,
            counter: id.counter,
        })
    }
    /// Plan an existing StructuralEdit and lower its effects as one user Action.
    /// No-op edits return None without consuming an identity or undo entry.
    /// # Errors
    /// Rejects invalid edits and the same conditions as apply_effects().
    pub fn edit(&mut self, edit: StructuralEdit, name: &str) -> Result<Option<Action>, Error> {
        let current = self.document()?;
        let mut planned = current.clone();
        planned.revision = Revision(0);
        // A temporary planner is consumed here; it is never a second mutable authority.
        let mut planner = LocalSession::restore_structured(planned, Vec::new())?;
        let id = RequestId::fresh();
        if !planner.apply_structural(StructuralRequest {
            document: self.document,
            base: Revision(0),
            request: id,
            edit,
        })? {
            return Ok(None);
        }
        self.apply_effects(
            id,
            Origin::User(name.into()),
            current.revision,
            planner.snapshot(),
        )
    }
    /// Lower an already validated planner result, preserving surviving identities/text runs.
    /// Expected revision prevents replacing a newer projection with stale planner effects.
    /// # Errors
    /// Rejects stale/wrong documents, reused IDs, invalid origins/structure and engine errors.
    /// Planning and validation happen on a fork before changing this replica.
    pub fn apply_effects(
        &mut self,
        id: RequestId,
        origin: Origin,
        expected: Revision,
        after: StructuredDocument,
    ) -> Result<Option<Action>, Error> {
        self.check_capacity()?;
        let before = self.document()?;
        if expected != before.revision {
            return Err(scholium_document::EditError::StaleRevision.into());
        }
        if after.document != self.document {
            return Err(Error::Identity);
        }
        if self.receipts.contains_key(&key(&id)) {
            return Err(Error::Duplicate);
        }
        after.validate()?;
        if before.blocks == after.blocks {
            return Ok(None);
        }
        let header = ActionHeader {
            document: self.document,
            id,
            actor: self.actor,
            origin,
        };
        // Validate origin and the exact operations before applying them to the live undo scope.
        header.origin.validate()?;
        let version = self.doc.oplog_vv();
        self.preflight(&before, &after, &header, &version)?;
        self.doc
            .set_next_commit_origin(&header.origin.engine_name());
        lower::apply(
            &self.doc,
            &before,
            &after,
            &header.origin.engine_name(),
            header.id,
        )?;
        self.undo.record_new_checkpoint()?;
        self.finish(header, &version).map(Some)
    }
    fn preflight(
        &self,
        before: &StructuredDocument,
        after: &StructuredDocument,
        header: &ActionHeader,
        version: &loro::VersionVector,
    ) -> Result<(), Error> {
        let staged = self.doc.fork();
        staged.set_peer_id(self.actor)?;
        staged.set_next_commit_origin(&header.origin.engine_name());
        lower::apply(
            &staged,
            before,
            after,
            &header.origin.engine_name(),
            header.id,
        )?;
        let projected = self.project(&staged, self.receipts.len() + 1)?;
        if projected.document.blocks != after.blocks {
            return Err(Error::Format);
        }
        Action {
            header: header.clone(),
            update: staged
                .export(ExportMode::updates(version))
                .map_err(|_| Error::Engine)?,
        }
        .encode()?;
        Ok(())
    }
    /// Import one Action idempotently, buffering causal gaps in Loro until dependencies arrive.
    /// # Errors
    /// Rejects reused IDs with different bytes, wrong context/actors, malformed schema or engine data.
    /// Rejection is checked on a fork and leaves the live authority untouched.
    pub fn receive(&mut self, action: &Action) -> Result<bool, Error> {
        if action.header.document != self.document || action.actor() == 0 {
            return Err(Error::Identity);
        }
        let encoded = action.encode()?;
        let digest = fingerprint(&encoded);
        let id = key(&action.id());
        if let Some(known) = self.receipts.get(&id) {
            return if *known == digest {
                Ok(false)
            } else {
                Err(Error::Duplicate)
            };
        }
        let meta = LoroDoc::decode_import_blob_meta(&action.update, true)?;
        if meta
            .partial_end_vv
            .iter()
            .any(|(&peer, _)| peer != action.actor())
            || meta.change_num == 0
        {
            return Err(Error::Identity);
        }
        self.check_capacity()?;
        let staged = self.doc.fork();
        for bytes in self.pending.values() {
            staged.import(&Action::decode(bytes)?.update)?;
        }
        let status = staged.import(&action.update)?;
        let is_pending = status.pending.is_some();
        if is_pending
            && self.pending.values().map(Vec::len).sum::<usize>() + encoded.len()
                > MAX_PENDING_BYTES
        {
            return Err(Error::Capacity);
        }
        self.check_document(&staged)?;
        self.project(&staged, self.receipts.len() + 1)?;
        self.doc.import_with(&action.update, "remote/action")?;
        self.receipts.insert(id.clone(), digest);
        if is_pending {
            self.pending.insert(id, encoded);
        }
        self.prune_pending()?;
        Ok(true)
    }
    /// Compensate one local user action without restoring a snapshot or undoing remote input.
    /// # Errors
    /// Returns engine/schema errors; a consumed no-op history item returns None.
    pub fn undo(&mut self) -> Result<Option<Action>, Error> {
        self.compensate(false)
    }
    /// Redo one local compensation, preserving remote edits and projection invariants.
    /// # Errors
    /// Returns engine/schema errors; a consumed no-op history item returns None.
    pub fn redo(&mut self) -> Result<Option<Action>, Error> {
        self.compensate(true)
    }
    fn compensate(&mut self, redo: bool) -> Result<Option<Action>, Error> {
        self.check_capacity()?;
        let version = self.doc.oplog_vv();
        let changed = if redo {
            self.undo.redo()?
        } else {
            self.undo.undo()?
        };
        if !changed {
            return Ok(None);
        }
        self.doc.commit();
        let header = ActionHeader {
            document: self.document,
            id: RequestId::fresh(),
            actor: self.actor,
            origin: if redo { Origin::Redo } else { Origin::Undo },
        };
        self.project(&self.doc, self.receipts.len() + 1)?;
        self.finish(header, &version).map(Some)
    }
    fn finish(
        &mut self,
        header: ActionHeader,
        version: &loro::VersionVector,
    ) -> Result<Action, Error> {
        let action = Action {
            header,
            update: self
                .doc
                .export(ExportMode::updates(version))
                .map_err(|_| Error::Engine)?,
        };
        let digest = fingerprint(&action.encode()?);
        self.receipts.insert(key(&action.id()), digest);
        Ok(action)
    }
    /// Snapshot the exact engine history, pending updates, base revision and Action receipt hashes.
    /// Save at a committed ProjectStore Action boundary, then replay subsequent encoded Actions.
    /// # Errors
    /// Returns codec/engine capacity errors. Undo stacks are intentionally device-session state.
    pub fn snapshot(&self) -> Result<Vec<u8>, Error> {
        let header = SnapshotHeader {
            document: self.document,
            base_revision: self.base_revision,
            receipts: self.receipts.clone(),
            pending: self.pending.clone(),
        };
        codec::pack(
            codec::SNAPSHOT_MAGIC,
            &header,
            &self
                .doc
                .export(ExportMode::Snapshot)
                .map_err(|_| Error::Engine)?,
        )
    }
    fn project(&self, doc: &LoroDoc, receipts: usize) -> Result<Projection, Error> {
        let revision = self
            .base_revision
            .checked_add(receipts as u64)
            .ok_or(Error::Capacity)?;
        project::project(&Graph::read(doc)?, self.document, Revision(revision))
    }
    fn check_capacity(&self) -> Result<(), Error> {
        if self.receipts.len() >= MAX_ACTIONS
            || self
                .base_revision
                .checked_add(self.receipts.len() as u64 + 1)
                .is_none()
        {
            return Err(Error::Capacity);
        }
        Ok(())
    }
    fn prune_pending(&mut self) -> Result<(), Error> {
        let vv = self.doc.oplog_vv();
        let mut finished = Vec::new();
        for (id, bytes) in &self.pending {
            let action = Action::decode(bytes)?;
            let meta = LoroDoc::decode_import_blob_meta(&action.update, true)?;
            if meta
                .partial_end_vv
                .iter()
                .all(|(peer, end)| vv.get(peer).is_some_and(|n| n >= end))
            {
                finished.push(id.clone());
            }
        }
        for id in finished {
            self.pending.remove(&id);
        }
        Ok(())
    }
    fn check_document(&self, doc: &LoroDoc) -> Result<(), Error> {
        let schema = doc.get_map("schema");
        if graph::string(&schema, "document")? != key(&self.document)
            || graph::string(&schema, "version")? != "1"
        {
            return Err(Error::Identity);
        }
        Ok(())
    }
}
fn engine(actor: u64) -> Result<LoroDoc, Error> {
    if actor == 0 {
        return Err(Error::Identity);
    }
    let doc = LoroDoc::new();
    doc.set_peer_id(actor)?;
    doc.get_tree("sdg").enable_fractional_index(0);
    Ok(doc)
}
fn fingerprint(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn validate_header(header: &SnapshotHeader) -> Result<(), Error> {
    if header.receipts.len() > MAX_ACTIONS
        || header.pending.values().map(Vec::len).sum::<usize>() > MAX_PENDING_BYTES
    {
        return Err(Error::Capacity);
    }
    for id in header.receipts.keys() {
        let value: RequestId = serde_json::from_str(id)?;
        if key(&value) != *id {
            return Err(Error::Format);
        }
    }
    Ok(())
}
