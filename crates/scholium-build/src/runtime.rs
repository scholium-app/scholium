//! Trusted runtime whitelist and shared dynamic library discovery.
use crate::{BuildError, BuildProfile, Sandbox};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::process::Command;
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

impl Sandbox {
    pub(crate) async fn runtime(&mut self) -> Result<(), BuildError> {
        if self.profile.is_worker() {
            let binary = self
                .mounts
                .get(Path::new("/worker"))
                .ok_or_else(|| BuildError::Configuration("missing trusted worker".into()))?
                .clone();
            let prlimit = PathBuf::from("/usr/bin/prlimit");
            self.mounts.insert(prlimit.clone(), prlimit.canonicalize()?);
            self.libraries(&binary).await?;
            return self.libraries(&prlimit).await;
        }
        let toolchain = self
            .toolchain
            .as_ref()
            .ok_or_else(|| BuildError::Configuration("missing TeX runtime".into()))?
            .clone();
        let mut binaries = Vec::new();
        if self.profile == BuildProfile::Rtex {
            self.mounts
                .insert("/rtex/bin/rtex".into(), toolchain.binary.canonicalize()?);
            self.mounts
                .insert("/rtex/tex".into(), toolchain.tex_directory.canonicalize()?);
            if !toolchain.tex_directory.join("rtex-dl.lua").is_file() {
                return Err(BuildError::Configuration(
                    "matching rtex support tree required".into(),
                ));
            }
            binaries.push(toolchain.binary.clone());
        }
        let mut tools = TOOLS.to_vec();
        if self.profile == BuildProfile::FullCompilation {
            tools.extend(["pdflatex", "pdftex", "xelatex", "xetex", "xdvipdfmx"]);
        }
        for tool in tools {
            let path = PathBuf::from(format!("/usr/bin/{tool}"));
            self.mounts.insert(path.clone(), path.canonicalize()?);
            binaries.push(path);
        }
        for binary in binaries {
            self.libraries(&binary).await?;
        }
        if self.profile == BuildProfile::FullCompilation {
            for path in ["/etc/paperspecs", "/etc/papersize"] {
                if Path::new(path).exists() {
                    self.mounts
                        .insert(path.into(), Path::new(path).canonicalize()?);
                }
            }
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
