use std::{ops::Range, time::Duration};

/// Host-facing compile outcome, independent of the tool's wire representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileOutcome {
    /// Completed without reported errors.
    Complete,
    /// Produced output with compilation errors.
    WithErrors,
    /// Failed or timed out in the upstream compiler.
    Failed,
}
/// Host-facing engine lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    /// Starting or synchronizing source.
    Starting,
    /// Ready to accept work.
    Ready,
    /// Recovering after a detected failure.
    Recovering,
    /// Failed; a responsive session is kept alive for source repair.
    Failed,
    /// Host cancelled the session.
    Stopped,
}
/// Live-preview events. No upstream protocol types or display lists cross this boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveEvent {
    /// An updated paragraph. Identity is scoped to `generation`.
    Paragraph {
        /// Host process generation, incremented after a whole-session restart.
        generation: u64,
        /// Host revision; `None` means the tool revision could not be safely matched.
        revision: Option<u64>,
        /// Paragraph identity within this process generation.
        paragraph: u64,
        /// Host monotonic time from forwarding the matching source edit to receiving this update.
        /// Unknown revision mappings also have unknown latency; excludes rasterization and UI.
        elapsed: Option<Duration>,
    },
    /// Whole-page layout metadata; rasterization is a later integration step.
    Pages {
        /// Host process generation.
        generation: u64,
        /// Host revision, or unknown/stale if no acknowledgement has established it.
        revision: Option<u64>,
        /// Number of pages in the layout.
        count: u64,
        /// Compilation outcome, including upstream pass timeouts.
        outcome: CompileOutcome,
        /// Host monotonic time since the latest layout request or background scheduling event.
        elapsed: Duration,
    },
    /// Some distinct preview updates were evicted while the consumer was behind. Request a layout
    /// to reconstruct a complete preview; source authority and the worker remain live.
    LayoutRequired {
        /// Latest process generation whose preview updates may be incomplete.
        generation: u64,
    },
    /// A source location supplied by a bounded spans query.
    Location {
        /// Normalized project-relative resource.
        path: String,
        /// UTF-8 byte range in that resource.
        bytes: Range<usize>,
    },
    /// Recoverable protocol, capability, engine or supervision diagnostic.
    Diagnostic {
        /// Human-readable reason; bounded by the session output budget.
        message: String,
    },
    /// Engine or host supervisor lifecycle, with an optional failure reason.
    Engine {
        /// Host process generation.
        generation: u64,
        /// Host-facing engine state.
        state: EngineState,
        /// Reason for failure, recovery, or degraded capability.
        reason: Option<String>,
    },
}
