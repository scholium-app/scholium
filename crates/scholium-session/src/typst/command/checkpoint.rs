//! A prevalidated replacement authority makes rollback infallible after unwinding.
use super::*;
#[cfg(test)]
mod tests;

pub(in crate::typst) struct Checkpoint {
    authority: LocalSession<StructuredDocument>,
    pub(in crate::typst) snapshot: Arc<StructuredDocument>,
    cursor: Position,
    anchor: Option<Position>,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    run: Option<history::Run>,
    clock: history::Clock,
    pending_style: Option<style::Pending>,
    placeholder: Option<attach::Placeholder>,
    saved: Option<Arc<StructuredDocument>>,
    layout_error: Option<String>,
    rejected_input: Option<String>,
    overflows: Vec<Target>,
    visible: ViewRange,
    raster: RasterSpec,
    pointer: Option<PointerState>,
    goal: Option<vertical::Goal>,
    follow: FollowState,
    pointer_drag: bool,
    #[cfg(test)]
    fail_next_input: bool,
}

impl std::fmt::Debug for Checkpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Epoch is an ephemeral safety identity, deliberately renewed by authority restore.
        f.debug_struct("Checkpoint")
            .field("snapshot", &self.snapshot)
            .field("actions", &self.authority.actions())
            .field("cursor", &self.cursor)
            .field("anchor", &self.anchor)
            .field("undo", &self.undo)
            .field("redo", &self.redo)
            .field("run", &self.run)
            .field("pending_style", &self.pending_style)
            .field("placeholder", &self.placeholder)
            .field("saved", &self.saved)
            .field("layout_error", &self.layout_error)
            .field("rejected_input", &self.rejected_input)
            .field("overflows", &self.overflows)
            .field("goal", &self.goal)
            .field("follow", &self.follow)
            .field("pointer_drag", &self.pointer_drag)
            .finish()
    }
}

impl EditorCore {
    pub(in crate::typst) fn checkpoint(&self) -> Result<Checkpoint, CommandError> {
        Ok(Checkpoint {
            authority: self.session.rollback_checkpoint(),
            snapshot: self.snapshot.clone(),
            cursor: self.cursor,
            anchor: self.anchor,
            undo: self.undo.clone(),
            redo: self.redo.clone(),
            run: self.run,
            clock: self.clock.clone(),
            pending_style: self.pending_style,
            placeholder: self.placeholder.clone(),
            saved: self.saved.clone(),
            layout_error: self.layout_error.clone(),
            rejected_input: self.rejected_input.clone(),
            overflows: self.overflows.clone(),
            visible: self.visible,
            raster: self.raster,
            pointer: self.pointer,
            goal: self.goal,
            follow: self.follow,
            pointer_drag: self.pointer_drag,
            #[cfg(test)]
            fail_next_input: self.fail_next_input,
        })
    }

    pub(in crate::typst) fn rollback(&mut self, before: Checkpoint) {
        // Rejections before a mutation preserve the existing layout lifetime. If a
        // partially executed command changed authority, restore renews its epoch so
        // no derived pre-rollback Scene can regain pointer authority.
        let changed = self.session.shared_snapshot() != before.authority.shared_snapshot()
            || self.session.actions().iter().map(|a| a.request).ne(before
                .authority
                .actions()
                .iter()
                .map(|a| a.request));
        if changed {
            self.session = ReadAuthority::new(before.authority);
        }
        self.snapshot = before.snapshot;
        self.cursor = before.cursor;
        self.anchor = before.anchor;
        self.undo = before.undo;
        self.redo = before.redo;
        self.run = before.run;
        self.clock = before.clock;
        self.pending_style = before.pending_style;
        self.placeholder = before.placeholder;
        self.saved = before.saved;
        self.layout_error = before.layout_error;
        self.rejected_input = before.rejected_input;
        self.overflows = before.overflows;
        self.visible = before.visible;
        self.raster = before.raster;
        self.pointer = before.pointer;
        self.goal = before.goal;
        self.follow = before.follow;
        self.pointer_drag = before.pointer_drag;
        #[cfg(test)]
        {
            self.fail_next_input = before.fail_next_input;
        }
    }

    #[cfg(test)]
    pub(in crate::typst) fn inject_failure(
        &mut self,
        id: CommandId,
        state: &mut EditState,
    ) -> Result<bool, CommandError> {
        self.insert("mutated".into(), Origin::Typed, state);
        self.cursor.byte = 0;
        self.anchor = Some(self.cursor);
        self.undo.clear();
        self.redo.clear();
        self.run = None;
        self.clock = Arc::new(std::time::Instant::now);
        self.pending_style = None;
        self.placeholder = None;
        self.rejected_input = Some("discard this".into());
        self.layout_error = Some("discard this".into());
        self.overflows.clear();
        self.saved = None;
        if matches!(id, CommandId::PanicAfterMutation) {
            panic!("injected panic after mutation");
        }
        Err(CommandError::Injected)
    }
}
