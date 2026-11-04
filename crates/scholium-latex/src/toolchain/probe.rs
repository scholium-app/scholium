use super::{
    BundleStatus, DetectionConfig, Issue, ProgramStatus, TexStatus, ToolchainStatus, bundle,
};
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};

const DETECTION_TIMEOUT: Duration = Duration::from_secs(6);
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const OUTPUT_LIMIT: u64 = 16 * 1024;

pub(super) async fn detect(config: &DetectionConfig) -> ToolchainStatus {
    tokio::time::timeout(DETECTION_TIMEOUT, detect_inner(config))
        .await
        .unwrap_or_else(|_| failed())
}

async fn detect_inner(config: &DetectionConfig) -> ToolchainStatus {
    let (rtex, issue) = bundle::inspect_bounded(config).await;
    let cache_path = config.font_cache.clone();
    let warm = tokio::task::spawn_blocking(move || scholium_build::font_cache_is_warm(&cache_path));
    let (kpse, lua, xe, pdf, biber, bibtex, warm) = tokio::join!(
        program(config, "kpsewhich"),
        program(config, "lualatex"),
        program(config, "xelatex"),
        program(config, "pdflatex"),
        program(config, "biber"),
        program(config, "bibtex"),
        warm
    );
    let mut issues: Vec<_> = issue.into_iter().collect();
    let engines = vec![lua, xe, pdf, biber, bibtex];
    // Standalone kpsewhich on TeX Live reports only kpathsea, not the distribution year.
    let distribution = engines
        .iter()
        .filter_map(|p| p.version.as_ref())
        .find(|v| v.contains("TeX Live") || v.contains("MiKTeX") || v.contains("MacTeX"))
        .cloned();
    let tex_live = TexStatus {
        found: kpse.version.as_ref().is_some_and(|v| {
            v.contains("kpathsea")
                || v.contains("MiKTeX")
                || v.contains("TeX Live")
                || v.contains("MacTeX")
        }),
        version: distribution.or(kpse.version),
        kpsewhich: kpse.path,
    };
    if !tex_live.found {
        issues.push(Issue::new("toolchain.tex-missing", "toolchain.install-tex"));
    }
    for engine in &engines {
        if engine.version.is_none() {
            issues.push(
                Issue::new(
                    "toolchain.program-unavailable",
                    "toolchain.install-tex-component",
                )
                .argument("program", engine.name),
            );
        }
    }
    let font_cache_warm = warm.unwrap_or(false);
    if !font_cache_warm {
        issues.push(Issue::new(
            "toolchain.font-cache-cold",
            "toolchain.warm-on-preview",
        ));
    }
    let development_override = config.binary_override.is_some() || config.tex_override.is_some();
    if development_override {
        issues.push(Issue::new(
            "toolchain.dev-override",
            "toolchain.remove-dev-override",
        ));
        let config = config.clone();
        match tokio::task::spawn_blocking(move || bundle::launch_config(&config)).await {
            Ok(Err(issue)) => issues.push(issue),
            Err(_) => issues.push(Issue::new(
                "toolchain.detection-failed",
                "toolchain.refresh",
            )),
            _ => {}
        }
    }
    ToolchainStatus {
        rtex,
        development_override,
        tex_live,
        engines,
        font_cache_warm,
        issues,
    }
}

