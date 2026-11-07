//! Typst policy preparation; execution uses the shared Sandbox launch and process owner.
use crate::{BuildError, BuildProfile, ResourceLimits, Sandbox};
use std::{collections::BTreeMap, path::Path, time::Duration};

pub(crate) const WALL_TIME: Duration = Duration::from_secs(10);

impl Sandbox {
    /// Prepare the fixed external Typst preview policy, without TeX or host fonts.
    /// The executable and request are selected by trusted host code, never by a document.
    ///
    /// # Errors
    /// Rejects non-regular inputs, unsupported hosts, unavailable runtime libraries,
    /// namespace isolation or cgroup enforcement. There is no host fallback.
    pub async fn prepare_typst_preview(
        executable: &Path,
        request: &Path,
    ) -> Result<Self, BuildError> {
        if !cfg!(target_os = "linux") {
            return Err(BuildError::Unavailable(
                "Typst preview requires Linux isolation".into(),
            ));
        }
        let mut sandbox = Self::typst_preview(executable, request)?;
        sandbox.runtime().await?;
        let project = tempfile::tempdir()?;
        let work = tempfile::tempdir()?;
        // Probe the actual profile without running a document or mounting another tool.
        sandbox
            .run(
                project.path(),
                work.path(),
                &["/usr/bin/prlimit".into(), "--version".into()],
                WALL_TIME,
            )
            .await
            .map_err(|error| BuildError::Unavailable(error.to_string()))?;
        Ok(sandbox)
    }

    pub(crate) fn typst_preview(executable: &Path, request: &Path) -> Result<Self, BuildError> {
        let executable = executable.canonicalize()?;
        let request = request.canonicalize()?;
        if !executable.is_file() || !request.is_file() {
            return Err(BuildError::Configuration(
                "Typst worker and request must be regular files".into(),
            ));
        }
        Ok(Self {
            mounts: BTreeMap::from([
                ("/worker".into(), executable),
                ("/request.json".into(), request),
            ]),
            toolchain: None,
            limits: ResourceLimits {
                memory_bytes: 4 * 1024 * 1024 * 1024,
                tasks: 64,
            },
            profile: BuildProfile::TypstPreview,
        })
    }
}
