//! Relocatable rtex discovery and cached, bounded detection for UI consumers.
mod bundle;
mod probe;

use scholium_build::RtexToolchain;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
};

/// A localization key with named substitution arguments; never rendered by this module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Stable key owned by the application's localization catalog.
    pub id: &'static str,
    /// Untranslated values such as tool names or paths.
    pub arguments: BTreeMap<String, String>,
}
/// An actionable toolchain problem, shared by all UI surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Description to localize.
    pub message: Message,
    /// Suggested remediation to localize.
    pub action: Message,
}
impl Issue {
    fn new(id: &'static str, action: &'static str) -> Self {
        Self {
            message: Message {
                id,
                arguments: BTreeMap::new(),
            },
            action: Message {
                id: action,
                arguments: BTreeMap::new(),
            },
        }
    }
    fn argument(mut self, name: &str, value: impl Into<String>) -> Self {
        self.message.arguments.insert(name.into(), value.into());
        self
    }
}
/// Bundle identity and integrity, independent of any development override.
#[derive(Debug, Clone, Default)]
pub struct BundleStatus {
    /// A bundle directory was found (even if incomplete).
    pub present: bool,
    /// All manifest, PIN and file-integrity checks passed.
    pub valid: bool,
    /// Selected bundle directory.
    pub directory: Option<PathBuf>,
    /// Manifest commit, if readable.
    pub commit: Option<String>,
    /// Manifest version, if readable.
    pub version: Option<String>,
}
/// One externally installed program and its bounded version probe.
#[derive(Debug, Clone)]
pub struct ProgramStatus {
    /// Executable name, also usable as a message argument.
    pub name: &'static str,
    /// Located executable; present even if the version command fails.
    pub path: Option<PathBuf>,
    /// First version-output line, at most 512 characters.
    pub version: Option<String>,
}
/// User-installed TeX distribution; TeX Live, MacTeX and MiKTeX are recognized.
#[derive(Debug, Clone)]
pub struct TexStatus {
    /// A recognized distribution responded successfully through kpsewhich.
    pub found: bool,
    /// Distribution/version output (not a localized label).
    pub version: Option<String>,
    /// Located kpsewhich executable.
    pub kpsewhich: Option<PathBuf>,
}
/// Immutable detection result. A ready bundle does not imply sandbox availability.
#[derive(Debug, Clone)]
pub struct ToolchainStatus {
    /// Bundled rtex identity and integrity.
    pub rtex: BundleStatus,
    /// A test/development override is selected instead of the bundle.
    pub development_override: bool,
    /// User-installed distribution.
    pub tex_live: TexStatus,
    /// LuaLaTeX, XeLaTeX, pdfLaTeX, biber and BibTeX, in that order.
    pub engines: Vec<ProgramStatus>,
    /// The build module's trusted font-cache marker is present.
    pub font_cache_warm: bool,
    /// Descriptions and suggested actions, expressed as localization keys/arguments.
    pub issues: Vec<Issue>,
}
/// Trusted host inputs, captured once so tests never mutate the process environment.
#[derive(Debug, Clone)]
pub struct DetectionConfig {
    executable: PathBuf,
    search_path: Option<OsString>,
    font_cache: PathBuf,
    binary_override: Option<PathBuf>,
    tex_override: Option<PathBuf>,
}
impl DetectionConfig {
    /// Construct explicit inputs without reading environment variables.
    pub fn for_executable(
        executable: &Path,
        search_path: Option<OsString>,
        font_cache: PathBuf,
    ) -> Self {
        Self {
            executable: executable.into(),
            search_path,
            font_cache,
            binary_override: None,
            tex_override: None,
        }
    }
    /// Capture installed/development layout, PATH and optional test/development overrides.
    /// # Errors
    /// Fails if the running executable cannot be located.
    pub fn from_environment() -> std::io::Result<Self> {
        let mut config = Self::for_executable(
            &std::env::current_exe()?,
            std::env::var_os("PATH"),
            font_cache_directory(),
        );
        config.binary_override = std::env::var_os("SCHOLIUM_RTEX_BIN").map(Into::into);
        config.tex_override = std::env::var_os("SCHOLIUM_RTEX_TEXDIR").map(Into::into);
        Ok(config)
    }
}
/// Persistent trusted font-cache location; the environment override is for tests/development only.
pub fn font_cache_directory() -> PathBuf {
    std::env::var_os("SCHOLIUM_RTEX_FONT_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::var_os("HOME")
                        .map(PathBuf::from)
                        .unwrap_or_else(std::env::temp_dir)
                        .join(".cache")
                })
                .join("scholium/tex-fonts")
        })
}
/// Discover a validated toolchain off the calling/UI thread, without warming fonts or launching TeX.
/// # Errors
/// Returns a localized, actionable bundle/override failure instead of panicking.
pub async fn discover() -> Result<RtexToolchain, Issue> {
    let config = DetectionConfig::from_environment()
        .map_err(|_| Issue::new("toolchain.executable-unavailable", "toolchain.reinstall"))?;
    let task = tokio::task::spawn_blocking(move || bundle::launch_config(&config));
    tokio::time::timeout(bundle::INSPECTION_TIMEOUT, task)
        .await
        .map_err(|_| Issue::new("toolchain.bundle-check-timeout", "toolchain.refresh"))?
        .map_err(|_| Issue::new("toolchain.detection-failed", "toolchain.refresh"))?
}
/// Background detector with a cached snapshot and a coalescing refresh queue.
/// Dropping it closes the queue; an active bounded detection finishes before the worker exits.
#[derive(Debug)]
pub struct ToolchainDetector {
    cache: Arc<Mutex<Option<Arc<ToolchainStatus>>>>,
    requests: mpsc::SyncSender<()>,
}
impl ToolchainDetector {
    /// Start initial detection on a dedicated worker; construction never probes programs on the caller.
    pub fn new(config: DetectionConfig) -> Self {
        let cache = Arc::new(Mutex::new(None));
        let output = cache.clone();
        let (requests, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            while receiver.recv().is_ok() {
                let status = match &runtime {
                    Ok(runtime) => runtime.block_on(probe::detect(&config)),
                    Err(_) => probe::failed(),
                };
                if let Ok(mut slot) = output.lock() {
                    *slot = Some(Arc::new(status));
                }
            }
        });
        let detector = Self { cache, requests };
        detector.refresh();
        detector
    }
    /// Read the last completed snapshot; returns None while the first detection is pending.
    pub fn cached(&self) -> Option<Arc<ToolchainStatus>> {
        self.cache.lock().ok()?.clone()
    }
    /// Request fresh detection, coalescing repeated requests and retaining the old snapshot meanwhile.
    pub fn refresh(&self) {
        let _ = self.requests.try_send(());
    }
}
