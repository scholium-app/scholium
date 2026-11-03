//! File operations around the existing page editor; no second document authority.
use super::*;
impl EditorSession {
    pub(crate) fn unsaved_project(&self) -> bool {
        self.core.dirty() || self.core.deferred_len() != 0
    }
    pub(crate) fn pending_input(&self) -> bool {
        self.core.deferred_len() != 0
    }
    pub(crate) fn save_project(&mut self) -> Result<(), SessionError> {
        self.core.save().map_err(Into::into)
    }
    pub(crate) fn discard_project(&mut self) -> Result<(), SessionError> {
        self.core.discard_project().map_err(Into::into)
    }
    pub(crate) fn save_as_project(
        &mut self,
        target: &std::path::Path,
    ) -> Result<scholium_storage::project::ProjectHandle, SessionError> {
        let handle = self.core.save_as_project(target)?;
        self.title = Some(crate::project_workspace::name(&handle));
        Ok(handle)
    }
}
