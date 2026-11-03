//! Source-authoritative LaTeX sessions and supervised, sandboxed live preview.
mod event;
mod live;
mod protocol;
mod session;
mod supervisor;
mod transport;

pub use event::{CompileOutcome, EngineState, LiveEvent};
pub use live::{LiveConfig, LiveSession};
pub use session::{Engine, PreviewMode, SourceSession};

/// Failure to open, edit, launch or communicate with a LaTeX session.
#[derive(Debug, thiserror::Error)]
pub enum LatexError {
    /// Source IO failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// The OS sandbox could not be enforced.
    #[error(transparent)]
    Build(#[from] scholium_build::BuildError),
    /// A resource is unknown, non-normal, or outside the project.
    #[error("invalid project-relative path: {0}")]
    Path(String),
    /// An edit range is invalid or not on UTF-8 character boundaries.
    #[error("invalid UTF-8 byte range")]
    Offset,
    /// The selected engine is unsupported by the requested route.
    #[error("unsupported engine: {0}")]
    Engine(String),
    /// The revision counter cannot advance.
    #[error("source revision exhausted")]
    Revision,
    /// The supervisor stopped or the consumer fell behind its bounded queue.
    #[error("live supervisor stopped")]
    Stopped,
    /// A host protocol write or startup check exceeded its deadline.
    #[error("live operation timed out")]
    Timeout,
    /// Invalid supervision budgets.
    #[error("invalid live configuration")]
    Configuration,
}

#[cfg(test)]
mod tests;
