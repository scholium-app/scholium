//! Host/decoder boundary. hayro never parses project-controlled bytes in the host process.
use super::{RasterOrigin, RasterPage, invalid};
use crate::LatexError;
use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path, time::Duration};
const PDF_BYTES: u64 = 64 * 1024 * 1024;
const METADATA_BYTES: u64 = 16 * 1024;
const RGBA_BYTES: u64 = super::MAX_SCENE_PIXELS * 4;
#[derive(Serialize, Deserialize)]
struct Metadata {
    version: u32,
    pages: Vec<(u64, u32, u32)>,
}

pub(super) fn render(
    data: &[u8],
    wanted: Option<u64>,
    origin: RasterOrigin,
) -> Result<Vec<RasterPage>, LatexError> {
    if data.len() as u64 > PDF_BYTES
        || !data.starts_with(b"%PDF-")
        || wanted.is_some_and(|p| !(1..=256).contains(&p))
    {
        return Err(invalid("invalid or oversized PDF request"));
    }
    let data = data.to_vec();
    std::thread::Builder::new()
        .name("pdf-boundary".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(decode(data, wanted, origin))
        })?
        .join()
        .map_err(|_| invalid("PDF boundary worker failed"))?
}
async fn decode(
    data: Vec<u8>,
    wanted: Option<u64>,
    origin: RasterOrigin,
) -> Result<Vec<RasterPage>, LatexError> {
    let project = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    let request = tempfile::NamedTempFile::new()?;
    std::fs::write(project.path().join("input.pdf"), data)?;
    std::fs::write(
        request.path(),
        serde_json::to_vec(&wanted).map_err(std::io::Error::other)?,
    )?;
    let exe = std::env::current_exe()?;
    let mut directory = exe
        .parent()
        .ok_or_else(|| invalid("decoder installation"))?;
    if directory.file_name().is_some_and(|s| s == "deps") {
        directory = directory
            .parent()
            .ok_or_else(|| invalid("decoder installation"))?;
    }
    let worker = directory.join("scholium-pdf-worker");
    let sandbox = scholium_build::Sandbox::prepare_pdf_decode(&worker, request.path()).await?;
    sandbox
        .run(
            project.path(),
            output.path(),
            &["/worker".into()],
            Duration::from_secs(10),
        )
        .await?;
    let metadata: Metadata =
        serde_json::from_slice(&read(&output.path().join("pages.json"), METADATA_BYTES)?)
            .map_err(std::io::Error::other)?;
    frames(output.path(), metadata, wanted, origin)
}
fn frames(
    root: &Path,
    metadata: Metadata,
    wanted: Option<u64>,
    origin: RasterOrigin,
) -> Result<Vec<RasterPage>, LatexError> {
    if metadata.version != 1 || metadata.pages.is_empty() || metadata.pages.len() > 256 {
        return Err(invalid("PDF frame count/version"));
    }
    let mut remaining = RGBA_BYTES;
    let mut previous = 0;
    let mut out = Vec::new();
    for (index, (page, width, height)) in metadata.pages.into_iter().enumerate() {
        if !(1..=256).contains(&page) || page <= previous || wanted.is_some_and(|p| p != page) {
            return Err(invalid("PDF frame page"));
        }
        previous = page;
        super::dimensions(f64::from(width), f64::from(height))?;
        let size = u64::from(width) * u64::from(height) * 4;
        remaining = remaining
            .checked_sub(size)
            .ok_or_else(|| invalid("PDF frame total bytes"))?;
        let rgba = read(&root.join(format!("page-{index}.rgba")), size)?;
        if rgba.len() as u64 != size {
            return Err(invalid("truncated PDF frame"));
        }
        out.push(RasterPage {
            page,
            width,
            height,
            rgba: rgba.into(),
            origin,
        });
    }
    Ok(out)
}
fn read(path: &Path, limit: u64) -> Result<Vec<u8>, LatexError> {
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("non-regular PDF frame"));
    }
    let mut data = Vec::new();
    file.take(limit + 1).read_to_end(&mut data)?;
    if data.len() as u64 > limit {
        return Err(invalid("oversized PDF frame"));
    }
    Ok(data)
}
/// Fixed decoder mode. Must run only inside the OS-isolated decoder profile.
/// # Errors
/// Rejects malformed/oversized PDF or request, excessive pages/pixels and output IO failures.
pub fn entry() -> Result<(), LatexError> {
    let wanted = serde_json::from_slice(&read(Path::new("/request.json"), METADATA_BYTES)?)
        .map_err(std::io::Error::other)?;
    let pages = super::pdf::render_untrusted(
        &read(Path::new("/project/input.pdf"), PDF_BYTES)?,
        wanted,
        RasterOrigin::FullCompilation,
    )?;
    let metadata = Metadata {
        version: 1,
        pages: pages.iter().map(|p| (p.page, p.width, p.height)).collect(),
    };
    for (index, page) in pages.iter().enumerate() {
        std::fs::write(format!("/out/page-{index}.rgba"), &page.rgba)?;
    }
    std::fs::write(
        "/out/pages.json",
        serde_json::to_vec(&metadata).map_err(std::io::Error::other)?,
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hostile_decoder_frames_fail_before_allocating_pixels() {
        let root = tempfile::tempdir().expect("output");
        for dimensions in [[u32::MAX, u32::MAX], [0, 1], [8193, 1], [2, 2]] {
            let metadata = Metadata {
                version: 1,
                pages: vec![(1, dimensions[0], dimensions[1])],
            };
            assert!(frames(root.path(), metadata, None, RasterOrigin::FullCompilation).is_err());
        }
        std::fs::write(root.path().join("page-0.rgba"), [0; 3]).expect("truncated");
        assert!(
            frames(
                root.path(),
                Metadata {
                    version: 1,
                    pages: vec![(1, 1, 1)]
                },
                None,
                RasterOrigin::FullCompilation
            )
            .is_err()
        );
    }
}
