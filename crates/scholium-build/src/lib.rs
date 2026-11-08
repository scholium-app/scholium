//! Shell-free, fail-closed Linux toolchain sandbox.
mod cache;
mod launch;
mod process;
mod quota;
mod runtime;
mod sandbox;
mod typst_preview;

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
    /// The kernel-enforced session output budget was exhausted.
    #[error("build output byte or inode limit exceeded")]
    OutputQuota,
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

/// Whether the trusted, atomically published font cache has the current warm marker.
/// This performs no compilation and never creates or changes a cache.
pub fn font_cache_is_warm(root: &std::path::Path) -> bool {
    root.join("ready").join(cache::CACHE_VERSION).is_file()
}
