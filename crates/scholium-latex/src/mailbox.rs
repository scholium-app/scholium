//! A bounded preview mailbox: replace superseded render events, retain lifecycle transitions.
use crate::{LatexError, LiveEvent};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
};
use tokio::{
    sync::Notify,
    time::{Duration, Instant},
};

const EVENT_CAPACITY: usize = 32;

#[derive(Debug, Default)]
struct State {
    events: VecDeque<LiveEvent>,
    layout_required: Option<u64>,
    lost_diagnostics: u64,
    lost_locations: u64,
    sender_closed: bool,
    receiver_closed: bool,
}
#[derive(Debug, Default)]
struct Shared {
    state: Mutex<State>,
    readable: Notify,
    writable: Notify,
}
impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[derive(Debug)]
pub(crate) struct EventSender(Arc<Shared>);
#[derive(Debug)]
pub(crate) struct EventReceiver(Arc<Shared>);

pub(crate) fn channel() -> (EventSender, EventReceiver) {
    let shared = Arc::new(Shared::default());
    (EventSender(shared.clone()), EventReceiver(shared))
}
impl EventSender {
    pub(crate) async fn send(&self, event: LiveEvent) -> Result<Duration, LatexError> {
        let mut blocked = Duration::ZERO;
        loop {
            // Register before checking state, so a concurrent receive/drop cannot lose a wakeup.
            let notified = self.0.writable.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.enqueue(&event)? {
                return Ok(blocked);
            }
            let started = Instant::now();
            notified.await;
            blocked += started.elapsed();
        }
    }

    fn enqueue(&self, event: &LiveEvent) -> Result<bool, LatexError> {
        let mut state = self.0.lock();
        if state.receiver_closed {
            return Err(LatexError::Stopped);
        }
        // Moving a replacement to the back preserves ordering relative to intervening lifecycle events.
        if let Some(index) = state.events.iter().position(|old| supersedes(event, old)) {
            state.events.remove(index);
        }
        if state.events.len() == EVENT_CAPACITY {
            if let Some(index) = state
                .events
                .iter()
                .position(|old| !matches!(old, LiveEvent::Engine { .. }))
            {
                if let Some(dropped) = state.events.remove(index) {
                    mark_missing(&mut state, &dropped);
                }
            } else if matches!(event, LiveEvent::Engine { .. }) {
                // Lifecycle transitions/reasons are lossless. Only this rare all-control case applies
                // backpressure; the actor discounts the blocked time from its heartbeat deadline.
                return Ok(false);
            } else {
                mark_missing(&mut state, event);
                return Ok(true);
            }
        }
        state.events.push_back(event.clone());
        drop(state);
        self.0.readable.notify_one();
        Ok(true)
    }
}

fn supersedes(new: &LiveEvent, old: &LiveEvent) -> bool {
    match (new, old) {
        (LiveEvent::Pages { generation: a, .. }, LiveEvent::Pages { generation: b, .. }) => a == b,
        (
            LiveEvent::Paragraph {
                generation: a,
                paragraph: x,
                ..
            },
            LiveEvent::Paragraph {
                generation: b,
                paragraph: y,
                ..
            },
        ) => a == b && x == y,
        _ => false,
    }
}
fn mark_missing(state: &mut State, event: &LiveEvent) {
    match event {
        LiveEvent::Diagnostic { .. } => {
            state.lost_diagnostics = state.lost_diagnostics.saturating_add(1)
        }
        LiveEvent::Location { .. } => state.lost_locations = state.lost_locations.saturating_add(1),
        _ => {}
    }
    if let LiveEvent::Pages { generation, .. } | LiveEvent::Paragraph { generation, .. } = event {
        state.layout_required = Some(state.layout_required.unwrap_or(0).max(*generation));
    }
}

impl EventReceiver {
    pub(crate) async fn recv(&mut self) -> Option<LiveEvent> {
        loop {
            let notified = self.0.readable.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            {
                let mut state = self.0.lock();
                if let Some(event) = state.events.pop_front() {
                    self.0.writable.notify_one();
                    return Some(event);
                }
                if let Some(generation) = state.layout_required.take() {
                    return Some(LiveEvent::LayoutRequired { generation });
                }
                if state.lost_diagnostics > 0 || state.lost_locations > 0 {
                    let message = format!(
                        "event overflow: {} diagnostics and {} locations omitted; request diagnostics/location again",
                        state.lost_diagnostics, state.lost_locations
                    );
                    state.lost_diagnostics = 0;
                    state.lost_locations = 0;
                    return Some(LiveEvent::Diagnostic { message });
                }
                if state.sender_closed {
                    return None;
                }
            }
            notified.await;
        }
    }
    #[cfg(test)]
    pub(crate) fn pending(&self) -> usize {
        let state = self.0.lock();
        state.events.len()
            + usize::from(state.layout_required.is_some())
            + usize::from(state.lost_diagnostics > 0 || state.lost_locations > 0)
    }
}
impl Drop for EventSender {
    fn drop(&mut self) {
        self.0.lock().sender_closed = true;
        self.0.readable.notify_one();
    }
}
impl Drop for EventReceiver {
    fn drop(&mut self) {
        self.0.lock().receiver_closed = true;
        self.0.writable.notify_one();
    }
}
