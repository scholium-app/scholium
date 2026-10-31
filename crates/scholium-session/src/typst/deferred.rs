//! Ordered abstract input admission while the wanted Scene is pending.
use super::*;

/// Maximum waiting input count; composition and Escape cancellation bypass the cap.
pub const DEFERRED_LIMIT: usize = 256;

/// Abstract event category. Platform composition payloads remain host-owned.
#[derive(Debug, Clone)]
pub enum DeferredEvent {
    /// Committed text, retained as a copyable draft if the queue is full.
    Text(String),
    /// Pressed key and its modifiers.
    Key(Key, Modifiers),
    /// Composition lifecycle event (including commit), never dropped at the cap.
    Composition,
    /// Clipboard copy/cut, or an otherwise toolkit-specific pressed key.
    Action,
    /// Pointer press while queued semantic input would change its picture.
    PointerPress,
    /// Pointer release or departure, which cancels dragging.
    PointerRelease,
    /// Events with no editing meaning while waiting.
    Ignore,
}

impl DeferredEvent {
    fn needs_scene(&self) -> bool {
        matches!(self, Self::Key(Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End, mods) if !mods.command)
    }
    fn always(&self) -> bool {
        matches!(self, Self::Composition | Self::Key(Key::Escape, _))
    }
}

/// Host action after abstract admission; no toolkit or presentation data crosses here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// Execute now through the single editing input path.
    Execute,
    /// Store the matching toolkit event until the core releases its abstract record.
    Queue,
    /// Pointer press refers to a picture that queued input will change.
    PointerRejected,
    /// Queue is full; text is retained in `rejected_input`.
    Full,
    /// Cancel the platform drag.
    CancelDrag,
    /// No event needs to be retained.
    Ignore,
}

/// Next action for the host's matching event queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredStep {
    /// No runnable event, or visual-line navigation still awaits layout.
    Wait,
    /// Execute and remove the matching host event.
    Execute,
    /// Layout failed; remove only the visual-line key and report its rejection.
    LayoutFailed,
}

impl EditorCore {
    /// Decide whether an arriving abstract event runs or queues, preserving arrival order.
    pub fn admit(&mut self, event: DeferredEvent) -> Admission {
        if self.deferred.is_empty() && (!event.needs_scene() || self.scene_is_current()) {
            return Admission::Execute;
        }
        match event {
            DeferredEvent::PointerPress => Admission::PointerRejected,
            DeferredEvent::PointerRelease => Admission::CancelDrag,
            DeferredEvent::Ignore => Admission::Ignore,
            event if self.deferred.len() >= DEFERRED_LIMIT && !event.always() => {
                if let DeferredEvent::Text(text) = event {
                    self.rejected_input = Some(text);
                }
                Admission::Full
            }
            event => {
                self.deferred.push_back(event);
                Admission::Queue
            }
        }
    }

    /// Release one queued record only when its needed geometry is current.
    pub fn next_deferred(&mut self) -> DeferredStep {
        let Some(event) = self.deferred.front() else {
            return DeferredStep::Wait;
        };
        if event.needs_scene() && !self.scene_is_current() {
            if self.layout_error.is_none() {
                return DeferredStep::Wait;
            }
            self.deferred.pop_front();
            return DeferredStep::LayoutFailed;
        }
        self.deferred.pop_front();
        DeferredStep::Execute
    }

    /// Count of accepted abstract events awaiting Scene-dependent execution.
    pub fn deferred_len(&self) -> usize {
        self.deferred.len()
    }
    /// Discard queued events after the user confirms a new document.
    pub fn clear_deferred(&mut self) {
        self.deferred.clear();
    }
}
