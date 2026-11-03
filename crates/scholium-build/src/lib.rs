//! Shell-free, fail-closed Linux toolchain sandbox.
mod cache;
mod process;
mod sandbox;

pub use process::SandboxProcess;
pub use sandbox::{BuildProfile, ResourceLimits, RtexToolchain, Sandbox};

#[cfg(test)]
mod tests;

/// Sandbox preparation or execution failure. Isolation failures never fall back to the host.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// A filesystem or process operation failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Trusted configuration violates the sandbox boundary.
    #[error("invalid sandbox configuration: {0}")]
    Configuration(String),
    /// The host cannot enforce the requested isolation or cgroup policy.
    #[error("sandbox unavailable: {0}")]
    Unavailable(String),
    /// A bounded host operation exceeded its deadline.
    #[error("operation timed out: {0}")]
    Timeout(&'static str),
    /// A trusted setup command failed or exceeded its output budget.
    #[error("tool failed: {0}")]
    Tool(String),
}