fn find(config: &DetectionConfig, name: &str) -> Option<PathBuf> {
    let search = config.search_path.as_ref()?;
    std::env::split_paths(search)
        .filter(|p| p.is_absolute())
        .map(|p| p.join(name))
        .find(|p| {
            use std::os::unix::fs::PermissionsExt;
            p.metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}
async fn program(config: &DetectionConfig, name: &'static str) -> ProgramStatus {
    let config = config.clone();
    let path = tokio::task::spawn_blocking(move || find(&config, name))
        .await
        .ok()
        .flatten();
    let version = match &path {
        Some(path) => version(path).await,
        None => None,
    };
    ProgramStatus {
        name,
        path,
        version,
    }
}
async fn version(path: &PathBuf) -> Option<String> {
    let mut child = Command::new(path)
        .arg("--version")
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let stderr = child.stderr.take()?;
    let operation = async {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let mut stdout = stdout.take(OUTPUT_LIMIT + 1);
        let mut stderr = stderr.take(OUTPUT_LIMIT + 1);
        let (a, b, status) = tokio::join!(
            stdout.read_to_end(&mut out),
            stderr.read_to_end(&mut err),
            child.wait()
        );
        a.ok()?;
        b.ok()?;
        if !status.ok()?.success()
            || out.len() as u64 > OUTPUT_LIMIT
            || err.len() as u64 > OUTPUT_LIMIT
        {
            return None;
        }
        out.extend(err);
        let text = String::from_utf8_lossy(&out);
        // Prefer a distribution-bearing line when the program supplies one.
        let line = text
            .lines()
            .find(|l| l.contains("TeX Live") || l.contains("MiKTeX") || l.contains("MacTeX"))
            .or_else(|| text.lines().next())?;
        Some(line.chars().take(512).collect())
    };
    tokio::time::timeout(PROBE_TIMEOUT, operation)
        .await
        .ok()
        .flatten()
}
pub(super) fn failed() -> ToolchainStatus {
    ToolchainStatus {
        rtex: BundleStatus::default(),
        development_override: false,
        tex_live: TexStatus {
            found: false,
            version: None,
            kpsewhich: None,
        },
        engines: Vec::new(),
        font_cache_warm: false,
        issues: vec![Issue::new(
            "toolchain.detection-failed",
            "toolchain.refresh",
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt, time::Instant};

    fn script(root: &std::path::Path, name: &str, body: &str) {
        let path = root.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("probe script");
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("executable");
    }
    #[tokio::test]
    async fn hanging_and_excess_output_probes_are_bounded_and_versions_are_reported() {
        let root = tempfile::tempdir().expect("fixture");
        script(
            root.path(),
            "kpsewhich",
            "printf 'kpathsea version 6.4\nkpathsea version 6.4 (TeX Live fixture)\n'",
        );
        script(root.path(), "lualatex", "exec /usr/bin/sleep 30");
        script(root.path(), "xelatex", "exec /usr/bin/yes oversized");
        script(root.path(), "bibtex", "echo 'BibTeX fixture'");
        let config = DetectionConfig::for_executable(
            &root.path().join("app"),
            Some(root.path().as_os_str().to_owned()),
            root.path().join("cache"),
        );
        let started = Instant::now();
        let status = detect(&config).await;
        assert!(started.elapsed() < Duration::from_secs(4));
        assert!(status.tex_live.found);
        assert!(
            status
                .tex_live
                .version
                .expect("distribution")
                .contains("TeX Live fixture")
        );
        assert!(status.engines[0].path.is_some());
        assert!(status.engines[0].version.is_none());
        assert!(status.engines[1].version.is_none());
        assert_eq!(status.engines[4].version.as_deref(), Some("BibTeX fixture"));
    }
    #[tokio::test]
    async fn kpathsea_only_version_finds_tex_live_and_uses_engine_distribution_version() {
        let root = tempfile::tempdir().expect("fixture");
        script(root.path(), "kpsewhich", "echo 'kpathsea version 6.4.2'");
        script(
            root.path(),
            "lualatex",
            "echo 'LuaHBTeX (TeX Live fixture year)'",
        );
        let config = DetectionConfig::for_executable(
            &root.path().join("app"),
            Some(root.path().as_os_str().to_owned()),
            root.path().join("cache"),
        );
        let status = detect(&config).await;
        assert!(status.tex_live.found);
        assert_eq!(
            status.tex_live.version.as_deref(),
            Some("LuaHBTeX (TeX Live fixture year)")
        );
    }
}
