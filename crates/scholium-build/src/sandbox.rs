use crate::{BuildError, SandboxProcess, cache};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::process::Command;

const MIB: u64 = 1024 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const TOOLS: &[&str] = &["prlimit", "lualatex", "luahbtex", "kpsewhich", "true"];
const RUNTIME: &[&str] = &[
    "/usr/lib/locale",
    "/usr/share/fonts",
    "/usr/share/fontconfig",
    "/etc/fonts",
    "/usr/share/texmf-dist",
    "/usr/share/texmf",
    "/usr/share/texlive/texmf-dist",
    "/etc/texmf",
    "/var/lib/texmf",
    "/var/cache/fontconfig",
];

/// Declarative toolchain policy. Only rtex is implemented; other kernels can add profiles later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildProfile {
    /// LuaLaTeX live preview, with a fixed runtime whitelist and no network.
    Rtex,
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

/// A prepared Linux rtex runtime. Construction probes both namespace and cgroup enforcement.
#[derive(Debug)]
pub struct Sandbox {
    pub(crate) mounts: BTreeMap<PathBuf, PathBuf>,
    toolchain: RtexToolchain,
    limits: ResourceLimits,
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
            toolchain,
            limits,
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

    /// The declarative policy applied to this runtime.
    pub fn profile(&self) -> BuildProfile {
        BuildProfile::Rtex
    }

    /// Run a bounded setup/capability command in the same sandbox policy as a session.
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
        self.launch(project, work, argv)?.output(deadline).await
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
        let persistent = self.toolchain.font_cache.canonicalize()?;
        if overlaps(&project, &work)
            || overlaps(&project, &persistent)
            || overlaps(&work, &persistent)
        {
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

    pub(crate) fn cache_path(&self) -> &Path {
        &self.toolchain.font_cache
    }

    pub(crate) fn spawn(
        &self,
        project: &Path,
        work: &Path,
        cache: tempfile::TempDir,
        argv: &[OsString],
    ) -> Result<SandboxProcess, BuildError> {
        let scope = format!(
            "scholium-rtex-{}-{}.scope",
            std::process::id(),
            cache
                .path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );
        let mut command = Command::new("/usr/bin/systemd-run");
        command.args([
            "--user",
            "--scope",
            "--quiet",
            "--expand-environment=no",
            "--unit",
            &scope,
        ]);
        command.args([
            "-p",
            &format!("MemoryMax={}", self.limits.memory_bytes),
            "-p",
            "MemorySwapMax=0",
            "-p",
            &format!("TasksMax={}", self.limits.tasks),
            "-p",
            "OOMPolicy=kill",
            "--",
            "/usr/bin/bwrap",
        ]);
        command.args(self.arguments(project, work, cache.path(), argv));
        SandboxProcess::spawn(command, scope, cache)
    }

    fn arguments(
        &self,
        project: &Path,
        work: &Path,
        cache: &Path,
        argv: &[OsString],
    ) -> Vec<OsString> {
        let mut args: Vec<OsString> = [
            "--unshare-all",
            "--die-with-parent",
            "--new-session",
            "--cap-drop",
            "ALL",
            "--clearenv",
        ]
        .map(Into::into)
        .to_vec();
        for (key, value) in [
            ("PATH", "/usr/bin"),
            ("HOME", "/tmp"),
            ("LANG", "C.UTF-8"),
            ("RTEX_TEXLIVE_BIN", "/usr/bin"),
            ("RTEX_TEXDIR", "/rtex/tex"),
            ("TEXMFVAR", "/rtex/texmf-var"),
            ("TEXMFCONFIG", "/tmp/texmf-config"),
            ("TEXMFHOME", "/tmp/texmf-home"),
            ("TEXINPUTS", ".:/project:"),
            ("TEXPICTS", "/project:"),
        ] {
            args.extend(["--setenv".into(), key.into(), value.into()]);
        }
        for dir in ["/usr", "/usr/bin", "/usr/lib", "/usr/share"] {
            args.extend(["--dir".into(), dir.into()]);
        }
        for (source, target) in [
            ("usr/bin", "/bin"),
            ("usr/lib", "/lib"),
            ("usr/lib", "/lib64"),
            ("lib", "/usr/lib64"),
        ] {
            args.extend(["--symlink".into(), source.into(), target.into()]);
        }
        for (target, source) in &self.mounts {
            args.extend(["--ro-bind".into(), source.into(), target.into()]);
        }
        args.extend(
            [
                "--proc",
                "/proc",
                "--dev",
                "/dev",
                "--size",
                "268435456",
                "--tmpfs",
                "/tmp",
            ]
            .map(Into::into),
        );
        for (mode, source, target) in [
            ("--ro-bind", project, "/project"),
            ("--bind", work, "/work"),
            ("--bind", cache, "/rtex/texmf-var"),
        ] {
            args.extend([mode.into(), source.into(), target.into()]);
        }
        args.extend(
            [
                "--chdir",
                "/work",
                "/usr/bin/prlimit",
                "--fsize=67108864",
                "--nofile=128",
                "--",
            ]
            .map(Into::into),
        );
        args.extend_from_slice(argv);
        args
    }

    async fn runtime(&mut self) -> Result<(), BuildError> {
        self.mounts.insert(
            "/rtex/bin/rtex".into(),
            self.toolchain.binary.canonicalize()?,
        );
        self.mounts.insert(
            "/rtex/tex".into(),
            self.toolchain.tex_directory.canonicalize()?,
        );
        if !self.toolchain.tex_directory.join("rtex-dl.lua").is_file() {
            return Err(BuildError::Configuration(
                "matching rtex support tree required".into(),
            ));
        }
        let mut binaries = vec![self.toolchain.binary.clone()];
        for tool in TOOLS {
            let path = PathBuf::from(format!("/usr/bin/{tool}"));
            self.mounts.insert(path.clone(), path.canonicalize()?);
            binaries.push(path);
        }
        for binary in binaries {
            self.libraries(&binary).await?;
        }
        for path in RUNTIME {
            if Path::new(path).exists() {
                self.mounts
                    .insert((*path).into(), Path::new(path).canonicalize()?);
            }
        }
        Ok(())
    }

    pub(crate) async fn libraries(&mut self, executable: &Path) -> Result<(), BuildError> {
        let mut command = Command::new("/usr/bin/ldd");
        command.arg(executable).kill_on_drop(true);
        let output = tokio::time::timeout(PROBE_TIMEOUT, command.output())
            .await
            .map_err(|_| BuildError::Timeout("runtime libraries"))??;
        if !output.status.success() {
            return Err(BuildError::Unavailable(format!(
                "ldd failed for {}",
                executable.display()
            )));
        }
        for word in String::from_utf8_lossy(&output.stdout).split_whitespace() {
            if word.starts_with('/') {
                let path = PathBuf::from(word);
                self.mounts.insert(path.clone(), path.canonicalize()?);
            }
        }
        Ok(())
    }
}

fn overlaps(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}
