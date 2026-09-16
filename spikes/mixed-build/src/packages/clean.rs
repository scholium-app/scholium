//! Build with no repository, home directory, network, or previous artifacts mounted.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{ir::Dialect, latex, pdf_evidence};

pub(super) fn tools(root: &Path) -> io::Result<PathBuf> {
    let tools = root.join("toolchain");
    fs::create_dir(&tools)?;
    fs::copy(std::env::current_exe()?, tools.join("driver"))?;
    let typst = std::env::var_os("SCHOLIUM_TYPST_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/scholium-typst-toolchain/bin/typst"));
    let version = Command::new(&typst).arg("--version").output()?;
    if !version.status.success()
        || !String::from_utf8_lossy(&version.stdout).starts_with("typst 0.15.1")
    {
        return Err(io::Error::other(
            "Typst CLI 0.15.1 required; set SCHOLIUM_TYPST_BIN",
        ));
    }
    fs::copy(typst, tools.join("typst"))?;
    fs::write(tools.join("version.txt"), version.stdout)?;
    Ok(tools)
}

pub(super) fn rebuild_and_compare(
    package: &Path,
    original: &Path,
    tools: &Path,
    host: Dialect,
    mixed: bool,
) -> io::Result<()> {
    let output = package.with_extension("check");
    fs::create_dir(&output)?;
    // Only the generated package is visible as /project. No build output or cargo cache is mounted.
    let script = match (mixed, host) {
        (true, _) => {
            "test ! -e /home; /toolchain/driver --rebuild /project /work/rebuilt; cp /work/rebuilt/final.pdf /work/final.pdf"
        }
        (false, Dialect::Latex) => {
            "test ! -e /home; cp /project/*.tex /work/; for f in /project/*.pdf; do test ! -f \"$f\" || cp \"$f\" /work/; done; xelatex -no-shell-escape -interaction=nonstopmode -halt-on-error main.tex; xelatex -no-shell-escape -interaction=nonstopmode -halt-on-error main.tex; mv main.pdf final.pdf"
        }
        (false, Dialect::Typst) => {
            "test ! -e /home; /toolchain/typst compile --root /project /project/main.typ /work/final.pdf"
        }
    };
    let run = Command::new("/usr/bin/bash")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../toolchain-sandbox.sh"))
        .args([package.as_os_str(), output.as_os_str()])
        .args(["45", "/bin/sh", "-ec", script])
        .env("SCHOLIUM_SPIKE_TOOLS", tools)
        .output()?;
    fs::write(output.join("stdout.log"), &run.stdout)?;
    fs::write(output.join("stderr.log"), &run.stderr)?;
    if !run.status.success() {
        return Err(io::Error::other(format!(
            "clean build {} failed: {}; see {}",
            package.display(),
            run.status,
            output.display()
        )));
    }
    compare(&original.join("final.pdf"), &output.join("final.pdf"))
}

fn pdf_text(path: &Path) -> io::Result<String> {
    let output = Command::new("pdftotext").arg(path).arg("-").output()?;
    if !output.status.success() {
        return Err(io::Error::other("pdftotext failed"));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .collect::<String>())
}

fn compare(original: &Path, rebuilt: &Path) -> io::Result<()> {
    let pages = latex::pdf_pages(original);
    if pages == 0 || latex::pdf_pages(rebuilt) != pages {
        return Err(io::Error::other("page count changed"));
    }
    if pdf_text(original)? != pdf_text(rebuilt)? {
        return Err(io::Error::other(format!(
            "extracted text changed: {}",
            rebuilt.display()
        )));
    }
    for path in [original, rebuilt] {
        let probe = Command::new("pdftohtml")
            .args(["-xml", "-stdout", "-i", "-q"])
            .arg(path)
            .output()?;
        if !probe.status.success() {
            return Err(io::Error::other("link inspection failed"));
        }
    }
    let links = |path: &Path| {
        latex::pdf_links(path)
            .into_iter()
            .map(|(page, target)| {
                (
                    page,
                    target.rsplit('#').next().unwrap_or(&target).to_owned(),
                )
            })
            .collect::<Vec<_>>()
    };
    if links(original) != links(rebuilt) {
        return Err(io::Error::other("link destinations changed"));
    }
    if pdf_evidence::raster_images(rebuilt)? != 0 {
        return Err(io::Error::other("rebuilt PDF contains raster images"));
    }
    Ok(())
}
