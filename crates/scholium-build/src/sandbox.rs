use crate::{BuildError, SandboxProcess, cache};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    time::Duration,
};

const MIB: u64 = 1024 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
/// Declarative policies for resident live sessions and bounded one-shot compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildProfile {
    /// LuaLaTeX live preview, with a fixed runtime whitelist and no network.
    Rtex,
    /// Bounded one-shot LaTeX compilation; no rtex runtime is required.
    FullCompilation,
    /// Bounded external Typst source preview, without a TeX runtime or font cache.
    TypstPreview,
}

/// Session-wide cgroup limits. Resident processes have neither a wall-clock nor CPU-seconds limit.
#[derive(Debug, Clone, Copy)]
pub struct ResourceLimits {
    /// Maximum resident memory, in bytes; swap is disabled.
    pub memory_bytes: u64,
    /// Maximum number of processes/threads in the cgroup.
    pub tasks: u32,
}
impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            memory_bytes: 2 * 1024 * MIB,
            tasks: 64,
        }
    }
}

/// Paths selected by trusted host configuration, never by a document or project manifest.
#[derive(Debug, Clone)]
pub struct RtexToolchain {
    /// Externally built, pinned rtex executable.
    pub binary: PathBuf,
    /// rtex's matching TeX/Lua support tree.
    pub tex_directory: PathBuf,
    /// Private per-user cache directory, outside projects and session build directories.
    pub font_cache: PathBuf,
}

/// A prepared Linux toolchain runtime. Construction probes both namespace and cgroup enforcement.
#[derive(Debug)]
pub struct Sandbox {
    pub(crate) mounts: BTreeMap<PathBuf, PathBuf>,
    pub(crate) toolchain: Option<RtexToolchain>,
    pub(crate) limits: ResourceLimits,
    pub(crate) profile: BuildProfile,
}
impl Sandbox {
    /// Prepare the runtime, probe isolation, and warm a private cache with fixed trusted source.
    ///
    /// # Errors
    /// Fails explicitly if tools, namespaces, user cgroups, trusted cache setup or warm-up are unavailable.
    pub async fn prepare(
        toolchain: RtexToolchain,
        limits: ResourceLimits,
    ) -> Result<Self, BuildError> {
        if limits.memory_bytes == 0 || limits.tasks == 0 {
            return Err(BuildError::Configuration("zero resource budget".into()));
        }
        let mut sandbox = Self {
            mounts: BTreeMap::new(),
            toolchain: Some(toolchain),
            limits,
            profile: BuildProfile::Rtex,
        };
        sandbox.runtime().await?;
        let project = tempfile::tempdir()?;
        let work = tempfile::tempdir()?;
        let cache = tempfile::tempdir()?;
        let mut probe = sandbox.spawn(
            project.path(),
            work.path(),
            cache,
            &["/usr/bin/true".into()],
        )?;
        probe
            .output(PROBE_TIMEOUT)
            .await
            .map_err(|e| BuildError::Unavailable(e.to_string()))?;
        cache::warm(&sandbox).await?;
        Ok(sandbox)
    }

    /// Prepare the one-shot XeLaTeX/pdfLaTeX policy, without requiring rtex.
    /// # Errors
    /// Reports missing engines, isolation or trusted font-cache preparation failures.
    pub async fn prepare_full(
        font_cache: PathBuf,
        limits: ResourceLimits,
    ) -> Result<Self, BuildError> {
        if limits.memory_bytes == 0 || limits.tasks == 0 {
            return Err(BuildError::Configuration("zero resource budget".into()));
        }
        let mut sandbox = Self {
            mounts: BTreeMap::new(),
            toolchain: Some(RtexToolchain {
                binary: "/usr/bin/true".into(),
                tex_directory: "/usr/share/texmf-dist".into(),
                font_cache,
            }),
            limits,
            profile: BuildProfile::FullCompilation,
        };
        sandbox.runtime().await?;
        let project = tempfile::tempdir()?;
        let work = tempfile::tempdir()?;
        let cache = tempfile::tempdir()?;
        sandbox
            .spawn(
                project.path(),
                work.path(),
                cache,
                &["/usr/bin/true".into()],
            )?
            .output(PROBE_TIMEOUT)
            .await
            .map_err(|e| BuildError::Unavailable(e.to_string()))?;
        cache::warm(&sandbox).await?;
        Ok(sandbox)
    }

    /// The declarative policy applied to this runtime.
    pub fn profile(&self) -> BuildProfile {
        self.profile
    }

    /// Run a bounded command under this profile, with stdout/stderr capped at 1 MiB each.
    /// TypstPreview clamps the requested wall deadline to 10 seconds. Timeout,
    /// cancellation and output rejection drop the shared process-tree owner.
    ///
    /// # Errors
    /// Reports isolation, launch, output-budget, exit-status and deadline failures.
    pub async fn run(
        &self,
        project: &Path,
        work: &Path,
        argv: &[OsString],
        deadline: Duration,
    ) -> Result<Vec<u8>, BuildError> {
        let result = self
            .launch(project, work, argv)?
            .output(self.deadline(deadline))
            .await;
        match result {
            Err(BuildError::Timeout(_)) if self.profile == BuildProfile::TypstPreview => Err(
                BuildError::Timeout("Typst preview exceeded its 10 second budget"),
            ),
            result => result,
        }
    }

    /// Launch argv inside the fixed profile. The executable must use a sandbox path.
    ///
    /// # Errors
    /// Rejects overlapping roots, invalid cache contents, and process launch failures.
    /// All descendants stay in the same cgroup and PID/network namespaces.
    pub fn launch(
        &self,
        project: &Path,
        work: &Path,
        argv: &[OsString],
    ) -> Result<SandboxProcess, BuildError> {
        let project = project.canonicalize()?;
        let work = work.canonicalize()?;
        if overlaps(&project, &work) {
            return Err(BuildError::Configuration(
                "project and output must be separate".into(),
            ));
        }
        if self.profile == BuildProfile::TypstPreview {
            if self.mounts.values().any(|input| overlaps(input, &work)) {
                return Err(BuildError::Configuration(
                    "trusted inputs and output must be separate".into(),
                ));
            }
            return self.spawn(&project, &work, tempfile::tempdir()?, argv);
        }
        let persistent = self.cache_path()?.canonicalize()?;
        if overlaps(&project, &persistent) || overlaps(&work, &persistent) {
            return Err(BuildError::Configuration(
                "project, build and trusted cache must be separate".into(),
            ));
        }
        let session = tempfile::Builder::new()
            .prefix("scholium-font-session-")
            .tempdir()?;
        cache::copy(&persistent.join("ready"), session.path())?;
        self.spawn(&project, &work, session, argv)
    }

    pub(crate) fn cache_path(&self) -> Result<&Path, BuildError> {
        self.toolchain
            .as_ref()
            .map(|t| t.font_cache.as_path())
            .ok_or_else(|| BuildError::Configuration("profile has no TeX font cache".into()))
    }

    pub(crate) fn deadline(&self, requested: Duration) -> Duration {
        if self.profile == BuildProfile::TypstPreview {
            requested.min(crate::typst_preview::WALL_TIME)
        } else {
            requested
        }
    }
}

fn overlaps(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}
