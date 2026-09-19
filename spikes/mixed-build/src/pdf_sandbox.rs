//! Inspect untrusted build artifacts in the same OS sandbox as the compilers.
use std::fs;
use std::io::{self, Read};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const PROFILE: &str = include_str!("../../toolchain-sandbox.sh");
const MAX_INPUT: u64 = 64 * 1024 * 1024;
const MAX_OUTPUT: u64 = 8 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);

pub(crate) enum Probe {
    Info,
    Text(Option<usize>),
    Links,
    Images,
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl Scratch {
    fn new() -> io::Result<Self> {
        loop {
            let path = std::env::temp_dir().join(format!(
                "scholium-pdf-probe-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }
}

pub(crate) fn run(pdf: &Path, probe: Probe) -> io::Result<Output> {
    let metadata = pdf.symlink_metadata()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_INPUT {
        return Err(io::Error::other(
            "PDF probe requires a regular file of at most 64 MiB",
        ));
    }
    let scratch = Scratch::new()?;
    let input = scratch.0.join("input");
    let output = scratch.0.join("output");
    fs::create_dir(&input)?;
    fs::create_dir(&output)?;
    // A bounded read also prevents a growing file from bypassing the metadata check.
    let bytes = read_limited(pdf, MAX_INPUT)?;
    fs::write(input.join("document.pdf"), bytes)?;
    let (tool, args) = command(probe);
    let mut result = Command::new("/usr/bin/bash")
        .args(["-c", PROFILE, "scholium-pdf-sandbox"])
        .args([input.as_os_str(), output.as_os_str()])
        .args([
            "10",
            "/bin/sh",
            "-c",
            "exec \"$@\" > /work/stdout 2> /work/stderr",
            "pdf-probe",
            tool,
        ])
        .args(args)
        .env_clear()
        .env("SCHOLIUM_SANDBOX_PROFILE", "pdf-probe")
        .env("PATH", "/usr/bin")
        .output()?;
    // File redirection puts decompression output under RLIMIT_FSIZE rather than a growing pipe.
    if output.join("stdout").exists() {
        result.stdout = read_limited(&output.join("stdout"), MAX_OUTPUT)?;
    }
    if output.join("stderr").exists() {
        result
            .stderr
            .extend(read_limited(&output.join("stderr"), MAX_OUTPUT)?);
    }
    if !result.status.success() {
        return Err(io::Error::other(format!(
            "PDF probe failed: {}: {}",
            result.status,
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    Ok(result)
}

fn command(probe: Probe) -> (&'static str, Vec<String>) {
    let (tool, mut args): (_, Vec<String>) = match probe {
        Probe::Info => ("/usr/bin/pdfinfo", Vec::new()),
        Probe::Text(page) => (
            "/usr/bin/pdftotext",
            page.map_or_else(Vec::new, |page| {
                vec!["-f".into(), page.to_string(), "-l".into(), page.to_string()]
            }),
        ),
        Probe::Links => (
            "/usr/bin/pdftohtml",
            ["-xml", "-stdout", "-i", "-q"].map(String::from).to_vec(),
        ),
        Probe::Images => ("/usr/bin/pdfimages", vec!["-list".into()]),
    };
    args.push("/project/document.pdf".into());
    if matches!(probe, Probe::Text(_)) {
        args.push("-".into());
    }
    (tool, args)
}

fn read_limited(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::other("PDF probe byte budget exceeded"));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reduced_profile_inspects_a_valid_pdf_with_all_four_tools() {
        let scratch = Scratch::new().expect("scratch");
        let path = scratch.0.join("valid.pdf");
        let stream = "BT /F1 12 Tf 20 100 Td (BENIGNCONTROL) Tj ET\n";
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".into(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".into(),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
            format!("<< /Length {} >>\nstream\n{stream}endstream", stream.len()),
        ];
        let mut pdf = String::from("%PDF-1.4\n");
        let mut offsets = Vec::new();
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", index + 1));
        }
        let xref = pdf.len();
        pdf.push_str("xref\n0 6\n0000000000 65535 f \n");
        for offset in offsets {
            pdf.push_str(&format!("{offset:010} 00000 n \n"));
        }
        pdf.push_str(&format!(
            "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
        ));
        fs::write(&path, pdf).expect("fixture");
        assert!(
            String::from_utf8_lossy(&run(&path, Probe::Info).expect("info").stdout)
                .contains("Pages:")
        );
        assert!(
            String::from_utf8_lossy(&run(&path, Probe::Text(None)).expect("text").stdout)
                .contains("BENIGNCONTROL")
        );
        assert!(
            String::from_utf8_lossy(&run(&path, Probe::Links).expect("links").stdout)
                .contains("<page")
        );
        assert!(run(&path, Probe::Images).expect("images").status.success());
    }

    #[test]
    fn rejects_symlinks_oversize_and_malformed_artifacts() {
        let scratch = Scratch::new().expect("scratch");
        let path = scratch.0.join("file.pdf");
        fs::write(&path, "not a PDF").expect("input");
        assert!(run(&path, Probe::Info).is_err());
        let link = scratch.0.join("link.pdf");
        std::os::unix::fs::symlink(&path, &link).expect("link");
        assert!(run(&link, Probe::Info).is_err());
        fs::File::create(&path)
            .expect("file")
            .set_len(MAX_INPUT + 1)
            .expect("sparse length");
        assert!(run(&path, Probe::Info).is_err());
    }
}
