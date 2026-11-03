//! Bounded one-shot preview on an immutable source/resource snapshot.
use crate::{
    Engine, LatexError, SourceSession,
    view::{RasterOrigin, RasterPage},
};
use scholium_build::{BuildProfile, Sandbox};
use std::{
    path::Path,
    time::{Duration, Instant},
};

/// A complete one-shot preview and its source identity.
#[derive(Debug)]
pub struct FullPreview {
    /// Monotonic host source revision compiled.
    pub revision: u64,
    /// Fixed project engine.
    pub engine: Engine,
    /// Whole compilation and rasterization latency.
    pub elapsed: Duration,
    /// Same-engine PDF pages, rasterized at product density.
    pub pages: Vec<RasterPage>,
    /// Compiler log; failed compilation returns an error instead of replacing preview.
    pub log: String,
}
/// Compile an immutable project snapshot, never modify source files. At most two passes for references.
/// # Errors
/// Rejects the live policy, LuaLaTeX, escaped/oversized resource trees, failed/late compilation or PDF.
pub async fn compile(
    source: &SourceSession,
    sandbox: &Sandbox,
    deadline: Duration,
) -> Result<FullPreview, LatexError> {
    if sandbox.profile() != BuildProfile::FullCompilation
        || source.engine() == Engine::LuaLatex
        || deadline.is_zero()
    {
        return Err(LatexError::Configuration);
    }
    let started = Instant::now();
    let project = tempfile::tempdir()?;
    let work = tempfile::tempdir()?;
    let mut budget = (0, 0);
    snapshot(source.root(), source.root(), project.path(), &mut budget)?;
    for (path, text) in source.documents() {
        std::fs::write(project.path().join(path), text)?;
    }
    let engine = match source.engine() {
        Engine::XeLatex => "/usr/bin/xelatex",
        Engine::PdfLatex => "/usr/bin/pdflatex",
        _ => return Err(LatexError::Configuration),
    };
    let mut args = vec![
        engine.into(),
        "-interaction=nonstopmode".into(),
        "-halt-on-error".into(),
        "-no-shell-escape".into(),
        "-jobname=preview".into(),
        "-output-directory=/work".into(),
        format!("/project/{}", source.entry()).into(),
    ];
    if source.engine() == Engine::XeLatex {
        args.insert(1, "-no-pdf".into());
    }
    let mut log = String::new();
    for pass in 0..2 {
        let remaining = deadline
            .checked_sub(started.elapsed())
            .ok_or(LatexError::Timeout)?;
        let output = sandbox
            .run(project.path(), work.path(), &args, remaining)
            .await?;
        log.push_str(&String::from_utf8_lossy(&output));
        let details = std::fs::read_to_string(work.path().join("preview.log")).unwrap_or_default();
        if pass > 0
            || !(details.contains("Rerun")
                || details.contains("rerun")
                || details.contains("undefined references"))
        {
            break;
        }
    }
    if source.engine() == Engine::XeLatex {
        let remaining = deadline
            .checked_sub(started.elapsed())
            .ok_or(LatexError::Timeout)?;
        let output = sandbox
            .run(
                project.path(),
                work.path(),
                &[
                    "/usr/bin/xdvipdfmx".into(),
                    "-o".into(),
                    "/work/preview.pdf".into(),
                    "/work/preview.xdv".into(),
                ],
                remaining,
            )
            .await?;
        log.push_str(&String::from_utf8_lossy(&output));
    }
    let path = work.path().join("preview.pdf");
    let meta = std::fs::symlink_metadata(&path)?;
    if !meta.is_file() || meta.len() > 64 * 1024 * 1024 {
        return Err(std::io::Error::other("invalid preview PDF").into());
    }
    let data = std::fs::read(path)?;
    let pages = crate::view::render_pdf(&data)?;
    if started.elapsed() > deadline {
        return Err(LatexError::Timeout);
    }
    debug_assert!(
        pages
            .iter()
            .all(|p| p.origin == RasterOrigin::FullCompilation)
    );
    Ok(FullPreview {
        revision: source.revision(),
        engine: source.engine(),
        elapsed: started.elapsed(),
        pages,
        log,
    })
}
pub(crate) fn snapshot(
    root: &Path,
    from: &Path,
    to: &Path,
    budget: &mut (u64, u64),
) -> Result<(), LatexError> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" || name == "target" {
            continue;
        }
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_symlink() || !(kind.is_file() || kind.is_dir()) {
            return Err(std::io::Error::other("non-regular project resource").into());
        }
        if !path.canonicalize()?.starts_with(root) {
            return Err(LatexError::Path(path.display().to_string()));
        }
        budget.0 += 1;
        budget.1 += if kind.is_file() {
            entry.metadata()?.len()
        } else {
            0
        };
        if budget.0 > 10_000 || budget.1 > 64 * 1024 * 1024 {
            return Err(std::io::Error::other("snapshot resource budget exceeded").into());
        }
        let target = to.join(name);
        if kind.is_dir() {
            std::fs::create_dir(&target)?;
            snapshot(root, &path, &target, budget)?;
        } else {
            std::fs::copy(path, target)?;
        }
    }
    Ok(())
}
