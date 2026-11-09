//! Cold-path rollback of engine state AND the device UndoManager. A wire snapshot alone
//! cannot restore that manager. Keep encoded device steps, reconstruct only on rejection.
use crate::{Action, Error, Replica};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) enum Step {
    Group,
    Coalesce(Vec<scholium_model::RequestId>, Action),
    Edit(Action),
    Remote(Action),
    Compensate(bool, Option<Action>),
}
#[derive(Debug)]
pub(crate) struct Journal {
    id: scholium_model::RequestId,
    seed: Arc<[u8]>,
    steps: Vec<Step>,
}
/// In-memory command rollback boundary, including actor undo/redo. Never persisted.
#[derive(Debug)]
pub struct ReplicaCheckpoint {
    journal: scholium_model::RequestId,
    length: usize,
    version: loro::VersionVector,
    receipts: usize,
}
impl Replica {
    /// Start an O(1) memory transaction boundary, retaining all actor history since attach.
    /// # Errors
    /// Returns snapshot/codec failures before command execution starts.
    pub fn checkpoint(&mut self) -> Result<ReplicaCheckpoint, Error> {
        if self.journal.is_none() {
            self.journal = Some(Journal {
                id: scholium_model::RequestId::fresh(),
                seed: self.snapshot()?.into(),
                steps: Vec::new(),
            });
        }
        Ok(ReplicaCheckpoint {
            journal: self.journal.as_ref().ok_or(Error::Identity)?.id,
            version: self.doc.oplog_vv(),
            receipts: self.receipts.len(),
            length: self.journal.as_ref().ok_or(Error::Identity)?.steps.len(),
        })
    }
    pub(crate) fn remember(&mut self, step: Step) {
        if let Some(journal) = &mut self.journal {
            journal.steps.push(step);
        }
    }
    /// Roll back a rejected command, retaining the prior actor undo/redo stack.
    /// Reconstruction is a cold path; successful keystrokes never replay device history.
    /// # Errors
    /// Rejects a checkpoint from another replica or an inconsistent device journal.
    pub fn rollback(&mut self, checkpoint: ReplicaCheckpoint) -> Result<(), Error> {
        let journal = self.journal.as_ref().ok_or(Error::Identity)?;
        if checkpoint.journal != journal.id || checkpoint.length > journal.steps.len() {
            return Err(Error::Identity);
        }
        if checkpoint.length == journal.steps.len()
            && checkpoint.version == self.doc.oplog_vv()
            && checkpoint.receipts == self.receipts.len()
        {
            return Ok(());
        }
        let mut restored = Self::restore(&journal.seed, self.actor)?;
        // Replay reconstructs the original journal below; do not record a second copy.
        restored.journal = None;
        for step in &journal.steps[..checkpoint.length] {
            restored.replay_device_step(step)?;
        }
        let mut journal = self.journal.take().ok_or(Error::Identity)?;
        journal.steps.truncate(checkpoint.length);
        restored.journal = Some(journal);
        *self = restored;
        Ok(())
    }
    /// Publish exactly one Action for a command containing several internal operations
    /// (for example settling a temporary slot and then splitting its paragraph).
    /// # Errors
    /// Returns encoding/capacity failures; caller rolls back the command checkpoint.
    pub fn finish_command(
        &mut self,
        checkpoint: &ReplicaCheckpoint,
        actions: Vec<Action>,
    ) -> Result<Option<Action>, Error> {
        if self
            .journal
            .as_ref()
            .is_none_or(|j| j.id != checkpoint.journal)
        {
            return Err(Error::Identity);
        }
        if actions.len() <= 1 {
            return Ok(actions.into_iter().next());
        }
        let ids: Vec<_> = actions.iter().map(Action::id).collect();
        let header = actions.last().ok_or(Error::Format)?.header.clone();
        for id in &ids {
            self.receipts.remove(&crate::codec::key(id));
        }
        let action = self.finish(header, &checkpoint.version)?;
        self.remember(Step::Coalesce(ids, action.clone()));
        Ok(Some(action))
    }
    fn replay_device_step(&mut self, step: &Step) -> Result<(), Error> {
        match step {
            Step::Group => self.start_undo_group(),
            Step::Coalesce(ids, action) => {
                for id in ids {
                    self.receipts.remove(&crate::codec::key(id));
                }
                self.receipts.insert(
                    crate::codec::key(&action.id()),
                    crate::replica::fingerprint(&action.encode()?),
                );
                Ok(())
            }
            Step::Remote(action) => self.receive(action).map(|_| ()),
            Step::Edit(action) => {
                let staged = self.doc.fork();
                staged.import(&action.update)?;
                let after = self.project(&staged, self.receipts.len() + 1)?.document;
                let generated = self.apply_effects(
                    action.id(),
                    action.origin().clone(),
                    self.document()?.revision,
                    after,
                )?;
                self.verify_replayed(generated.as_ref(), Some(action))
            }
            Step::Compensate(redo, action) => {
                let generated = self.compensate_with_id(*redo, action.as_ref().map(Action::id))?;
                self.verify_replayed(generated.as_ref(), action.as_ref())
            }
        }
    }
    fn verify_replayed(
        &mut self,
        generated: Option<&Action>,
        original: Option<&Action>,
    ) -> Result<(), Error> {
        match (generated, original) {
            (None, None) => Ok(()),
            (Some(generated), Some(original)) if generated == original => Ok(()),
            _ => Err(Error::Identity),
        }
    }
}
